#!/usr/bin/env bash
# Move the Ghostty pin and regenerate everything derived from it.
#
#   scripts/sync.sh [<ghostty-commit> [<crate-version>]]
#
# Without an argument the current commit from GHOSTTY.lock is resynced.
# A new commit requires the SemVer core for ghostty-vt and ghostty-vt-sys,
# without build metadata. This normally matches gpui-cn. The script appends
# the Ghostty version and short commit as SemVer build metadata.
# Steps: submodule checkout, GHOSTTY.lock (sha256 verified against both the
# published tarball and `git archive`), crate versions, bindings, terminfo
# (source plus `tic -x` output), shell integration scripts.
#
# Needs: git, curl, shasum, tar, zig 0.16, tic, rsync, cargo, Homebrew llvm@22
# (for bindgen). Run on macOS, where the bindings are generated.
set -euo pipefail

root=$(cd "$(dirname "$0")/.." && pwd)
cd "$root"

lock=crates/ghostty-vt-sys/GHOSTTY.lock
current=$(sed -n 's/^commit = //p' "$lock")
commit=${1:-$current}
gpui_cn_version=$(awk '
  /^\[workspace.package\]$/ { package = 1; next }
  /^\[/ { package = 0 }
  package && /^version = "/ {
    gsub(/^version = "|".*$/, "")
    print
    exit
  }
' Cargo.toml)
crate_version=${2:-$gpui_cn_version}
case "$commit" in
  *[!0-9a-f]* | ?????????????????????????????????????????*) ;;
esac
if [ ${#commit} -ne 40 ]; then
  echo "expected a full 40-character Ghostty commit, got '$commit'" >&2
  exit 1
fi
case "$crate_version" in
  *+*)
    echo "pass the crate version without build metadata, got '$crate_version'" >&2
    exit 1
    ;;
esac
if ! [[ "$crate_version" =~ ^[0-9]+\.[0-9]+\.[0-9]+([\-][0-9A-Za-z.-]+)?$ ]]; then
  echo "expected a SemVer crate version, got '$crate_version'" >&2
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
short=${commit:0:7}
if [ "${version##*+}" != "$short" ]; then
  echo "Ghostty version '$version' does not end in commit '$short'" >&2
  exit 1
fi
ghostty_version=${version%+"$short"}
package_version="$crate_version+ghostty.${ghostty_version//+/.}.$short"
echo "    version $version"
echo "    crate   $package_version"
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
cp "$lock" third_party/GHOSTTY.lock

set_package_version() {
  local manifest=$1
  local output
  output="$work/$(basename "$(dirname "$manifest")").Cargo.toml"
  awk -v version="$package_version" '
    !updated && /^version = "/ {
      print "version = \"" version "\""
      updated = 1
      next
    }
    { print }
    END {
      if (!updated) exit 1
    }
  ' "$manifest" > "$output"
  cat "$output" > "$manifest"
}

set_workspace_dependency_version() {
  local dependency=$1
  local output="$work/root-$dependency.Cargo.toml"
  awk -v dependency="$dependency" -v version="$crate_version" '
    $0 ~ "^" dependency " = " {
      print dependency " = { path = \"crates/" dependency "\", version = \"" version "\" }"
      updated = 1
      next
    }
    { print }
    END {
      if (!updated) exit 1
    }
  ' Cargo.toml > "$output"
  cat "$output" > Cargo.toml
}

set_version_mode() {
  local mode=follow-gpui-cn
  local output="$work/root-version-mode.Cargo.toml"
  if [ "$crate_version" != "$gpui_cn_version" ]; then
    mode=independent
  fi
  awk -v mode="$mode" '
    /^version-mode = "/ {
      print "version-mode = \"" mode "\""
      updated = 1
      next
    }
    { print }
    END {
      if (!updated) exit 1
    }
  ' Cargo.toml > "$output"
  cat "$output" > Cargo.toml
}

echo "==> crate versions"
set_package_version crates/ghostty-vt-sys/Cargo.toml
set_package_version crates/ghostty-vt/Cargo.toml
set_workspace_dependency_version ghostty-vt-sys
set_workspace_dependency_version ghostty-vt
set_version_mode

echo "==> bindings"
# The doc comments bindgen keeps depend on the libclang version; CI checks
# the bindings with the same Homebrew LLVM.
LIBCLANG_PATH="$(brew --prefix llvm@22)/lib" DOCS_RS=1 \
  cargo run --quiet -p ghostty-vt-sys --features bindgen-tool --bin gen-bindings
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
cargo build -p ghostty-vt-sys -p ghostty-vt
"$root/scripts/check-ghostty-version.sh"
echo "synced to $commit ($version)"
