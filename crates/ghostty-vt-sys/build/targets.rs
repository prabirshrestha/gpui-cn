//! Which prebuilt library of the pinned release a Rust target links, and
//! whether the MSVC linker can take it.

/// A file of the release, for one Rust target.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Asset {
    /// A slice of the XCFramework.
    Apple {
        slice: &'static str,
        archive: &'static str,
        /// The architecture to take from a universal archive.
        arch: Option<MachArch>,
    },
    /// A Windows zip, which holds a GNU-ABI build.
    Windows(WindowsArch),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MachArch {
    X86_64,
    Arm64,
}

impl MachArch {
    /// The Mach-O CPU type of this architecture.
    pub fn cpu_type(self) -> u32 {
        const ABI64: u32 = 0x0100_0000;
        match self {
            Self::X86_64 => ABI64 | 7,
            Self::Arm64 => ABI64 | 12,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WindowsArch {
    X86_64,
    Aarch64,
}

impl WindowsArch {
    /// The Zig triple in the zip's name.
    pub fn zig_triple(self) -> &'static str {
        match self {
            Self::X86_64 => "x86_64-windows-gnu",
            Self::Aarch64 => "aarch64-windows-gnu",
        }
    }
}

/// The prebuilt asset `target` links, or `None` when it builds from source.
/// The MSVC targets use the Windows zip only with `prebuilt_msvc`.
pub fn asset(target: &str, prebuilt_msvc: bool) -> Option<Asset> {
    let apple = |slice, archive, arch| Asset::Apple {
        slice,
        archive,
        arch,
    };
    Some(match target {
        "x86_64-apple-darwin" => apple(
            "macos-arm64_x86_64",
            "libghostty-vt.a",
            Some(MachArch::X86_64),
        ),
        "aarch64-apple-darwin" => apple(
            "macos-arm64_x86_64",
            "libghostty-vt.a",
            Some(MachArch::Arm64),
        ),
        "aarch64-apple-ios" => apple("ios-arm64", "libghostty-vt-fat.a", None),
        "aarch64-apple-ios-sim" => apple("ios-arm64-simulator", "libghostty-vt-fat.a", None),
        "x86_64-pc-windows-gnu" | "x86_64-pc-windows-gnullvm" => {
            Asset::Windows(WindowsArch::X86_64)
        }
        "aarch64-pc-windows-gnullvm" => Asset::Windows(WindowsArch::Aarch64),
        "x86_64-pc-windows-msvc" if prebuilt_msvc => Asset::Windows(WindowsArch::X86_64),
        "aarch64-pc-windows-msvc" if prebuilt_msvc => Asset::Windows(WindowsArch::Aarch64),
        _ => return None,
    })
}

/// The linker rustc uses for the target: the last `-C linker=` in the
/// encoded RUSTFLAGS (`\x1f`-separated), else Cargo's `RUSTC_LINKER`.
pub fn linker<'a>(rustc_linker: Option<&'a str>, encoded_rustflags: &'a str) -> Option<&'a str> {
    let mut found = None;
    let mut flags = encoded_rustflags
        .split('\x1f')
        .filter(|flag| !flag.is_empty());
    while let Some(flag) = flags.next() {
        let value = match flag {
            "-C" | "--codegen" => flags.next(),
            _ => flag
                .strip_prefix("-C")
                .or_else(|| flag.strip_prefix("--codegen=")),
        };
        if let Some(linker) = value.and_then(|value| value.strip_prefix("linker=")) {
            found = Some(linker);
        }
    }
    found.or(rustc_linker.filter(|linker| !linker.is_empty()))
}

/// Whether `linker` is Microsoft's `link.exe`. No linker means rustc's
/// default for MSVC targets, which is `link.exe`.
pub fn is_link_exe(linker: Option<&str>) -> bool {
    let Some(linker) = linker else {
        return true;
    };
    let name = linker.rsplit(['/', '\\']).next().unwrap_or(linker);
    let stem = name
        .rsplit_once('.')
        .filter(|(_, ext)| ext.eq_ignore_ascii_case("exe"))
        .map_or(name, |(stem, _)| stem);
    stem.eq_ignore_ascii_case("link")
}

#[cfg(test)]
mod tests {
    use super::{Asset, MachArch, WindowsArch, asset, is_link_exe, linker};

    #[test]
    fn targets_map_to_their_release_files() {
        let windows = |arch| Some(Asset::Windows(arch));
        assert_eq!(
            asset("x86_64-pc-windows-gnu", false),
            windows(WindowsArch::X86_64)
        );
        assert_eq!(
            asset("x86_64-pc-windows-gnullvm", false),
            windows(WindowsArch::X86_64)
        );
        assert_eq!(
            asset("aarch64-pc-windows-gnullvm", false),
            windows(WindowsArch::Aarch64)
        );
        assert_eq!(
            asset("x86_64-pc-windows-msvc", true),
            windows(WindowsArch::X86_64)
        );
        assert_eq!(
            asset("aarch64-pc-windows-msvc", true),
            windows(WindowsArch::Aarch64)
        );
        assert_eq!(
            asset("aarch64-apple-darwin", false),
            Some(Asset::Apple {
                slice: "macos-arm64_x86_64",
                archive: "libghostty-vt.a",
                arch: Some(MachArch::Arm64),
            })
        );
        assert_eq!(WindowsArch::Aarch64.zig_triple(), "aarch64-windows-gnu");
    }

    #[test]
    fn msvc_builds_from_source_without_the_opt_in() {
        assert_eq!(asset("x86_64-pc-windows-msvc", false), None);
        assert_eq!(asset("aarch64-pc-windows-msvc", false), None);
        assert_eq!(asset("x86_64-unknown-linux-gnu", true), None);
    }

    #[test]
    fn the_linker_comes_from_rustflags_then_cargo() {
        assert_eq!(linker(Some("lld-link"), ""), Some("lld-link"));
        assert_eq!(linker(Some(""), ""), None);
        assert_eq!(linker(None, "-Clinker=rust-lld"), Some("rust-lld"));
        assert_eq!(
            linker(Some("link.exe"), "-C\x1flinker=a\x1f--codegen=linker=b"),
            Some("b")
        );
        assert_eq!(linker(None, "-C\x1fopt-level=3"), None);
    }

    #[test]
    fn only_microsoft_link_is_link_exe() {
        assert!(is_link_exe(None));
        assert!(is_link_exe(Some("link")));
        assert!(is_link_exe(Some(r"C:\VS\bin\Hostx64\x64\LINK.EXE")));
        assert!(!is_link_exe(Some("lld-link")));
        assert!(!is_link_exe(Some("/usr/bin/lld-link.exe")));
        assert!(!is_link_exe(Some("rust-lld")));
    }
}
