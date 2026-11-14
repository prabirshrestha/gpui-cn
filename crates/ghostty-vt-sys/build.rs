//! Link libghostty-vt: a pinned prebuilt library where the release has one,
//! otherwise a Zig build of the pinned source.
//!
//! Lookup order (see `build/source.rs` for how the overrides are chosen):
//!
//! 1. `GHOSTTY_SOURCE_DIR`: build this Ghostty checkout or extracted source
//!    tarball. `GHOSTTY_VT_FROM_SOURCE=1` skips steps 2 to 4.
//! 2. `GHOSTTY_VT_LIB_DIR`: link the static library in this directory as it
//!    is, with no download and no Zig. It must match the C ABI of the pinned
//!    release. `GHOSTTY_VT_LIB_SHA256` checks it.
//! 3. `GHOSTTY_VT_PREBUILT_URL`: download this archive instead of the
//!    target's release file, and check it against
//!    `GHOSTTY_VT_PREBUILT_SHA256`. It has the layout of the release file.
//! 4. The prebuilt library of the pinned libghostty-vt release, for the Apple
//!    targets (the XCFramework) and the Windows GNU targets (the Windows zip).
//!    It is downloaded (from `GHOSTTY_RELEASE_URL` when set), or read from
//!    `GHOSTTY_RELEASE_DIR`, and verified
//!    against the sha256 in `GHOSTTY.lock`. No Zig is needed. Both link the
//!    static library; for the Windows GNU targets symbols of Zig's
//!    compiler_rt in it are renamed first, so GNU ld and Zig's own linker
//!    take it (see `build/compiler_rt.rs`). With
//!    `GHOSTTY_VT_PREBUILT_MSVC=1` the Windows MSVC targets link the
//!    Windows zip too, unchanged; that needs lld as the linker, and the
//!    build stops when the linker is `link.exe`.
//! 5. The `third_party/ghostty` git submodule at the repository root.
//! 6. With the `pkg-config` feature, an installed `libghostty-vt-static`.
//! 7. A download of `GHOSTTY_SOURCE_URL` (default: the URL in `GHOSTTY.lock`)
//!    verified against the sha256 in `GHOSTTY.lock`.
//!
//! A source build of a release also downloads the release's dependency
//! tarball and passes it to `zig build --system`, so Zig fetches nothing.
//! `GHOSTTY_ZIG_SYSTEM_DIR` replaces that directory. The source tree is
//! copied into `$OUT_DIR/src` before Zig runs so that Zig never writes into
//! the submodule or the registry directory.
//!
//! Adapted from libghostty-vt-sys 0.2.2 (MIT OR Apache-2.0).

use std::env;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use sha2::{Digest, Sha256};

#[path = "build/compiler_rt.rs"]
mod compiler_rt;
#[path = "build/source.rs"]
mod source;
#[path = "build/targets.rs"]
mod targets;

use source::{Library, Origin};
use targets::{Asset, MachArch};

/// A release file: its name in the release directory and its sha256.
struct ReleaseFile {
    name: String,
    sha256: String,
}

/// Pinned Ghostty source, parsed from `GHOSTTY.lock`.
struct Lock {
    commit: String,
    version: String,
    url: String,
    sha256: String,
    root: String,
    /// The libghostty-vt release, present for a `vt-X.Y.Z` pin.
    release: Option<Release>,
}

/// The files of a libghostty-vt release beside its source tarball.
struct Release {
    tag: String,
    url: String,
    deps: ReleaseFile,
    deps_root: String,
    xcframework: ReleaseFile,
    windows_x86_64: ReleaseFile,
    windows_aarch64: ReleaseFile,
}

impl Lock {
    fn read(path: &Path) -> Self {
        let text = fs::read_to_string(path)
            .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));
        let optional = |name: &str| -> Option<String> {
            text.lines()
                .filter_map(|line| line.split_once('='))
                .find(|(key, _)| key.trim() == name)
                .map(|(_, value)| value.trim().to_owned())
        };
        let field = |name: &str| -> String {
            optional(name).unwrap_or_else(|| panic!("{} has no `{name}` field", path.display()))
        };
        let file = |name: &str| ReleaseFile {
            name: field(name),
            sha256: field(&format!("{name}_sha256")),
        };
        let release = optional("tag").map(|tag| Release {
            tag,
            url: field("release"),
            deps: file("deps"),
            deps_root: field("deps_root"),
            xcframework: file("xcframework"),
            windows_x86_64: file("windows_x86_64"),
            windows_aarch64: file("windows_aarch64"),
        });
        Self {
            commit: field("commit"),
            version: field("version"),
            url: field("url"),
            sha256: field("sha256"),
            root: field("root"),
            release,
        }
    }
}

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=GHOSTTY.lock");
    for knob in [
        "DOCS_RS",
        "GHOSTTY_RELEASE_DIR",
        "GHOSTTY_RELEASE_URL",
        "GHOSTTY_SOURCE_DIR",
        "GHOSTTY_SOURCE_URL",
        "GHOSTTY_VT_CPU",
        "GHOSTTY_VT_FROM_SOURCE",
        "GHOSTTY_VT_INCLUDE_DIR",
        "GHOSTTY_VT_LIB_DIR",
        "GHOSTTY_VT_LIB_SHA256",
        "GHOSTTY_VT_PREBUILT_MSVC",
        "GHOSTTY_VT_PREBUILT_SHA256",
        "GHOSTTY_VT_PREBUILT_URL",
        "GHOSTTY_ZIG_SYSTEM_DIR",
        "ZIG",
    ] {
        println!("cargo:rerun-if-env-changed={knob}");
    }

    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let repo_root = manifest_dir.join("..").join("..");
    let submodule = repo_root.join("third_party").join("ghostty");
    // The gitlink file changes when the submodule moves to another commit.
    // Never watch the source files themselves: a checkout touches mtimes.
    let gitlink = submodule.join(".git");
    println!("cargo:rerun-if-changed={}", gitlink.display());
    if let Some(head) = submodule_head(&gitlink) {
        println!("cargo:rerun-if-changed={}", head.display());
    }

    let lock = Lock::read(&manifest_dir.join("GHOSTTY.lock"));
    let build_metadata = validate_package_version(&lock);
    println!("cargo:rustc-env=GHOSTTY_VT_COMMIT={}", lock.commit);
    println!("cargo:rustc-env=GHOSTTY_VT_VERSION={}", lock.version);
    println!("cargo::metadata=COMMIT={}", lock.commit);
    println!("cargo::metadata=VERSION={}", lock.version);
    // ghostty-vt checks that its own version carries the same metadata.
    println!("cargo::metadata=BUILD_METADATA={build_metadata}");

    // docs.rs has no Zig toolchain. The committed bindings are enough to
    // build the documentation.
    if env::var_os("DOCS_RS").is_some() {
        return;
    }

    let target = env::var("TARGET").expect("TARGET");

    let (library, warnings) =
        source::choose(&overrides()).unwrap_or_else(|error| panic!("{error}"));
    for warning in warnings {
        println!("cargo:warning={warning}");
    }

    let flag = |name| env::var(name).is_ok_and(|v| !v.is_empty() && v != "0");
    let prebuilt_msvc = flag("GHOSTTY_VT_PREBUILT_MSVC") && target.ends_with("windows-msvc");
    match library {
        Library::SourceDir(dir) => {
            let dir = PathBuf::from(dir);
            assert!(
                dir.join("build.zig").exists(),
                "GHOSTTY_SOURCE_DIR does not contain build.zig: {}",
                dir.display()
            );
            build_from(&dir, &lock, &target);
            return;
        }
        Library::LibDir { dir, sha256 } => {
            link_lib_dir(&lock, &target, Path::new(&dir), sha256.as_deref());
            return;
        }
        Library::PrebuiltUrl { url, sha256 } => {
            let prebuilt = lock
                .release
                .as_ref()
                .and_then(|release| Prebuilt::for_target(release, &target, prebuilt_msvc))
                .unwrap_or_else(|| {
                    panic!(
                        "GHOSTTY_VT_PREBUILT_URL replaces the prebuilt release file of the \
                         target, and {target} links none (an MSVC target links the Windows \
                         zip only with GHOSTTY_VT_PREBUILT_MSVC=1). Use GHOSTTY_VT_LIB_DIR to \
                         link a library on disk."
                    )
                });
            if prebuilt_msvc {
                check_msvc_linker();
            }
            let file = ReleaseFile {
                name: prebuilt.file.name.clone(),
                sha256,
            };
            link_prebuilt(&file, Origin::Url(url), &prebuilt, &target);
            return;
        }
        Library::Release => {
            if let Some(release) = &lock.release
                && let Some(prebuilt) = Prebuilt::for_target(release, &target, prebuilt_msvc)
            {
                if prebuilt_msvc {
                    check_msvc_linker();
                }
                let origin =
                    source::release_origin(&overrides(), &release.url, &prebuilt.file.name);
                link_prebuilt(prebuilt.file, origin, &prebuilt, &target);
                return;
            }
        }
        Library::FromSource => {}
    }

    if submodule.join("build.zig").exists() {
        build_from(&submodule, &lock, &target);
        return;
    }

    #[cfg(feature = "pkg-config")]
    if try_pkg_config(&target) {
        return;
    }

    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR"));
    let src = out_dir.join("src");
    download_and_extract(&lock, &src);
    build_tree(&src, &lock, &target);
}

/// Check that the published crate version identifies the pinned Ghostty source.
///
/// A release pin carries `+ghostty-vt.X.Y.Z`, a tip pin carries
/// `+ghostty.<ghostty-version>.<short-commit>`.
fn validate_package_version(lock: &Lock) -> String {
    let short_commit = lock
        .commit
        .get(..7)
        .unwrap_or_else(|| panic!("GHOSTTY.lock has an invalid commit: {}", lock.commit));
    let expected = if let Some(release) = &lock.release {
        assert_eq!(
            release.tag,
            format!("vt-{}", lock.version),
            "GHOSTTY.lock tag does not match its version"
        );
        format!("ghostty-vt.{}", lock.version)
    } else {
        let (ghostty_version, version_commit) =
            lock.version.rsplit_once('+').unwrap_or_else(|| {
                panic!(
                    "GHOSTTY.lock version must end in the short commit: {}",
                    lock.version
                )
            });
        assert_eq!(
            version_commit, short_commit,
            "GHOSTTY.lock version does not match its commit"
        );
        format!(
            "ghostty.{}.{}",
            ghostty_version.replace('+', "."),
            short_commit
        )
    };

    let package_version = env::var("CARGO_PKG_VERSION").expect("CARGO_PKG_VERSION");
    let metadata = package_version.split_once('+').map(|(_, value)| value);
    assert_eq!(
        metadata,
        Some(expected.as_str()),
        "ghostty-vt-sys version must end in +{expected}"
    );
    expected
}

/// Resolve the HEAD file of the submodule's git directory, if checked out.
fn submodule_head(gitlink: &Path) -> Option<PathBuf> {
    let text = fs::read_to_string(gitlink).ok()?;
    let rel = text.strip_prefix("gitdir:")?.trim();
    let dir = gitlink.parent()?.join(rel);
    let head = dir.join("HEAD");
    head.exists().then_some(head)
}

/// A prebuilt library of the pinned release for one Rust target.
struct Prebuilt<'a> {
    file: &'a ReleaseFile,
    /// The directory inside the zip that holds the library and headers.
    dir: String,
    kind: PrebuiltKind,
}

enum PrebuiltKind {
    /// An XCFramework slice: a static archive, `Headers/` beside it.
    Apple {
        archive: &'static str,
        /// The architecture to take from a universal archive.
        arch: Option<MachArch>,
    },
    /// The Windows install tree, a GNU-ABI build: the static library
    /// `lib/ghostty-vt-static.lib` and `include/`.
    Windows,
}

impl<'a> Prebuilt<'a> {
    fn for_target(release: &'a Release, target: &str, prebuilt_msvc: bool) -> Option<Self> {
        Some(match targets::asset(target, prebuilt_msvc)? {
            Asset::Apple {
                slice,
                archive,
                arch,
            } => Prebuilt {
                file: &release.xcframework,
                dir: format!("ghostty-vt.xcframework/{slice}"),
                kind: PrebuiltKind::Apple { archive, arch },
            },
            Asset::Windows(arch) => Prebuilt {
                file: match arch {
                    targets::WindowsArch::X86_64 => &release.windows_x86_64,
                    targets::WindowsArch::Aarch64 => &release.windows_aarch64,
                },
                dir: format!(
                    "ghostty-vt-{}-{}",
                    release_version(release),
                    arch.zig_triple()
                ),
                kind: PrebuiltKind::Windows,
            },
        })
    }
}

/// Stop before a link that cannot work: Microsoft `link.exe` rejects the
/// COMDAT sections of the GNU-ABI objects in the Windows zip (LNK1143).
fn check_msvc_linker() {
    let rustc_linker = env::var("RUSTC_LINKER").ok();
    let rustflags = env::var("CARGO_ENCODED_RUSTFLAGS").unwrap_or_default();
    let linker = targets::linker(rustc_linker.as_deref(), &rustflags);
    assert!(
        !targets::is_link_exe(linker),
        "GHOSTTY_VT_PREBUILT_MSVC=1 links the prebuilt GNU-ABI static library, which \
         Microsoft link.exe rejects (LNK1143), and the linker here is {}. Link with lld: \
         use cargo-xwin, or set CARGO_TARGET_<TRIPLE>_LINKER=rust-lld (or lld-link). \
         Or unset GHOSTTY_VT_PREBUILT_MSVC to build from source with Zig.",
        linker.unwrap_or("rustc's default, link.exe")
    );
}

/// The libghostty-vt version of a release, from its `vt-X.Y.Z` tag.
fn release_version(release: &Release) -> &str {
    release.tag.strip_prefix("vt-").unwrap_or(&release.tag)
}

/// The overrides in the environment.
fn overrides() -> source::Env {
    source::Env::read(|name| env::var(name).ok())
}

/// Link the static library in `dir` as it is: `libghostty-vt.a`, or
/// `ghostty-vt-static.lib` on Windows. A universal Apple archive gives the
/// target's slice, and a Windows GNU library gets the compiler_rt renames,
/// both in a copy in `OUT_DIR`.
fn link_lib_dir(lock: &Lock, target: &str, dir: &Path, sha256: Option<&str>) {
    let windows = target.contains("windows");
    let name = if windows {
        "ghostty-vt-static.lib"
    } else {
        "libghostty-vt.a"
    };
    let archive = dir.join(name);
    println!("cargo:rerun-if-changed={}", archive.display());
    println!(
        "cargo:warning=GHOSTTY_VT_LIB_DIR links {} as it is. It must match the C ABI of \
         libghostty-vt {} that these bindings were generated from.",
        archive.display(),
        lock.version
    );
    let bytes = fs::read(&archive).unwrap_or_else(|error| {
        panic!(
            "GHOSTTY_VT_LIB_DIR has no {name} (failed to read {}: {error})",
            archive.display()
        )
    });
    if let Some(expected) = sha256 {
        let actual = format!("{:x}", Sha256::digest(&bytes));
        assert_eq!(
            actual,
            expected,
            "{} does not match GHOSTTY_VT_LIB_SHA256",
            archive.display()
        );
    }

    if windows {
        link_windows_static(target, dir);
    } else {
        let arch = match targets::asset(target, false) {
            Some(Asset::Apple { arch, .. }) => arch,
            _ => None,
        };
        let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR"));
        let lib_dir = out_dir.join("lib");
        fs::create_dir_all(&lib_dir)
            .unwrap_or_else(|error| panic!("failed to create {}: {error}", lib_dir.display()));
        let thin = match arch {
            Some(arch) => thin_archive(&bytes, arch)
                .unwrap_or_else(|error| panic!("{}: {error}", archive.display())),
            None => bytes.as_slice(),
        };
        let copy = lib_dir.join(name);
        fs::write(&copy, thin)
            .unwrap_or_else(|error| panic!("failed to write {}: {error}", copy.display()));
        println!("cargo:rustc-link-search=native={}", lib_dir.display());
        println!("cargo:rustc-link-lib=static=ghostty-vt");
    }

    let include = env::var_os("GHOSTTY_VT_INCLUDE_DIR")
        .filter(|dir| !dir.is_empty())
        .map(PathBuf::from)
        .or_else(|| {
            let beside = dir.join("..").join("include");
            beside
                .join("ghostty")
                .join("vt.h")
                .exists()
                .then_some(beside)
        });
    if let Some(include) = include {
        println!("cargo:include={}", include.display());
    }
}

/// Fetch `file` from `origin`, verify, extract and link a prebuilt library.
fn link_prebuilt(file: &ReleaseFile, origin: Origin, prebuilt: &Prebuilt, target: &str) {
    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR"));
    let dest = out_dir.join("prebuilt");
    let stamp = dest.join(".ghostty-vt-sha256");
    let stamp_value = format!("{} {}", file.sha256, prebuilt.dir);
    let dir = dest.join(&prebuilt.dir);
    if fs::read_to_string(&stamp).ok().as_deref() != Some(stamp_value.as_str()) {
        if dest.exists() {
            fs::remove_dir_all(&dest)
                .unwrap_or_else(|error| panic!("failed to remove {}: {error}", dest.display()));
        }
        let bytes = verified_bytes(file, origin);
        extract_zip(&bytes, &prebuilt.dir, &dest);
        fs::write(&stamp, &stamp_value)
            .unwrap_or_else(|error| panic!("failed to write {}: {error}", stamp.display()));
    }

    match prebuilt.kind {
        PrebuiltKind::Apple { archive, arch } => {
            let source = dir.join(archive);
            let lib_dir = out_dir.join("lib");
            fs::create_dir_all(&lib_dir)
                .unwrap_or_else(|error| panic!("failed to create {}: {error}", lib_dir.display()));
            // rustc bundles the archive into the rlib and cannot read a
            // universal archive, so take the target's slice out of it.
            let bytes = fs::read(&source)
                .unwrap_or_else(|error| panic!("failed to read {}: {error}", source.display()));
            let thin = match arch {
                Some(arch) => thin_archive(&bytes, arch)
                    .unwrap_or_else(|error| panic!("{}: {error}", source.display())),
                None => bytes.as_slice(),
            };
            let archive = lib_dir.join("libghostty-vt.a");
            fs::write(&archive, thin)
                .unwrap_or_else(|error| panic!("failed to write {}: {error}", archive.display()));
            println!("cargo:rustc-link-search=native={}", lib_dir.display());
            println!("cargo:rustc-link-lib=static=ghostty-vt");
            println!("cargo:include={}", dir.join("Headers").display());
        }
        PrebuiltKind::Windows => {
            link_windows_static(target, &dir.join("lib"));
            println!("cargo:include={}", dir.join("include").display());
        }
    }
}

/// The slice of `arch` in a universal (fat) Mach-O archive, or the archive
/// itself when it is not universal.
fn thin_archive(bytes: &[u8], arch: MachArch) -> Result<&[u8], String> {
    const FAT_MAGIC: u32 = 0xcafe_babe;
    const FAT_MAGIC_64: u32 = 0xcafe_babf;
    let be32 = |at: usize| -> Result<u32, String> {
        bytes
            .get(at..at + 4)
            .map(|b| u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
            .ok_or_else(|| "truncated universal header".to_owned())
    };
    let be64 = |at: usize| -> Result<u64, String> {
        bytes
            .get(at..at + 8)
            .map(|b| u64::from_be_bytes([b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]]))
            .ok_or_else(|| "truncated universal header".to_owned())
    };
    let magic = be32(0)?;
    let wide = match magic {
        FAT_MAGIC => false,
        FAT_MAGIC_64 => true,
        _ => return Ok(bytes),
    };
    let count = be32(4)? as usize;
    let entry_size = if wide { 32 } else { 20 };
    for index in 0..count {
        let at = 8 + index * entry_size;
        if be32(at)? != arch.cpu_type() {
            continue;
        }
        let (offset, size) = if wide {
            (be64(at + 8)?, be64(at + 16)?)
        } else {
            (u64::from(be32(at + 8)?), u64::from(be32(at + 12)?))
        };
        let start = usize::try_from(offset).map_err(|e| e.to_string())?;
        let end = start + usize::try_from(size).map_err(|e| e.to_string())?;
        return bytes
            .get(start..end)
            .ok_or_else(|| "universal slice is out of bounds".to_owned());
    }
    Err("the universal archive has no slice for the target".to_owned())
}

/// Extract the entries under `prefix/` of a zip into `dest`.
fn extract_zip(bytes: &[u8], prefix: &str, dest: &Path) {
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes))
        .unwrap_or_else(|error| panic!("failed to read zip: {error}"));
    let prefix = Path::new(prefix);
    let mut found = false;
    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .unwrap_or_else(|error| panic!("failed to read zip entry: {error}"));
        let Some(path) = entry.enclosed_name() else {
            panic!("zip entry {:?} escapes the archive", entry.name());
        };
        if !path.starts_with(prefix) {
            continue;
        }
        found = true;
        let out = dest.join(&path);
        if entry.is_dir() {
            fs::create_dir_all(&out)
                .unwrap_or_else(|error| panic!("failed to create {}: {error}", out.display()));
            continue;
        }
        if let Some(parent) = out.parent() {
            fs::create_dir_all(parent)
                .unwrap_or_else(|error| panic!("failed to create {}: {error}", parent.display()));
        }
        let mut data = Vec::new();
        entry
            .read_to_end(&mut data)
            .unwrap_or_else(|error| panic!("failed to extract {}: {error}", path.display()));
        fs::write(&out, data)
            .unwrap_or_else(|error| panic!("failed to write {}: {error}", out.display()));
    }
    assert!(found, "the zip has no {}", prefix.display());
}

/// Read a release file from `GHOSTTY_RELEASE_DIR`, or download it from
/// `GHOSTTY_RELEASE_URL` or the release URL, and check its sha256.
fn release_bytes(release: &Release, file: &ReleaseFile) -> Vec<u8> {
    verified_bytes(
        file,
        source::release_origin(&overrides(), &release.url, &file.name),
    )
}

/// Read or download `file` from `origin`, and check its sha256.
fn verified_bytes(file: &ReleaseFile, origin: Origin) -> Vec<u8> {
    let (bytes, origin) = match origin {
        Origin::Path(path) => match fs::read(&path) {
            Ok(bytes) => (bytes, path),
            Err(error) => panic!("{}", release_failure(file, &path, &error.to_string())),
        },
        Origin::Url(url) => match fetch(&url) {
            Ok(bytes) => (bytes, url),
            Err(error) => panic!("{}", release_failure(file, &url, &error)),
        },
    };
    let actual = format!("{:x}", Sha256::digest(&bytes));
    if actual != file.sha256 {
        panic!(
            "{}",
            release_failure(file, &origin, &format!("sha256 mismatch: got {actual}"))
        );
    }
    bytes
}

fn release_failure(file: &ReleaseFile, origin: &str, error: &str) -> String {
    format!(
        "\n\
         ghostty-vt-sys could not obtain the libghostty-vt release file {name}.\n\
         \n\
         error:    {error}\n\
         from:     {origin}\n\
         sha256:   {sha}\n\
         \n\
         Without network access, download the file and set GHOSTTY_RELEASE_DIR to\n\
         the directory that holds it, or set GHOSTTY_RELEASE_URL to a mirror of the\n\
         release. GHOSTTY_VT_FROM_SOURCE=1 builds from source with Zig instead.\n",
        name = file.name,
        sha = file.sha256,
    )
}

/// Copy a source tree into `$OUT_DIR/src` and build it.
fn build_from(source: &Path, lock: &Lock, target: &str) {
    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR"));
    let src = out_dir.join("src");
    if src.exists() {
        fs::remove_dir_all(&src)
            .unwrap_or_else(|error| panic!("failed to remove {}: {error}", src.display()));
    }
    copy_tree(source, &src);
    build_tree(&src, lock, target);
}

/// Copy a Ghostty tree without git metadata and Zig outputs.
fn copy_tree(from: &Path, to: &Path) {
    fs::create_dir_all(to)
        .unwrap_or_else(|error| panic!("failed to create {}: {error}", to.display()));
    let entries = fs::read_dir(from)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", from.display()));
    for entry in entries {
        let entry =
            entry.unwrap_or_else(|error| panic!("failed to read {}: {error}", from.display()));
        let name = entry.file_name();
        if matches!(
            name.to_str(),
            Some(".git" | ".zig-cache" | "zig-cache" | "zig-out" | "zig-pkg")
        ) {
            continue;
        }
        let path = entry.path();
        let dest = to.join(&name);
        let kind = entry
            .file_type()
            .unwrap_or_else(|error| panic!("failed to stat {}: {error}", path.display()));
        if kind.is_dir() {
            copy_tree(&path, &dest);
        } else if kind.is_file() {
            fs::copy(&path, &dest).unwrap_or_else(|error| {
                panic!(
                    "failed to copy {} to {}: {error}",
                    path.display(),
                    dest.display()
                )
            });
        }
    }
}

/// Download the pinned tarball, verify it and extract it into `src`.
fn download_and_extract(lock: &Lock, src: &Path) {
    let url = env::var("GHOSTTY_SOURCE_URL").unwrap_or_else(|_| lock.url.clone());
    let stamp = src.join(".ghostty-vt-sha256");
    if fs::read_to_string(&stamp).is_ok_and(|s| s.trim() == lock.sha256)
        && src.join("build.zig").exists()
    {
        return;
    }
    if src.exists() {
        fs::remove_dir_all(src)
            .unwrap_or_else(|error| panic!("failed to remove {}: {error}", src.display()));
    }

    let bytes = match &lock.release {
        // A release tarball may also come from GHOSTTY_RELEASE_DIR.
        Some(release) if env::var_os("GHOSTTY_SOURCE_URL").is_none() => release_bytes(
            release,
            &ReleaseFile {
                name: lock.url.rsplit('/').next().unwrap_or_default().to_owned(),
                sha256: lock.sha256.clone(),
            },
        ),
        _ => {
            let bytes = match fetch(&url) {
                Ok(bytes) => bytes,
                Err(error) => panic!("{}", download_failure(lock, &url, &error)),
            };
            let actual = format!("{:x}", Sha256::digest(&bytes));
            if actual != lock.sha256 {
                panic!(
                    "{}",
                    download_failure(lock, &url, &format!("sha256 mismatch: got {actual}"))
                );
            }
            bytes
        }
    };

    untar(&bytes, Path::new(&lock.root), src);
    fs::write(&stamp, &lock.sha256)
        .unwrap_or_else(|error| panic!("failed to write {}: {error}", stamp.display()));
}

/// Extract a gzip tarball whose entries are all under `root` into `dest`.
fn untar(bytes: &[u8], root: &Path, dest: &Path) {
    let decoder = flate2::read::GzDecoder::new(bytes);
    let mut archive = tar::Archive::new(decoder);
    let entries = archive
        .entries()
        .unwrap_or_else(|error| panic!("failed to read tarball: {error}"));
    fs::create_dir_all(dest)
        .unwrap_or_else(|error| panic!("failed to create {}: {error}", dest.display()));
    for entry in entries {
        let mut entry =
            entry.unwrap_or_else(|error| panic!("failed to read tarball entry: {error}"));
        // git archive writes a pax global header carrying the commit id.
        let kind = entry.header().entry_type();
        if matches!(
            kind,
            tar::EntryType::XGlobalHeader | tar::EntryType::XHeader
        ) {
            continue;
        }
        let path = entry
            .path()
            .unwrap_or_else(|error| panic!("failed to read tarball entry path: {error}"))
            .into_owned();
        let Ok(rel) = path.strip_prefix(root) else {
            panic!(
                "tarball entry {} is outside the expected root {}",
                path.display(),
                root.display()
            );
        };
        if rel.as_os_str().is_empty() {
            continue;
        }
        let out = dest.join(rel);
        entry
            .unpack(&out)
            .unwrap_or_else(|error| panic!("failed to extract {}: {error}", out.display()));
    }
}

/// The Zig package directory for `zig build --system`: the release's
/// dependency tarball, extracted into `$OUT_DIR/deps`.
fn release_deps(release: &Release) -> PathBuf {
    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR"));
    let deps = out_dir.join("deps");
    let stamp = deps.join(".ghostty-vt-sha256");
    if fs::read_to_string(&stamp).is_ok_and(|s| s.trim() == release.deps.sha256) {
        return deps;
    }
    if deps.exists() {
        fs::remove_dir_all(&deps)
            .unwrap_or_else(|error| panic!("failed to remove {}: {error}", deps.display()));
    }
    let bytes = release_bytes(release, &release.deps);
    untar(&bytes, Path::new(&release.deps_root), &deps);
    fs::write(&stamp, &release.deps.sha256)
        .unwrap_or_else(|error| panic!("failed to write {}: {error}", stamp.display()));
    deps
}

fn fetch(url: &str) -> Result<Vec<u8>, String> {
    use ureq::tls::{RootCerts, TlsConfig, TlsProvider};
    // Cargo.toml picks the TLS backend per build host: the OS stack on macOS
    // and Windows, rustls elsewhere. ureq still has to be told which one.
    // The OS stack also trusts the OS roots: ureq's default puts Mozilla's
    // roots in their place, and Windows schannel cannot anchor a chain in
    // roots added that way ("unable to find any user-specified roots").
    let tls = if cfg!(any(target_os = "macos", windows)) {
        TlsConfig::builder()
            .provider(TlsProvider::NativeTls)
            .root_certs(RootCerts::PlatformVerifier)
    } else {
        TlsConfig::builder().provider(TlsProvider::Rustls)
    };
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .tls_config(tls.build())
        // A stalled server must fail the build, not hang it.
        .timeout_connect(Some(Duration::from_secs(30)))
        .timeout_recv_response(Some(Duration::from_secs(60)))
        .timeout_recv_body(Some(Duration::from_secs(600)))
        .build()
        .into();
    let response = agent.get(url).call().map_err(|error| error.to_string())?;
    let mut bytes = Vec::new();
    response
        .into_body()
        .with_config()
        .limit(256 * 1024 * 1024)
        .reader()
        .read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    Ok(bytes)
}

fn download_failure(lock: &Lock, url: &str, error: &str) -> String {
    format!(
        "\n\
         ghostty-vt-sys could not obtain the Ghostty source tarball.\n\
         \n\
         error:    {error}\n\
         url:      {url}\n\
         sha256:   {sha}\n\
         commit:   {commit}\n\
         \n\
         Without network access, point GHOSTTY_SOURCE_DIR at a Ghostty checkout or\n\
         an extracted libghostty-vt source tarball at commit {commit}, or set\n\
         GHOSTTY_SOURCE_URL to a mirror of the tarball. The tarball is reproducible\n\
         from a Ghostty checkout with:\n\
         \n\
         \x20 printf '{version}' > VERSION\n\
         \x20 git archive --format=tgz --prefix={root}/ --add-file=VERSION \\\n\
         \x20   --prefix={root}/ -o libghostty-vt-source.tar.gz {commit} \\\n\
         \x20   $(for d in images macos dist/doxygen dist/linux dist/macos dist/windows \\\n\
         \x20     flatpak snap po example test src/font/res src/crash/testdata \\\n\
         \x20     pkg/wuffs/src/too_big.jpg pkg/wuffs/src/too_big.png pkg/breakpad/vendor \\\n\
         \x20     vendor; do printf ':(exclude)%s ' $d; done)\n",
        sha = lock.sha256,
        commit = lock.commit,
        version = lock.version,
        root = lock.root,
    )
}

/// Run `zig build` in `src` and emit the link lines.
fn build_tree(src: &Path, lock: &Lock, target: &str) {
    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR"));
    let zig = resolve_zig();

    let read_version = |file: &str| {
        fs::read_to_string(src.join(file))
            .map(|v| v.trim().to_owned())
            .unwrap_or_else(|_| lock.version.clone())
    };
    let prefix = out_dir.join("zig-out");
    let cache = out_dir.join("zig-cache");
    let cpu = env::var("GHOSTTY_VT_CPU").unwrap_or_else(|_| "baseline".to_owned());

    let mut build = Command::new(&zig);
    build
        .arg("build")
        .arg("-Demit-lib-vt")
        .arg("-Doptimize=ReleaseFast")
        .arg("-Dsimd=true")
        .arg(format!("-Dtarget={}", zig_target(target)))
        .arg(format!("-Dcpu={cpu}"))
        // The copy has no git metadata, and Zig must not read the version
        // from a repository around OUT_DIR.
        .arg(format!("-Dversion-string={}", read_version("VERSION")))
        .arg("-Demit-xcframework=false")
        .arg("-Demit-themes=false")
        .arg("--prefix")
        .arg(&prefix)
        .arg("--cache-dir")
        .arg(&cache)
        .current_dir(src);
    if lock.release.is_some() {
        build.arg(format!(
            "-Dlib-version-string={}",
            read_version("LIB_VERSION")
        ));
    }

    let system = match env::var_os("GHOSTTY_ZIG_SYSTEM_DIR").map(PathBuf::from) {
        Some(dir) => {
            assert!(
                dir.is_dir(),
                "GHOSTTY_ZIG_SYSTEM_DIR is not a directory: {}",
                dir.display()
            );
            Some(dir)
        }
        None => lock.release.as_ref().map(release_deps),
    };
    if let Some(dir) = system {
        // With --system Zig fetches nothing. Its global cache must not hold
        // package archives from other builds, so it gets its own.
        build
            .arg("--system")
            .arg(&dir)
            .env("ZIG_GLOBAL_CACHE_DIR", out_dir.join("zig-global"));
    }

    run(build, "zig build");

    let lib_dir = prefix.join("lib");
    // On Windows `ghostty-vt.lib` is the DLL's import library and the static
    // library is `ghostty-vt-static.lib`.
    let windows = target.contains("windows");
    let archive = if windows {
        lib_dir.join("ghostty-vt-static.lib")
    } else {
        lib_dir.join("libghostty-vt.a")
    };
    assert!(
        archive.exists(),
        "zig build did not produce {}",
        archive.display()
    );
    let include_dir = prefix.join("include");
    assert!(
        include_dir.join("ghostty").join("vt.h").exists(),
        "zig build did not install headers in {}",
        include_dir.display()
    );

    if windows {
        link_windows_static(target, &lib_dir);
    } else {
        println!("cargo:rustc-link-search=native={}", lib_dir.display());
        println!("cargo:rustc-link-lib=static=ghostty-vt");
    }
    println!("cargo:include={}", include_dir.display());
}

/// Link `ghostty-vt-static.lib` from `lib_dir`. The static library imports
/// from ntdll.
///
/// The GNU-ABI targets link a copy with symbols of Zig's compiler_rt
/// renamed (see `build/compiler_rt.rs`): its weak defaults, which clash
/// with the compiler_rt Zig's own linker adds, and for `*-windows-gnu`,
/// which links with GNU ld, its weak stack probes. lld-link and link.exe
/// take the library unchanged.
fn link_windows_static(target: &str, lib_dir: &Path) {
    const ARCHIVE: &str = "ghostty-vt-static.lib";
    if target.ends_with("windows-msvc") {
        println!("cargo:rustc-link-search=native={}", lib_dir.display());
        println!("cargo:rustc-link-lib=static=ghostty-vt-static");
    } else {
        let source = lib_dir.join(ARCHIVE);
        let mut bytes = fs::read(&source)
            .unwrap_or_else(|error| panic!("failed to read {}: {error}", source.display()));
        compiler_rt::patch(&mut bytes, target.ends_with("windows-gnu"))
            .unwrap_or_else(|error| panic!("{}: {error}", source.display()));
        let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR"));
        let dir = out_dir.join("windows-gnu");
        fs::create_dir_all(&dir)
            .unwrap_or_else(|error| panic!("failed to create {}: {error}", dir.display()));
        let patched = dir.join(ARCHIVE);
        fs::write(&patched, &bytes)
            .unwrap_or_else(|error| panic!("failed to write {}: {error}", patched.display()));
        println!("cargo:rustc-link-search=native={}", dir.display());
        // The GNU linkers look for `lib<name>.a`, so name the file itself.
        println!("cargo:rustc-link-lib=static:+verbatim={ARCHIVE}");
    }
    println!("cargo:rustc-link-lib=dylib=ntdll");
}

fn resolve_zig() -> PathBuf {
    if let Some(zig) = env::var_os("ZIG").map(PathBuf::from) {
        check_zig_version(&zig);
        return zig;
    }

    let path_zig = PathBuf::from("zig");
    let path_result = zig_version(&path_zig);
    if matches!(&path_result, Ok(version) if version.starts_with("0.17.")) {
        return path_zig;
    }

    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    if let Ok(output) = Command::new("mise")
        .args(["which", "zig"])
        .current_dir(manifest_dir)
        .output()
        && output.status.success()
    {
        let zig = PathBuf::from(String::from_utf8_lossy(&output.stdout).trim());
        if !zig.as_os_str().is_empty() {
            check_zig_version(&zig);
            return zig;
        }
    }

    match path_result {
        Ok(version) => panic!(
            "ghostty-vt-sys needs Zig 0.17.x, found `{version}` at `zig`.\n\
             Install it with `mise install`, activate mise, or set ZIG to the Zig 0.17 binary."
        ),
        Err(error) => panic!(
            "ghostty-vt-sys could not find Zig 0.17.x: {error}\n\
             Install it with `mise install`, activate mise, or set ZIG to the Zig 0.17 binary."
        ),
    }
}

fn check_zig_version(zig: &Path) {
    let version = zig_version(zig).unwrap_or_else(|error| panic!("{error}"));
    assert!(
        version.starts_with("0.17."),
        "ghostty-vt-sys needs Zig 0.17.x, found `{version}` at `{}`.\n\
         Install it with `mise install`, activate mise, or set ZIG to the Zig 0.17 binary.",
        zig.display()
    );
}

fn zig_version(zig: &Path) -> Result<String, String> {
    let output = Command::new(zig)
        .arg("version")
        .output()
        .map_err(|error| format!("failed to run `{} version`: {error}", zig.display()))?;
    let version = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    if !output.status.success() {
        return Err(format!(
            "`{} version` failed with status {}",
            zig.display(),
            output.status
        ));
    }
    Ok(version)
}

fn run(mut command: Command, context: &str) {
    let status = command
        .status()
        .unwrap_or_else(|error| panic!("failed to execute {context}: {error}"));
    assert!(status.success(), "{context} failed with status {status}");
}

fn zig_target(target: &str) -> &'static str {
    match target {
        "x86_64-unknown-linux-gnu" => "x86_64-linux-gnu",
        "x86_64-unknown-linux-musl" => "x86_64-linux-musl",
        "aarch64-unknown-linux-gnu" => "aarch64-linux-gnu",
        "aarch64-unknown-linux-musl" => "aarch64-linux-musl",
        "x86_64-apple-darwin" => "x86_64-macos",
        "aarch64-apple-darwin" => "aarch64-macos",
        "x86_64-pc-windows-msvc" => "x86_64-windows-msvc",
        "aarch64-pc-windows-msvc" => "aarch64-windows-msvc",
        "x86_64-pc-windows-gnu" | "x86_64-pc-windows-gnullvm" => "x86_64-windows-gnu",
        "aarch64-pc-windows-gnullvm" => "aarch64-windows-gnu",
        "aarch64-apple-ios" => "aarch64-ios",
        "aarch64-apple-ios-sim" => "aarch64-ios-simulator",
        other => panic!(
            "ghostty-vt-sys has no Zig target for `{other}`; supported targets are \
             x86_64 and aarch64 linux-gnu, linux-musl, macos, windows-msvc and \
             windows-gnu, plus aarch64-apple-ios and aarch64-apple-ios-sim"
        ),
    }
}

#[cfg(feature = "pkg-config")]
fn try_pkg_config(target: &str) -> bool {
    let Ok(lib) = pkg_config::Config::new()
        .statik(true)
        .cargo_metadata(false)
        .probe("libghostty-vt-static")
    else {
        return false;
    };
    for path in &lib.link_paths {
        println!("cargo:rustc-link-search=native={}", path.display());
    }
    for path in &lib.framework_paths {
        println!("cargo:rustc-link-search=framework={}", path.display());
    }
    for framework in &lib.frameworks {
        println!("cargo:rustc-link-lib=framework={framework}");
    }
    if target.contains("windows") {
        let lib_dir = lib
            .link_paths
            .iter()
            .find(|dir| dir.join("ghostty-vt-static.lib").exists())
            .expect("pkg-config gave no directory with ghostty-vt-static.lib");
        link_windows_static(target, lib_dir);
    } else {
        println!("cargo:rustc-link-lib=static=ghostty-vt");
    }
    for library in &lib.libs {
        if library != "ghostty-vt" && library != "ghostty-vt-static" {
            println!("cargo:rustc-link-lib={library}");
        }
    }
    if !lib.include_paths.is_empty() {
        let joined = env::join_paths(&lib.include_paths).expect("include paths");
        println!("cargo:include={}", joined.to_string_lossy());
    }
    true
}
