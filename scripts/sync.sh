#!/usr/bin/env bash
# Move the Ghostty pin and regenerate everything derived from it.
#
#   scripts/sync.sh [<vt-X.Y.Z> | <ghostty-commit>] [<crate-version>]
#
# A `vt-X.Y.Z` tag pins a libghostty-vt release. The script checks the signed
# SHA256SUMS of the release with minisign and records the source tarball, the
# dependency tarball, and the prebuilt libraries (the XCFramework and the two
# Windows zips) with their sha256 in GHOSTTY.lock. The crate build metadata
# is `ghostty-vt.X.Y.Z`.
#
# A full 40-character commit pins a tip build of Ghostty's main branch. Its
# source tarball is checked against a `git archive` reproduction. Tip builds
# have no prebuilt libraries, so every target builds from source. The crate
# build metadata is `ghostty.<ghostty-version>.<short-commit>`.
#
# Without an argument the current pin from GHOSTTY.lock is resynced.
# A new pin requires the SemVer core for ghostty-vt and ghostty-vt-sys,
# without build metadata. This normally matches gpui-cn.
# Steps: submodule checkout, GHOSTTY.lock, crate versions, bindings, terminfo
# (source plus `tic -x` output), shell integration scripts.
#
# Needs: git, curl, shasum, tar, zig 0.17, tic, rsync, cargo, Homebrew llvm@22
# (for bindgen), and minisign for a release. Run on macOS, where the bindings
# are generated.
set -euo pipefail

root=$(cd "$(dirname "$0")/.." && pwd)
cd "$root"

# The key that signs Ghostty and libghostty-vt releases.
minisign_key=RWQlAjJC23149WL2sEpT/l0QKy7hMIFhYdQOFy0Z7z7PbneUgvlsnYcV

lock=crates/ghostty-vt-sys/GHOSTTY.lock
current=$(sed -n 's/^tag = //p' "$lock")
if [ -z "$current" ]; then
  current=$(sed -n 's/^commit = //p' "$lock")
fi
pin=${1:-$current}
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

checkout() {
  echo "==> submodule third_party/ghostty at $1"
  git submodule update --init third_party/ghostty
  git -C third_party/ghostty fetch --quiet origin "$2"
  git -C third_party/ghostty checkout --quiet --detach "$1"
  # Stage the gitlink, so the pin survives a later `git submodule deinit`.
  git add third_party/ghostty
}

if [[ "$pin" =~ ^vt-([0-9]+\.[0-9]+\.[0-9]+)$ ]]; then
  tag=$pin
  version=${BASH_REMATCH[1]}
  release="https://release.files.ghostty.org/libghostty-vt/$version"

  git submodule update --init third_party/ghostty
  git -C third_party/ghostty fetch --quiet origin "refs/tags/$tag:refs/tags/$tag"
  commit=$(git -C third_party/ghostty rev-parse "$tag^{commit}")
  checkout "$commit" "$commit"

  echo "==> release $release"
  curl -fsSL -o "$work/SHA256SUMS" "$release/SHA256SUMS"
  curl -fsSL -o "$work/SHA256SUMS.minisig" "$release/SHA256SUMS.minisig"
  minisign -Vq -P "$minisign_key" -m "$work/SHA256SUMS"
  echo "    SHA256SUMS signature verified"

  source_file="libghostty-vt-$version.tar.gz"
  deps_file="libghostty-vt-$version-deps.tar.gz"
  xcframework_file="ghostty-vt-$version.xcframework.zip"
  windows_x86_64_file="ghostty-vt-$version-x86_64-windows-gnu.zip"
  windows_aarch64_file="ghostty-vt-$version-aarch64-windows-gnu.zip"
  # Print the signed sha256 of a release file after checking the download.
  release_sha() {
    local file=$1 expected actual
    expected=$(awk -v file="$file" '$2 == file { print $1 }' "$work/SHA256SUMS")
    if [ -z "$expected" ]; then
      echo "$file is not in the signed SHA256SUMS" >&2
      exit 1
    fi
    curl -fsSL -o "$work/$file" "$release/$file"
    actual=$(shasum -a 256 "$work/$file" | cut -d' ' -f1)
    if [ "$actual" != "$expected" ]; then
      echo "$file has sha256 $actual, SHA256SUMS says $expected" >&2
      exit 1
    fi
    printf '%s' "$expected"
  }
  sha=$(release_sha "$source_file")
  deps_sha=$(release_sha "$deps_file")
  xcframework_sha=$(release_sha "$xcframework_file")
  windows_x86_64_sha=$(release_sha "$windows_x86_64_file")
  windows_aarch64_sha=$(release_sha "$windows_aarch64_file")

  tarroot="libghostty-vt-$version"
  depsroot="libghostty-vt-$version-deps"
  if [ "$(tar tzf "$work/$source_file" | head -1 | cut -d/ -f1)" != "$tarroot" ]; then
    echo "$source_file does not unpack to $tarroot" >&2
    exit 1
  fi
  if [ "$(tar tzf "$work/$deps_file" | head -1 | cut -d/ -f1)" != "$depsroot" ]; then
    echo "$deps_file does not unpack to $depsroot" >&2
    exit 1
  fi
  tar xzf "$work/$source_file" -C "$work"
  lib_version=$(tr -d '[:space:]' < "$work/$tarroot/LIB_VERSION")
  if [ "$lib_version" != "$version" ]; then
    echo "$source_file has LIB_VERSION '$lib_version', expected '$version'" >&2
    exit 1
  fi
  # The bindings come from the submodule headers, so they must be the
  # headers of the released library.
  if ! diff -r "$work/$tarroot/include" third_party/ghostty/include > /dev/null; then
    echo "the headers in $source_file differ from $tag in the submodule" >&2
    exit 1
  fi
  package_version="$crate_version+ghostty-vt.$version"
  echo "    version $version ($commit)"
  echo "    crate   $package_version"

  cat > "$lock" <<EOF
# Pinned libghostty-vt release. Written by scripts/sync.sh.
commit = $commit
tag = $tag
version = $version
url = $release/$source_file
sha256 = $sha
root = $tarroot
release = $release
deps = $deps_file
deps_sha256 = $deps_sha
deps_root = $depsroot
xcframework = $xcframework_file
xcframework_sha256 = $xcframework_sha
windows_x86_64 = $windows_x86_64_file
windows_x86_64_sha256 = $windows_x86_64_sha
windows_aarch64 = $windows_aarch64_file
windows_aarch64_sha256 = $windows_aarch64_sha
EOF
else
  commit=$pin
  if ! [[ "$commit" =~ ^[0-9a-f]{40}$ ]]; then
    echo "expected vt-X.Y.Z or a full 40-character Ghostty commit, got '$commit'" >&2
    exit 1
  fi
  checkout "$commit" "$commit"

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
fi

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
cargo build -p ghostty-vt-sys
"$root/scripts/check-ghostty-version.sh"
echo "synced to $pin ($commit, $version)"
