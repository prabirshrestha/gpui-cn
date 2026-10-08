---
name: update-ghostty
description: Update gpui-cn's pinned Ghostty source, generated bindings, terminal resources, and the SHA-bearing versions of ghostty-vt-sys and ghostty-vt. Use for requests to update, sync, repin, or bump Ghostty or libghostty-vt.
---

# Update Ghostty

Use `scripts/sync.sh` as the only way to move the Ghostty pin. Never patch the
Ghostty submodule or copy individual generated files.

## Choose the versions

Require a full 40-character Ghostty commit. Read the Ghostty version from the
official source tarball through `scripts/sync.sh`. Do not infer it from a branch
or tag.

Both engine crates use one exact package version:

```text
<crate-core>+ghostty.<ghostty-version>.<short-commit>
```

For example, Ghostty `1.3.2-main+b40acce` and crate core `0.6.0` produce:

```text
0.6.0+ghostty.1.3.2-main.b40acce
```

The crate core defaults to `workspace.package.version`, which is gpui-cn's
version. Pass a different core only when the user requests an independent
engine release or that core was already published with another Ghostty pin.
The script records this choice in `workspace.metadata.ghostty.version-mode`,
and the deterministic checker enforces it. Never publish two versions that
differ only in build metadata. Cargo ignores build metadata during version
selection.

## Sync

Start from a clean working tree. On macOS, put Zig 0.16 first on `PATH` and
ensure Homebrew `llvm@22` is installed.

Use the gpui-cn core:

```sh
scripts/sync.sh <full-ghostty-commit>
```

Use an independent engine core:

```sh
scripts/sync.sh <full-ghostty-commit> <major.minor.patch>
```

The script updates the submodule, both lock files, both engine crate versions,
their root dependency requirements, generated bindings, terminfo, and shell
integration resources. It verifies the official tarball against a reproduced
archive.

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
