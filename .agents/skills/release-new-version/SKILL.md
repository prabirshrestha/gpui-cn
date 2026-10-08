---
name: release-new-version
description: Release a new version of gpui-cn by updating main, checking CI, applying a semantic version bump, validating the workspace and package, pushing a release commit, then tagging for crates.io publication through GitHub Actions. Use for requests to release, version, tag, or publish gpui-cn.
---

# Release new version

## Preconditions

- Read `AGENTS.md` and the current `.github/workflows/ci.yml` and
  `.github/workflows/release.yml`.
- Work from the repository root and on `main` only.
- Treat a request to release as live unless the user requests a dry run.
  A dry run must not commit, push, create tags, or publish.
- Inspect `git status --short --branch`. Stop if the working tree has changes.
- Pull with `git pull --ff-only` when `main` tracks a remote.
- Publish `ghostty-vt-sys`, `ghostty-vt`, and `gpui-cn`. The gallery has
  `publish = false`.
- Do not publish locally. A version tag starts the `Release` workflow in
  `.github/workflows/release.yml`.
- That workflow uses crates.io Trusted Publishing through GitHub OIDC,
  with `rust-lang/crates-io-auth-action`. No repository token secret or
  GitHub environment is required. Each published crate has its own crates.io
  publisher entry. Every entry must match `prabirshrestha/gpui-cn` and
  workflow filename `release.yml`, with the environment left blank.
- Never print or request a token in chat. If publication fails, diagnose
  the failure before retrying. Do not move or recreate a release tag.

## Check upstream CI

After pulling, inspect recent runs:

```sh
gh run list --workflow ci.yml --branch main --limit 10 --json databaseId,headSha,status,conclusion,displayTitle,url
```

Match the run's `headSha` to `git rev-parse HEAD`. Watch an active run with
`gh run watch <run-id> --exit-status`. Stop on failed CI and report the failure.
Local checks do not replace successful CI for the release commit.

## Bump the version

1. Read `workspace.package.version` from the root `Cargo.toml`.
2. Inspect release tags and the changes since the latest release tag:

   ```sh
   git tag --sort=-version:refname | head
   git log --oneline <latest-tag>..HEAD
   git diff <latest-tag>..HEAD -- crates/gpui-cn crates/ghostty-vt crates/ghostty-vt-sys
   ```

3. Use the version requested by the user. Otherwise, select a SemVer bump from
   the changes, including public API changes. Do not infer a breaking release
   only because the major version is zero. Confirm that the new version is
   greater than the current version and that its local and remote tags do not
   exist.
4. Update `workspace.package.version` and the version of
   `workspace.dependencies.gpui-cn` in the root `Cargo.toml`.
5. Unless the user requests an independent engine version, set the SemVer core
   of `ghostty-vt-sys` and `ghostty-vt` to the new gpui-cn version. Preserve
   the build metadata derived from `crates/ghostty-vt-sys/GHOSTTY.lock`. For
   example, gpui-cn `0.6.0` at Ghostty `1.3.2-main+b40acce` uses
   `0.6.0+ghostty.1.3.2-main.b40acce` for both engine crates. Keep
   `workspace.metadata.ghostty.version-mode` as `follow-gpui-cn`. An
   independent engine release changes it to `independent`.
6. Update `workspace.dependencies.ghostty-vt-sys` and
   `workspace.dependencies.ghostty-vt` to the engine SemVer core without build
   metadata. Run `scripts/check-ghostty-version.sh`. Never edit the metadata to
   move the Ghostty pin. Use the `update-ghostty` skill and `scripts/sync.sh`.
7. Refresh and verify `Cargo.lock`:

   ```sh
   cargo check --workspace --all-targets --all-features
   cargo check --workspace --all-targets --all-features --locked
   ```

8. Inspect the diff. Only the intended version fields in the three published
   crate manifests and root `Cargo.toml`, plus the four workspace package
   versions in `Cargo.lock`, should change. Do not include unrelated dependency
   updates.

## Validate

Run the required repository checks before the release commit:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo test --workspace --all-features --locked
RUSTDOCFLAGS='-D warnings' cargo doc -p gpui-cn -p ghostty-vt -p ghostty-vt-sys --no-deps --all-features --locked
cargo deny check licenses
scripts/check-ghostty-version.sh
```

Use the repository's Rust toolchain. Follow `AGENTS.md` for benchmark, and snapshot checks when the release includes the relevant changes.

For a dry run, validate the package without committing:

```sh
cargo publish --workspace --dry-run --locked --allow-dirty
```

Use `--allow-dirty` only after confirming that the diff contains just the
intended version changes. Leave those changes uncommitted and report them.

## Commit and publish

1. Stage only the intended version files and `Cargo.lock`. Commit as
   `chore: release vX.Y.Z`, with a body that states what changed and why.
2. Validate the committed packages:

   ```sh
   cargo publish --workspace --dry-run --locked
   ```

3. Push the commit with `git push origin HEAD`.
4. Find the `CI` run in `ci.yml` whose `headSha` matches the pushed commit.
   Wait for it to complete successfully:

   ```sh
   gh run watch <run-id> --exit-status
   ```

5. Create an annotated tag: `git tag -a vX.Y.Z -m "vX.Y.Z"`.
6. Push only that tag: `git push origin vX.Y.Z`.
7. Find the tag's `Release` run in `release.yml`. Confirm its commit SHA and
   tag, then watch it with `gh run watch <release-run-id> --exit-status`.

The release workflow accepts `v*` tags but requires the exact value
`v<gpui-cn package version>`. This includes valid SemVer prereleases.

Do not push a tag before the release commit CI succeeds. If a workflow cannot
be found or its result is unclear, stop before the next publication step and
report the pending state. Do not report publication until the `Release`
workflow's publish step succeeds.

## Recover an existing release

Do not bump the version again to retry an unpublished tag. Confirm that
the version is not already published and that the tag's commit passed CI.
If only external settings changed, rerun the failed Release run.

If the workflow itself changed, a rerun still uses the old workflow.
Commit and push the workflow fix on `main`, wait for its CI to pass, then
dispatch the updated workflow with the unchanged release tag:

```sh
gh workflow run release.yml --ref main -f tag=vX.Y.Z
```

Find the new `workflow_dispatch` Release run and check its workflow commit
against `main`. Its `headSha` is the workflow commit, not the release tag's
commit. Check the tag and resolved SHA in the "Check the release commit CI"
step before reporting publication. The workflow checks that the tag is on
main, has successful CI, and matches the package version.

## Report

Report the old and new versions, validation results, release commit, tag,
commit CI result, release workflow result, and crates.io publication result.
For a dry run, report the local changes and state that nothing was published.
