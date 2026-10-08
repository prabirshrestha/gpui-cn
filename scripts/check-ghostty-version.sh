#!/usr/bin/env bash
# Verify that both terminal crate versions identify the pinned Ghostty source.
set -euo pipefail

root=$(cd "$(dirname "$0")/.." && pwd)
cd "$root"

lock=crates/ghostty-vt-sys/GHOSTTY.lock
commit=$(sed -n 's/^commit = //p' "$lock")
ghostty_version=$(sed -n 's/^version = //p' "$lock")
gpui_cn_version=$(awk '
  /^\[workspace.package\]$/ { package = 1; next }
  /^\[/ { package = 0 }
  package && /^version = "/ {
    gsub(/^version = "|".*$/, "")
    print
    exit
  }
' Cargo.toml)
version_mode=$(awk '
  /^\[workspace.metadata.ghostty\]$/ { metadata = 1; next }
  /^\[/ { metadata = 0 }
  metadata && /^version-mode = "/ {
    gsub(/^version-mode = "|".*$/, "")
    print
    exit
  }
' Cargo.toml)

if ! [[ "$commit" =~ ^[0-9a-f]{40}$ ]]; then
  echo "$lock has an invalid commit: '$commit'" >&2
  exit 1
fi

short=${commit:0:7}
if [ "${ghostty_version##*+}" != "$short" ]; then
  echo "$lock version '$ghostty_version' does not end in commit '$short'" >&2
  exit 1
fi

upstream=${ghostty_version%+"$short"}
metadata="ghostty.${upstream//+/.}.$short"
sys_version=$(sed -n 's/^version = "\(.*\)"/\1/p' crates/ghostty-vt-sys/Cargo.toml)
safe_version=$(sed -n 's/^version = "\(.*\)"/\1/p' crates/ghostty-vt/Cargo.toml)
sys_core=${sys_version%%+*}
safe_core=${safe_version%%+*}
sys_requirement=$(sed -n 's/^ghostty-vt-sys = .*version = "\([^"]*\)".*/\1/p' Cargo.toml)
safe_requirement=$(sed -n 's/^ghostty-vt = .*version = "\([^"]*\)".*/\1/p' Cargo.toml)

if [ "$sys_version" != "$safe_version" ]; then
  echo "ghostty-vt-sys is $sys_version but ghostty-vt is $safe_version" >&2
  exit 1
fi

if [ "$sys_requirement" != "$sys_core" ] || [ "$safe_requirement" != "$safe_core" ]; then
  echo "workspace terminal requirements do not match crate cores" >&2
  echo "ghostty-vt-sys: crate $sys_core, requirement $sys_requirement" >&2
  echo "ghostty-vt: crate $safe_core, requirement $safe_requirement" >&2
  exit 1
fi

case "$version_mode" in
  follow-gpui-cn)
    if [ "$sys_core" != "$gpui_cn_version" ]; then
      echo "terminal crate core $sys_core must follow gpui-cn $gpui_cn_version" >&2
      echo "use independent mode only for an intentional engine release" >&2
      exit 1
    fi
    ;;
  independent) ;;
  *)
    echo "unknown Ghostty version mode '$version_mode'" >&2
    exit 1
    ;;
esac

case "$sys_version" in
  *+"$metadata") ;;
  *)
    echo "terminal crate version '$sys_version' must end in '+$metadata'" >&2
    exit 1
    ;;
esac

if ! cmp -s "$lock" third_party/GHOSTTY.lock; then
  echo "$lock and third_party/GHOSTTY.lock differ; run scripts/sync.sh" >&2
  exit 1
fi

echo "ghostty-vt crates $sys_version pin Ghostty $ghostty_version ($commit), mode $version_mode"
