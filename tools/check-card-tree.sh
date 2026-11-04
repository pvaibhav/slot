#!/bin/sh
# Run against the exact tree that will be packaged for a device release.
set -eu
out="${1:?usage: check-card-tree.sh <card tree>}"

for binary in slot mgba_libretro.so gpsp_libretro.so gambatte_libretro.so fsck.exfat; do
    file "$out/System/$binary" | grep -q aarch64
done
test -x "$out/System/slot"
test -f "$out/Config/retroachievements.toml.example"
test ! -e "$out/Config/retroachievements.toml"
test ! -e "$out/Config/slot.state"
for folder in Games Guides Labels Saves States Cheats; do
    for platform in GBA GBAV GB GBC; do
        test -d "$out/$folder/$platform"
    done
done
for folder in Bezels Cheats Labels Wallpapers; do
    test -f "$out/$folder/README.txt"
done

gpsp_src_sha="$(cat vendor/gpsp-src.sha)"
test -f "$out/System/licenses/gpsp-${gpsp_src_sha}.tar.gz"
test -f "$out/System/licenses/gpsp-${gpsp_src_sha}.meta"
gambatte_src_sha="$(cat vendor/gambatte-src.sha)"
test -f "$out/System/licenses/gambatte-${gambatte_src_sha}.tar.gz"
test -f "$out/System/licenses/gambatte-${gambatte_src_sha}.meta"
mgba_src_commit="$(sed -n 's/^commit=//p' vendor/mgba_libretro.so.meta)"
test -n "$mgba_src_commit"
test -f "$out/System/licenses/mgba-${mgba_src_commit}.meta"
for core in gambatte gpsp mgba; do
    for patch in cores/$core/*.patch; do
        test -f "$out/System/licenses/$core-$(basename "$patch")"
    done
done
echo "Device binaries, content folders, config examples and core sources verified in $out"
