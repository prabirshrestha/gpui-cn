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
| `x86_64-pc-windows-gnu`, `*-pc-windows-gnullvm` | Prebuilt DLL, through its import library. |
| Linux, Windows MSVC | Built from source with Zig, linked statically. |

On Windows GNU the static library does not link under rustc: GNU ld drops
the default of Zig's weak `___chkstk_ms` with `--gc-sections`, which rustc
always passes. So these targets link `ghostty-vt.dll`. The build script
exposes its path as `DEP_GHOSTTY_VT_DLL` to the build scripts of crates
that depend on this one, and `cargo run` and `cargo test` find it on
`PATH`. An application ships the DLL next to its executable.

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
| `GHOSTTY_VT_FROM_SOURCE` | Set to `1` to build from source with Zig on a target that has a prebuilt library. |
| `GHOSTTY_SOURCE_DIR` | Build from this Ghostty source tree with Zig, on every target. |
| `GHOSTTY_SOURCE_URL` | Download the source tarball from this mirror. It must match the pinned sha256 in `GHOSTTY.lock`. |
| `GHOSTTY_ZIG_SYSTEM_DIR` | Resolve Zig packages from this directory (`zig build --system`) instead of the release deps tarball. |
| `GHOSTTY_VT_CPU` | Zig CPU model, default `baseline`. |
| `ZIG` | Path to the Zig binary. |
| `DOCS_RS` | Skip the native build. |

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
