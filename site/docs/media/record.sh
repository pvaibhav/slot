#!/bin/sh
set -eu
here=$(cd "$(dirname "$0")" && pwd)
card=$(cd "$1" && pwd)
shift
[ $# -gt 0 ] || set -- $(cd "$here/clips" && ls *.txt | grep -v '^link-' | sed 's/\.txt$//')
cd "$here/../../.."
cargo build -q --release --example record -p slot
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
for name in "$@"; do
	cart=$(sed -n 's/^# cart: *//p' "$here/clips/$name.txt")
	rm -rf "$work/card" && cp -R "$card" "$work/card"
	rm -rf "$work/card/Wallpapers"
	platform=$(sed -n 's/^# platform: *//p' "$here/clips/$name.txt")
	[ -n "$cart" ] && [ -z "$platform" ] && platform=gba
	if [ -n "$cart" ]; then
		cart=$(python3 -c 'import os, sys, unicodedata as u
want = u.normalize("NFC", sys.argv[1])
for f in os.listdir(sys.argv[2]):
    stem = os.path.splitext(f)[0]
    if u.normalize("NFC", stem) == want:
        print(stem)
        break' "$cart" "$card/Games/$(echo "$platform" | tr a-z A-Z)")
	fi
	sed -i.bak "s|^cart=.*|cart=$cart|; s|^cart_platform=.*|cart_platform=$platform|" \
		"$work/card/Config/slot.state"
	sed -n 's/^# state: *//p' "$here/clips/$name.txt" | while IFS='=' read -r key value; do
		grep -v "^$key=" "$work/card/Config/slot.state" > "$work/state" || true
		echo "$key=$value" >> "$work/state"
		mv "$work/state" "$work/card/Config/slot.state"
	done
	sed -n 's/^# bare: *//p' "$here/clips/$name.txt" | while IFS= read -r stem; do
		find "$work/card/Labels" -name "$stem.*" -delete
	done
	sed -n 's/^# shell: *//p' "$here/clips/$name.txt" >> "$work/card/Labels/cart_shell.txt"
	core=$(sed -n 's/^# core: *//p' "$here/clips/$name.txt")
	[ -n "$core" ] && echo "$cart = $core" >> "$work/card/Config/selected_core.txt"
	clips=$(sed -n 's/^rec \([a-z0-9-]*\)$/\1/p' "$here/clips/$name.txt")
	out="$here/$name.mp4"
	[ -n "$clips" ] && out="$here"
	SLOT_SILENT=1 target/release/examples/record "$work/card" "$here/clips/$name.txt" "$out"
	for clip in ${clips:-$name}; do
		ffmpeg -v error -y -i "$here/$clip.mp4" -vf "select=eq(n\,0)" -vframes 1 \
			-c:v libwebp -quality 82 "$here/$clip.webp"
	done
done
