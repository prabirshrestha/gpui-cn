# ghostty-vt-sys

Raw FFI bindings for [libghostty-vt](https://ghostty.org), the terminal
emulation library extracted from the Ghostty terminal emulator.

This crate is currently developed primarily for
[`gpui-cn`](https://github.com/prabirshrestha/gpui-cn).

The crate version normally follows gpui-cn and records its libghostty-vt
release as build metadata. For example, `0.8.0+ghostty-vt.1.0.1` uses the
libghostty-vt `vt-1.0.1` release.

On macOS, iOS and Windows GNU, the build script downloads the release's
prebuilt library, checks its sha256 against `GHOSTTY.lock`, and links it.
These targets need no Zig. On the other targets it builds the release
source tarball with Zig 0.17, and resolves Zig packages from the release's
deps tarball with `zig build --system`, so the build fetches nothing from
the package hosts. `src/bindings.rs` is generated from the pinned headers
and committed, so the crate needs no bindgen or Clang to build.

| Target | Library |
| --- | --- |
| `x86_64-apple-darwin`, `aarch64-apple-darwin` | Prebuilt static, from the XCFramework. |
| `aarch64-apple-ios`, `aarch64-apple-ios-sim` | Prebuilt static, from the XCFramework. |
| `x86_64-pc-windows-gnu`, `*-pc-windows-gnullvm` | Prebuilt static, from the Windows zip. |
| `*-pc-windows-msvc` with `GHOSTTY_VT_PREBUILT_MSVC=1` | Prebuilt static, from the Windows zip. Needs lld. |
| Linux, Windows MSVC | Built from source with Zig, linked statically. |

Zig's compiler_rt in the static library exports `___chkstk_ms` and the
other stack probes as COFF weak externals. GNU ld takes them as
definitions, so it does not pull the probe from libgcc, and with
`--gc-sections`, which rustc always passes, it then drops the weak
default. Zig's own linker (`zig cc`, as cargo-zigbuild uses) adds a
second Zig compiler_rt, whose `.weak.<name>.default` symbols clash with
the ones in the library. So the build script links a copy of the library
with these symbols renamed: the weak defaults on every GNU-ABI target,
and the weak stack probes for `x86_64-pc-windows-gnu`, so the
toolchain's own probe is used. It links with GNU ld (MinGW GCC), lld and
`zig cc`. No DLL is linked, and an application needs no extra link flags.

The Windows zip is a GNU-ABI build. Microsoft `link.exe` rejects its
COMDAT sections (LNK1143), so the MSVC targets build from source by
default, which needs Zig and the Windows SDK on a Windows host. lld
(`lld-link`) links the zip into an MSVC program with no change. To use it,
for example to cross-compile from macOS or Linux, set
`GHOSTTY_VT_PREBUILT_MSVC=1` and link with lld: use
[cargo-xwin](https://github.com/rust-cross/cargo-xwin), or set
`CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_LINKER=rust-lld` (or `lld-link`).
The build stops with an error when the linker is `link.exe`, which is
rustc's default for these targets.

Most users want the safe crate, [`ghostty-vt`](https://crates.io/crates/ghostty-vt).

## Requirements

- For a source build only, Zig 0.17.x on `PATH` or in `ZIG`. In this
  repository, the build also finds the version from `mise.toml` through
  `mise`.
- On macOS, the Xcode command line tools.
- Network on the first build, or `GHOSTTY_RELEASE_DIR` for offline builds.

## Environment variables

| Variable | Effect |
| --- | --- |
| `GHOSTTY_RELEASE_DIR` | Read the release files (prebuilt archives, source and deps tarballs) from this directory instead of downloading them. Each must match its sha256 in `GHOSTTY.lock`. |
| `GHOSTTY_RELEASE_URL` | Download the release files from this mirror instead of `https://release.files.ghostty.org/libghostty-vt/<version>`. It holds the same file names, and each must match its sha256 in `GHOSTTY.lock`. |
| `GHOSTTY_VT_PREBUILT_URL` | Download this archive instead of the target's prebuilt release file. It has the same layout: the XCFramework zip for the Apple targets, the Windows zip (`ghostty-vt-<version>-<arch>-windows-gnu/lib/ghostty-vt-static.lib` and `include/`) for Windows. Only for a target that links a prebuilt library. Needs `GHOSTTY_VT_PREBUILT_SHA256`. |
| `GHOSTTY_VT_PREBUILT_SHA256` | The sha256 of the `GHOSTTY_VT_PREBUILT_URL` archive. Required with it. |
| `GHOSTTY_VT_LIB_DIR` | Link the static library in this directory, on every target, with no download and no Zig: `libghostty-vt.a`, or `ghostty-vt-static.lib` on Windows. It must match the C ABI of the pinned release; the build prints a warning that says so. A universal Apple archive and a Windows GNU library are adjusted in a copy, as for the release files. |
| `GHOSTTY_VT_LIB_SHA256` | Check the `GHOSTTY_VT_LIB_DIR` library against this sha256. |
| `GHOSTTY_VT_INCLUDE_DIR` | The headers of the `GHOSTTY_VT_LIB_DIR` library, for dependent build scripts. Default: `../include` beside the library, if it holds `ghostty/vt.h`. |
| `GHOSTTY_VT_FROM_SOURCE` | Set to `1` to build from source with Zig on a target that has a prebuilt library. |
| `GHOSTTY_VT_PREBUILT_MSVC` | Set to `1` to link the prebuilt Windows zip on `*-pc-windows-msvc` instead of building from source. Needs lld as the linker. |
| `GHOSTTY_SOURCE_DIR` | Build from this Ghostty source tree with Zig, on every target. |
| `GHOSTTY_SOURCE_URL` | Download the source tarball from this mirror. It must match the pinned sha256 in `GHOSTTY.lock`. |
| `GHOSTTY_ZIG_SYSTEM_DIR` | Resolve Zig packages from this directory (`zig build --system`) instead of the release deps tarball. |
| `GHOSTTY_VT_CPU` | Zig CPU model, default `baseline`. |
| `ZIG` | Path to the Zig binary. |
| `DOCS_RS` | Skip the native build. |

The library comes from the first of these that is set:
`GHOSTTY_SOURCE_DIR`, `GHOSTTY_VT_FROM_SOURCE`, `GHOSTTY_VT_LIB_DIR`,
`GHOSTTY_VT_PREBUILT_URL`, then the pinned release. The release files come
from `GHOSTTY_RELEASE_DIR`, then `GHOSTTY_RELEASE_URL`, then the release
server. An override that a higher one hides gives a build warning.

## Features

- `pkg-config`: on a source-build target, use an installed
  `libghostty-vt-static` found through pkg-config before downloading. It
  must be the pinned release.
- `bindgen-tool`: builds the `gen-bindings` binary that regenerates
  `src/bindings.rs`.

## Targets

x86_64 and aarch64 for Linux (glibc, musl), macOS and Windows (MSVC built
natively on Windows, GNU and GNU LLVM), plus `aarch64-apple-ios` and
`aarch64-apple-ios-sim`. macOS is the only supported runtime in this
release; the others build in CI.

## License

Apache-2.0. Ghostty is MIT (see `LICENSE-GHOSTTY`). Adapted from
libghostty-vt-sys 0.2.2 (MIT OR Apache-2.0).
