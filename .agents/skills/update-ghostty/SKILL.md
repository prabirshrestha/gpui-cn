---
name: update-ghostty
description: Update gpui-cn's pinned Ghostty source, generated bindings, terminal resources, and the SHA-bearing versions of ghostty-vt-sys and ghostty-vt. Use for requests to update, sync, repin, or bump Ghostty or libghostty-vt.
---

# Update Ghostty

Use `scripts/sync.sh` as the only way to move the Ghostty pin. Never patch the
Ghostty submodule or copy individual generated files.

## Choose the versions

Pin an official libghostty-vt release, a `vt-X.Y.Z` tag with files under
`https://release.files.ghostty.org/libghostty-vt/X.Y.Z/`. Pin a bare
40-character Ghostty commit only when the user asks for an unreleased
commit; that tip mode has no prebuilt libraries, so every target then
needs Zig.

Both engine crates use one exact package version:

```text
<crate-core>+ghostty-vt.<release-version>
```

For example, release `vt-1.0.1` and crate core `0.8.0` produce:

```text
0.8.0+ghostty-vt.1.0.1
```

A tip pin uses `<crate-core>+ghostty.<ghostty-version>.<short-commit>`.

The crate core defaults to `workspace.package.version`, which is gpui-cn's
version. Pass a different core only when the user requests an independent
engine release or that core was already published with another Ghostty pin.
The script records this choice in `workspace.metadata.ghostty.version-mode`,
and the deterministic checker enforces it. Never publish two versions that
differ only in build metadata. Cargo ignores build metadata during version
selection.

## Sync

Start from a clean working tree. Install `minisign` and Homebrew
`llvm@22`; a release sync needs no Zig on macOS, because the verify build
links the prebuilt library. If you install minisign only for the sync,
remove it afterwards.

Use the gpui-cn core:

```sh
scripts/sync.sh vt-<version>
```

Use an independent engine core:

```sh
scripts/sync.sh vt-<version> <major.minor.patch>
```

For a release, the script checks `SHA256SUMS` against its minisign
signature, checks every release file against `SHA256SUMS`, and checks that
the tarball headers match the tag in the submodule. It updates the
submodule gitlink, both lock files with every release file and its sha256,
both engine crate versions, their root dependency requirements, generated
bindings, terminfo, and shell integration resources. A tip sync verifies
the official tarball against a reproduced archive instead.

The C API passes many values through `void*` get and set keys, so the
compiler does not catch a changed value type. Read the header diff and
check every key the safe API uses against its documented input or output
type.

Inspect every changed path. Reject unrelated Ghostty source changes and
dependency updates. Run:

```sh
scripts/check-ghostty-version.sh
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo test --workspace --all-features --locked
RUSTDOCFLAGS='-D warnings' cargo doc -p gpui-cn -p ghostty-vt -p ghostty-vt-sys --no-deps --all-features --locked
cargo deny check licenses
cargo publish --workspace --dry-run --locked
```

Do not publish, tag, push, or commit unless the user requests that action.
