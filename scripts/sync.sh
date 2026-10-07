#!/usr/bin/env bash
# Move the Ghostty pin and regenerate everything derived from it.
#
#   scripts/sync.sh [<ghostty-commit>]
#
# Without an argument the current commit from GHOSTTY.lock is resynced.
# Steps: submodule checkout, GHOSTTY.lock (sha256 verified against both the
# published tarball and `git archive`), bindings, terminfo (source plus
# `tic -x` output), shell integration scripts.
#
# Needs: git, curl, shasum, tar, zig 0.16, tic, rsync, cargo, libclang
# (for bindgen). Run on macOS, where the bindings are generated.
set -euo pipefail

root=$(cd "$(dirname "$0")/.." && pwd)
cd "$root"

lock=crates/ghostty-vt-sys/GHOSTTY.lock
current=$(sed -n 's/^commit = //p' "$lock")
commit=${1:-$current}
case "$commit" in
  *[!0-9a-f]* | ?????????????????????????????????????????*) ;;
esac
if [ ${#commit} -ne 40 ]; then
  echo "expected a full 40-character Ghostty commit, got '$commit'" >&2
  exit 1
fi

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

echo "==> submodule third_party/ghostty at $commit"
git submodule update --init third_party/ghostty
git -C third_party/ghostty fetch --quiet origin "$commit"
git -C third_party/ghostty checkout --quiet --detach "$commit"

echo "==> tarball"
url="https://tip.files.ghostty.org/$commit/libghostty-vt-source.tar.gz"
curl -fsSL -o "$work/source.tar.gz" "$url"
sha=$(shasum -a 256 "$work/source.tar.gz" | cut -d' ' -f1)
tarroot=$(tar tzf "$work/source.tar.gz" | head -1 | cut -d/ -f1)
version=${tarroot#libghostty-vt-}
echo "    version $version"
echo "    sha256  $sha"

echo "==> reproduce the tarball with git archive"
printf '%s' "$version" > "$work/VERSION"
excludes=()
while IFS= read -r line; do
  excludes+=(":(exclude)$line")
done < <(awk '/^const lib_vt_excludes/{p=1;next} p&&/^};/{exit} p' \
  third_party/ghostty/src/build/GhosttyDist.zig | sed -n 's/^[[:space:]]*"\([^"]*\)",.*/\1/p')
git -C third_party/ghostty archive --format=tgz "--prefix=$tarroot/" \
  "--add-file=$work/VERSION" "--prefix=$tarroot/" \
  -o "$work/repro.tar.gz" "$commit" "${excludes[@]}"
repro=$(shasum -a 256 "$work/repro.tar.gz" | cut -d' ' -f1)
if [ "$repro" != "$sha" ]; then
  echo "git archive reproduction ($repro) does not match the published tarball ($sha)" >&2
  exit 1
fi
echo "    reproduced"

cat > "$lock" <<EOF
# Pinned Ghostty source for libghostty-vt. Written by scripts/sync.sh.
commit = $commit
version = $version
url = $url
sha256 = $sha
root = $tarroot
EOF

echo "==> bindings"
DOCS_RS=1 cargo run --quiet -p ghostty-vt-sys --features bindgen-tool --bin gen-bindings
cargo fmt -p ghostty-vt-sys 2>/dev/null || true

echo "==> terminfo"
ti="$work/terminfo"
mkdir -p "$ti"
cp third_party/ghostty/src/terminfo/Source.zig third_party/ghostty/src/terminfo/ghostty.zig "$ti/"
cat > "$ti/main.zig" <<'EOF'
const std = @import("std");
const ghostty = @import("ghostty.zig").ghostty;

pub fn main(init: std.process.Init) !void {
    var buffer: [1024]u8 = undefined;
    var stdout_writer = std.Io.File.stdout().writerStreaming(init.io, &buffer);
    const writer = &stdout_writer.interface;
    try ghostty.encode(writer);
    try stdout_writer.end();
}
EOF
(cd "$ti" && zig run main.zig > ghostty.terminfo)
mkdir -p "$ti/db"
tic -x -o "$ti/db" "$ti/ghostty.terminfo"
res=crates/ghostty-vt/resources
rm -rf "$res/terminfo"
mkdir -p "$res/terminfo"
cp "$ti/ghostty.terminfo" "$res/terminfo/"
cp -R "$ti/db/." "$res/terminfo/"
test -f "$res/terminfo/78/xterm-ghostty"
test -f "$res/terminfo/67/ghostty"

echo "==> shell integration"
rm -rf "$res/shell-integration"
mkdir -p "$res/shell-integration"
rsync -a --exclude README.md third_party/ghostty/src/shell-integration/ "$res/shell-integration/"

echo "==> verify"
cargo build -p ghostty-vt-sys
echo "synced to $commit ($version)"
