//! Make the Zig compiler_rt in a Windows GNU-ABI static library link with
//! other toolchains, by renaming symbols in place.
//!
//! Zig bundles its compiler_rt into `ghostty-vt-static.lib`. It exports each
//! routine as a COFF weak external whose default is a global symbol named
//! `.weak.<name>.default`. Two problems follow:
//!
//! - GNU ld takes a weak external as a definition, so it never pulls the
//!   strong `___chkstk_ms` from libgcc or Rust's compiler_builtins. With
//!   `--gc-sections`, which rustc always passes, ld then drops the section
//!   of the weak default, and the link fails with "relocation truncated to
//!   fit ... against undefined symbol `___chkstk_ms'". So for GNU ld the
//!   weak stack probes are renamed: Ghostty's reference is then undefined,
//!   and the linker takes the toolchain's probe.
//! - Zig's own linker (`zig cc`, as cargo-zigbuild uses) adds a second Zig
//!   compiler_rt, with the same `.weak.<name>.default` globals, and fails
//!   with "duplicate symbol". So the defaults are renamed too. A weak
//!   external finds its default by symbol index, not by name, so this
//!   changes nothing else.
//!
//! Every new name is an 8-byte inline name that no C or Rust code can
//! declare. Names only change in the symbol table, so no offset in the
//! archive moves.

/// The stack probes compiler_rt exports.
const STACK_PROBES: [&[u8]; 4] = [b"___chkstk", b"___chkstk_ms", b"__chkstk", b"__chkstk_ms"];

/// `IMAGE_SYM_CLASS_EXTERNAL`.
const EXTERNAL: u8 = 2;
/// `IMAGE_SYM_CLASS_WEAK_EXTERNAL`.
const WEAK_EXTERNAL: u8 = 105;

const SYMBOL_SIZE: usize = 18;

/// Rename, in every COFF object of the `ar` archive `archive`, the Zig weak
/// defaults, and with `stack_probes` the weak stack probes too. Returns how
/// many symbols were renamed.
pub fn patch(archive: &mut [u8], stack_probes: bool) -> Result<usize, String> {
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
            renamed = patch_object(body, renamed, stack_probes)?;
        }
        at = start + size + size % 2;
    }
    Ok(renamed)
}

/// Rename the symbols of one COFF object. Members that are not COFF
/// objects, such as short import entries, are left alone. `renamed` counts
/// the names given so far, and the new count is returned.
fn patch_object(
    object: &mut [u8],
    mut renamed: usize,
    stack_probes: bool,
) -> Result<usize, String> {
    const I386: u16 = 0x14c;
    const AMD64: u16 = 0x8664;
    const ARM64: u16 = 0xaa64;
    let le16 = |b: &[u8], at: usize| u16::from_le_bytes([b[at], b[at + 1]]);
    let le32 = |b: &[u8], at: usize| u32::from_le_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]]);
    if object.len() < 20 || !matches!(le16(object, 0), I386 | AMD64 | ARM64) {
        return Ok(renamed);
    }
    let symbols = le32(object, 8) as usize;
    let count = le32(object, 12) as usize;
    let strings = symbols + count * SYMBOL_SIZE;
    if strings > object.len() {
        return Err("COFF symbol table is out of bounds".to_owned());
    }
    let mut index = 0;
    while index < count {
        let entry = symbols + index * SYMBOL_SIZE;
        let class = object[entry + 16];
        let aux = object[entry + 17] as usize;
        let name = symbol_name(object, entry, strings)?;
        let rename = match class {
            WEAK_EXTERNAL => stack_probes && STACK_PROBES.contains(&name),
            EXTERNAL => name.starts_with(b".weak.") && name.ends_with(b".default"),
            _ => false,
        };
        if rename {
            if renamed > 0xff_ffff {
                return Err("too many compiler_rt symbols".to_owned());
            }
            let hidden = format!(".g{renamed:06x}");
            object[entry..entry + 8].copy_from_slice(hidden.as_bytes());
            renamed += 1;
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
    use super::{EXTERNAL, SYMBOL_SIZE, WEAK_EXTERNAL, patch};

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

    fn compiler_rt() -> Vec<u8> {
        object(&[
            ("___chkstk_ms", WEAK_EXTERNAL),
            (".weak.___chkstk_ms.default", EXTERNAL),
            ("__chkstk", WEAK_EXTERNAL),
            ("memcpy", WEAK_EXTERNAL),
            (".weak.memcpy.default", EXTERNAL),
            ("___chkstk", EXTERNAL),
        ])
    }

    #[test]
    fn the_weak_defaults_and_stack_probes_are_renamed() {
        let compiler_rt = compiler_rt();
        let user = object(&[("___chkstk_ms", EXTERNAL), (".weak.x.y", EXTERNAL)]);
        let (mut bytes, starts) = archive(&[
            ("/", vec![0; 3]),
            ("//", b"compiler_rt.obj\0".to_vec()),
            ("/0", compiler_rt.clone()),
            ("user.obj/", user.clone()),
        ]);
        let before = bytes.len();
        assert_eq!(patch(&mut bytes, true), Ok(4));
        assert_eq!(bytes.len(), before);

        let patched = &bytes[starts[2]..starts[2] + compiler_rt.len()];
        assert_eq!(
            names(patched),
            [
                ".g000000",
                ".g000001",
                ".g000002",
                "memcpy",
                ".g000003",
                "___chkstk"
            ]
        );
        assert_eq!(&bytes[starts[3]..starts[3] + user.len()], user.as_slice());
    }

    #[test]
    fn without_stack_probes_only_the_weak_defaults_are_renamed() {
        let compiler_rt = compiler_rt();
        let (mut bytes, starts) = archive(&[("compiler_rt.obj/", compiler_rt.clone())]);
        assert_eq!(patch(&mut bytes, false), Ok(2));
        let patched = &bytes[starts[0]..starts[0] + compiler_rt.len()];
        assert_eq!(
            names(patched),
            [
                "___chkstk_ms",
                ".g000000",
                "__chkstk",
                "memcpy",
                ".g000001",
                "___chkstk"
            ]
        );
    }

    #[test]
    fn an_archive_without_compiler_rt_is_unchanged() {
        let (mut bytes, _) = archive(&[("a.obj/", object(&[("main", EXTERNAL)]))]);
        let before = bytes.clone();
        assert_eq!(patch(&mut bytes, true), Ok(0));
        assert_eq!(bytes, before);
    }

    #[test]
    fn a_file_that_is_not_an_archive_is_an_error() {
        assert!(patch(&mut b"MZ".to_vec(), true).is_err());
    }
}
