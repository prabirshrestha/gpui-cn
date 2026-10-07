# Third-party notices

The crates in this repository are Apache-2.0 (see `../LICENSE-APACHE`). The
terminal engine crates and the `ghostty` feature of gpui-cn ship material
derived from, and link native code built from, the projects below.
`cargo deny check licenses` covers the Rust dependencies; the native code
compiled into `libghostty-vt.a` is invisible to it and is listed here.

## Shipped in the crates

| Project | License | What is included |
| --- | --- | --- |
| [Ghostty](https://github.com/ghostty-org/ghostty) | MIT, Copyright (c) 2024 Mitchell Hashimoto and Ghostty contributors | Bindings generated from `include/ghostty/vt/*.h` (with their doc comments), the `xterm-ghostty` terminfo source and its compiled database, and the shell integration scripts under `crates/ghostty-vt/resources/shell-integration`. License text in `LICENSE-GHOSTTY`. |
| [Herdr](https://github.com/herdrdev/herdr) | Apache-2.0 | The terminal in `crates/gpui-cn/src/terminal` (frames, engine, pty, element, selection, links, IME) is derived from Herdr's terminal pane. Every derived file says so in its header. |
| [Muxy v2](https://github.com/muxy-app/muxy) (the Rust 2.x line) | MIT, Copyright (c) 2026 Muxy | Block element quads, decoration quads, the canvas structure and cell-aligned glyph positioning in `crates/gpui-cn/src/terminal`, from `crates/muxy-app/src/views/terminal/element.rs`. Every adapted file says so in its header. License text in `LICENSE-MUXY`. |
| [libghostty-rs](https://github.com/Uzaaft/libghostty-rs) (`libghostty-vt`, `libghostty-vt-sys` 0.2.2) | MIT OR Apache-2.0, by Uzaaft and pluiedev | The safe crate is a fork; the sys crate's build script, `lib.rs` and bindgen tool are adapted from it. Every adapted file says "Adapted from libghostty-vt 0.2.2" or "Adapted from libghostty-vt-sys 0.2.2" in its header. The crates ship no license text of their own; the repository above has it. Used here under Apache-2.0. |

## Linked into the static archive

`ghostty-vt-sys` builds `libghostty-vt` from Ghostty's official source
tarball. The archive contains Ghostty's own Zig code (MIT) and these
libraries, fetched by Zig from the sources named in Ghostty's
`build.zig.zon` files and verified against the hashes there:

| Project | License | Role |
| --- | --- | --- |
| [simdutf](https://github.com/simdutf/simdutf) | Apache-2.0 OR MIT | SIMD UTF-8 validation and decoding (`pkg/simdutf`). |
| [Highway](https://github.com/google/highway) | Apache-2.0 OR BSD-3-Clause | SIMD dispatch used by the VT parser. |
| [Wuffs](https://github.com/google/wuffs) | Apache-2.0 (with MIT for the generated code) | PNG and image decoding for Kitty graphics. |
| [pixels](https://github.com/make-github-pseudonymous-again/pixels) | CC0-1.0 | Image test data used by the Wuffs package. |
| [zlib](https://zlib.net) | Zlib | Compressed Kitty graphics payloads. |
| [aro](https://github.com/Vexu/arocc) | MIT | C parser behind translate-c. |
| [translate-c](https://codeberg.org/vancluever/translate-c) | MIT | Zig's C translation used to build the C libraries. |
| [uucode](https://github.com/jacobsandlund/uucode) | MIT | Unicode tables (grapheme clustering, width). |

## Build-time Rust dependencies

`ghostty-vt-sys` downloads the source tarball with `ureq`. On Linux hosts
that pulls in `webpki-roots`, whose root certificate data is under
CDLA-Permissive-2.0; `deny.toml` carries an exception for it.
