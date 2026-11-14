//! Make the Zig stack probes in a Windows static library invisible to the
//! linker, so the toolchain's own probe is used.
//!
//! Zig bundles its compiler_rt into `ghostty-vt-static.lib` and exports
//! `___chkstk_ms` and the other stack probes as COFF weak externals. GNU ld
//! takes a weak external as a definition, so it never pulls the strong
//! probe from libgcc or Rust's compiler_builtins. With `--gc-sections`,
//! which rustc always passes, ld then drops the section of the weak
//! default, and the link fails with "relocation truncated to fit ...
//! against undefined symbol `___chkstk_ms'".
//!
//! Renaming the weak externals makes the reference from Ghostty's code
//! undefined, so the linker takes the probe from libgcc or
//! compiler_builtins. The names stay in the symbol table, so no offset in
//! the archive moves.

/// The stack probes compiler_rt exports.
const STACK_PROBES: [&[u8]; 4] = [b"___chkstk", b"___chkstk_ms", b"__chkstk", b"__chkstk_ms"];

/// `IMAGE_SYM_CLASS_WEAK_EXTERNAL`.
const WEAK_EXTERNAL: u8 = 105;

const SYMBOL_SIZE: usize = 18;

/// Rename the weak stack probes of every COFF object in the `ar` archive
/// `archive`. Returns how many symbols were renamed.
pub fn hide_weak_stack_probes(archive: &mut [u8]) -> Result<usize, String> {
    const MAGIC: &[u8] = b"!<arch>\n";
    const HEADER: usize = 60;
    if !archive.starts_with(MAGIC) {
        return Err("not an ar archive".to_owned());
    }
    let mut renamed = 0;
    let mut at = MAGIC.len();
    while at < archive.len() {
        let header = archive
            .get(at..at + HEADER)
            .ok_or("truncated archive member header")?;
        let size = std::str::from_utf8(&header[48..58])
            .ok()
            .and_then(|size| size.trim().parse::<usize>().ok())
            .ok_or("bad archive member size")?;
        // The linker members ("/") and the long-name member ("//") are not
        // objects.
        let is_object = header[0] != b'/' || header[1].is_ascii_digit();
        let start = at + HEADER;
        let body = archive
            .get_mut(start..start + size)
            .ok_or("truncated archive member")?;
        if is_object {
            renamed += hide_in_object(body, renamed)?;
        }
        at = start + size + size % 2;
    }
    Ok(renamed)
}

/// Rename the weak stack probes of one COFF object. Members that are not
/// COFF objects, such as short import entries, are left alone. `first` is
/// the number of the first new name.
fn hide_in_object(object: &mut [u8], first: usize) -> Result<usize, String> {
    const I386: u16 = 0x14c;
    const AMD64: u16 = 0x8664;
    const ARM64: u16 = 0xaa64;
    let le16 = |b: &[u8], at: usize| u16::from_le_bytes([b[at], b[at + 1]]);
    let le32 = |b: &[u8], at: usize| u32::from_le_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]]);
    if object.len() < 20 || !matches!(le16(object, 0), I386 | AMD64 | ARM64) {
        return Ok(0);
    }
    let symbols = le32(object, 8) as usize;
    let count = le32(object, 12) as usize;
    let strings = symbols + count * SYMBOL_SIZE;
    if strings > object.len() {
        return Err("COFF symbol table is out of bounds".to_owned());
    }
    let mut renamed = 0;
    let mut index = 0;
    while index < count {
        let entry = symbols + index * SYMBOL_SIZE;
        let class = object[entry + 16];
        let aux = object[entry + 17] as usize;
        if class == WEAK_EXTERNAL {
            let name = symbol_name(object, entry, strings)?;
            if STACK_PROBES.contains(&name) {
                let number = first + renamed;
                if number > 0xffff {
                    return Err("too many weak stack probes".to_owned());
                }
                // An 8-byte inline name that no C or Rust code can declare.
                let hidden = format!(".gvt{number:04x}");
                object[entry..entry + 8].copy_from_slice(hidden.as_bytes());
                renamed += 1;
            }
        }
        index += 1 + aux;
    }
    Ok(renamed)
}

/// The name of the symbol at `entry`: inline, or in the string table at
/// `strings` when its first four bytes are zero.
fn symbol_name(object: &[u8], entry: usize, strings: usize) -> Result<&[u8], String> {
    let raw = &object[entry..entry + 8];
    let name = if raw[..4] == [0; 4] {
        let offset = u32::from_le_bytes([raw[4], raw[5], raw[6], raw[7]]) as usize;
        let tail = object
            .get(strings + offset..)
            .ok_or("COFF symbol name is out of bounds")?;
        let end = tail.iter().position(|&b| b == 0).unwrap_or(tail.len());
        &tail[..end]
    } else {
        let end = raw.iter().position(|&b| b == 0).unwrap_or(raw.len());
        &raw[..end]
    };
    Ok(name)
}

#[cfg(test)]
mod tests {
    use super::{SYMBOL_SIZE, WEAK_EXTERNAL, hide_weak_stack_probes};

    /// An x86_64 COFF object with `symbols` as (name, storage class).
    fn object(symbols: &[(&str, u8)]) -> Vec<u8> {
        let mut table = Vec::new();
        let mut strings = vec![0; 4];
        for &(name, class) in symbols {
            let mut entry = [0; SYMBOL_SIZE];
            if name.len() <= 8 {
                entry[..name.len()].copy_from_slice(name.as_bytes());
            } else {
                let offset = u32::try_from(strings.len()).unwrap();
                entry[4..8].copy_from_slice(&offset.to_le_bytes());
                strings.extend_from_slice(name.as_bytes());
                strings.push(0);
            }
            entry[16] = class;
            // Weak externals carry one auxiliary record.
            let aux = u8::from(class == WEAK_EXTERNAL);
            entry[17] = aux;
            table.extend_from_slice(&entry);
            if aux == 1 {
                table.extend_from_slice(&[0xee; SYMBOL_SIZE]);
            }
        }
        let size = u32::try_from(strings.len()).unwrap();
        strings[..4].copy_from_slice(&size.to_le_bytes());
        let count = u32::try_from(table.len() / SYMBOL_SIZE).unwrap();
        let mut object = Vec::new();
        object.extend_from_slice(&0x8664_u16.to_le_bytes());
        object.extend_from_slice(&0_u16.to_le_bytes());
        object.extend_from_slice(&0_u32.to_le_bytes());
        object.extend_from_slice(&20_u32.to_le_bytes());
        object.extend_from_slice(&count.to_le_bytes());
        object.extend_from_slice(&[0; 4]);
        object.extend_from_slice(&table);
        object.extend_from_slice(&strings);
        object
    }

    /// An ar archive of `members`, as (name field, body), and where each
    /// body starts.
    fn archive(members: &[(&str, Vec<u8>)]) -> (Vec<u8>, Vec<usize>) {
        let mut archive = b"!<arch>\n".to_vec();
        let mut starts = Vec::new();
        for (name, body) in members {
            let header = format!(
                "{name:<16}{:<12}{:<6}{:<6}{:<8}{:<10}`\n",
                0,
                0,
                0,
                0,
                body.len()
            );
            archive.extend_from_slice(header.as_bytes());
            starts.push(archive.len());
            archive.extend_from_slice(body);
            if body.len() % 2 == 1 {
                archive.push(b'\n');
            }
        }
        (archive, starts)
    }

    fn names(object: &[u8]) -> Vec<String> {
        let count = u32::from_le_bytes(object[12..16].try_into().unwrap()) as usize;
        let strings = 20 + count * SYMBOL_SIZE;
        let mut names = Vec::new();
        let mut index = 0;
        while index < count {
            let entry = 20 + index * SYMBOL_SIZE;
            let name = super::symbol_name(object, entry, strings).unwrap();
            names.push(String::from_utf8(name.to_vec()).unwrap());
            index += 1 + object[entry + 17] as usize;
        }
        names
    }

    #[test]
    fn only_the_weak_stack_probes_are_renamed() {
        let compiler_rt = object(&[
            ("___chkstk_ms", WEAK_EXTERNAL),
            ("__chkstk", WEAK_EXTERNAL),
            ("memcpy", WEAK_EXTERNAL),
            ("___chkstk", 2),
        ]);
        let user = object(&[("___chkstk_ms", 2)]);
        let (mut bytes, starts) = archive(&[
            ("/", vec![0; 3]),
            ("//", b"compiler_rt.obj\0".to_vec()),
            ("/0", compiler_rt.clone()),
            ("user.obj/", user.clone()),
        ]);
        let before = bytes.len();
        assert_eq!(hide_weak_stack_probes(&mut bytes), Ok(2));
        assert_eq!(bytes.len(), before);

        let patched = &bytes[starts[2]..starts[2] + compiler_rt.len()];
        assert_eq!(
            names(patched),
            [".gvt0000", ".gvt0001", "memcpy", "___chkstk"]
        );
        assert_eq!(&bytes[starts[3]..starts[3] + user.len()], user.as_slice());
    }

    #[test]
    fn an_archive_without_weak_probes_is_unchanged() {
        let (mut bytes, _) = archive(&[("a.obj/", object(&[("main", 2)]))]);
        let before = bytes.clone();
        assert_eq!(hide_weak_stack_probes(&mut bytes), Ok(0));
        assert_eq!(bytes, before);
    }

    #[test]
    fn a_file_that_is_not_an_archive_is_an_error() {
        assert!(hide_weak_stack_probes(&mut b"MZ".to_vec()).is_err());
    }
}
