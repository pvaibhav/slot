#!/bin/sh
# Run in the ARM64 bullseye build container. Keep runtime dependencies on the card.
set -eu
out=${1:-vendor/slot-net}
mkdir -p "$out"
out=$(cd "$out" && pwd)
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT HUP INT TERM
cd "$work"
printf '%s\n' 'deb-src http://deb.debian.org/debian bullseye main' > /etc/apt/sources.list.d/slot-net-src.list
apt-get -qq update
apt-get download iw=5.9-3 libnl-3-200=3.4.0-1+b1 libnl-genl-3-200=3.4.0-1+b1
mkdir root
for package in ./*.deb; do dpkg-deb -x "$package" root; done
cp root/sbin/iw "$out/iw"
cp -L root/lib/aarch64-linux-gnu/libnl-3.so.200 "$out/"
cp -L root/lib/aarch64-linux-gnu/libnl-genl-3.so.200 "$out/"
mkdir -p "$out/licenses/sources"
for package in iw libnl-3-200 libnl-genl-3-200; do
    cp "root/usr/share/doc/$package/copyright" "$out/licenses/$package.txt"
done
cp /usr/share/common-licenses/LGPL-2.1 "$out/licenses/"
apt-get source --download-only iw=5.9-3 libnl3=3.4.0-1
for source in ./*.dsc ./*.tar.*; do [ ! -f "$source" ] || cp "$source" "$out/licenses/sources/"; done
printf '%s\n' 'iw 5.9-3 arm64; libnl 3.4.0-1+b1 arm64 (Debian bullseye)' > "$out/versions.txt"
