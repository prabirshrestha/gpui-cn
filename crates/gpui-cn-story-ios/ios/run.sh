#!/usr/bin/env bash
# Build the gallery for the iOS simulator, wrap it in an app bundle, and
# launch it.
#
#   crates/gpui-cn-story-ios/ios/run.sh [--release] [--device "iPhone 16 Pro"] [--no-run]
#
# The Rust binary is the whole application; there is no Xcode project.
# Needs Xcode with an iOS simulator runtime and the
# aarch64-apple-ios-sim Rust target (rustup target add aarch64-apple-ios-sim).
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
root="$(cd "$here/../../.." && pwd)"
target=aarch64-apple-ios-sim
profile=debug
cargo_flags=()
device=""
run=1

while [[ $# -gt 0 ]]; do
    case "$1" in
        --release) profile=release; cargo_flags+=(--release); shift ;;
        --device) device="$2"; shift 2 ;;
        --no-run) run=0; shift ;;
        *) echo "unknown argument: $1" >&2; exit 2 ;;
    esac
done

cd "$root"
cargo build -p gpui-cn-story-ios --target "$target" ${cargo_flags[@]+"${cargo_flags[@]}"}

bundle="$root/target/$target/$profile/gpui-cn-story.app"
rm -rf "$bundle"
mkdir -p "$bundle"
cp "$root/target/$target/$profile/gpui-cn-story-ios" "$bundle/gpui-cn-story-ios"
cp "$here/Info.plist" "$bundle/Info.plist"
codesign --force --sign - "$bundle" >/dev/null
echo "$bundle"

if [[ $run -eq 0 ]]; then
    exit 0
fi

# Boot a simulator: the named one, else the one already booted, else the
# first available iPhone.
if [[ -n "$device" ]]; then
    udid="$(xcrun simctl list devices available -j | python3 -c "
import json, sys
devices = [d for runtime in json.load(sys.stdin)['devices'].values() for d in runtime]
match = [d for d in devices if d['name'] == sys.argv[1]]
print(match[0]['udid'] if match else '')" "$device")"
else
    udid="$(xcrun simctl list devices booted -j | python3 -c "
import json, sys
devices = [d for runtime in json.load(sys.stdin)['devices'].values() for d in runtime]
print(devices[0]['udid'] if devices else '')")"
    if [[ -z "$udid" ]]; then
        udid="$(xcrun simctl list devices available -j | python3 -c "
import json, sys
devices = [d for runtime in json.load(sys.stdin)['devices'].values() for d in runtime if 'iPhone' in d['name']]
print(devices[-1]['udid'] if devices else '')")"
    fi
fi
if [[ -z "$udid" ]]; then
    echo "no iOS simulator found; install one in Xcode" >&2
    exit 1
fi

xcrun simctl boot "$udid" 2>/dev/null || true
open -a Simulator
xcrun simctl install "$udid" "$bundle"
xcrun simctl launch --console-pty "$udid" me.prabir.gpui-cn-story
