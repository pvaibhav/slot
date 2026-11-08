#!/bin/sh
set -eu

here="$(cd "$(dirname "$0")" && pwd)"

device_tune="-mcpu=cortex-a55"
image="debian:bookworm"

usage() {
	echo "usage: $0 stamp COMMIT | build COMMIT TARBALL WORKDIR OUTDIR | host-stamp COMMIT | build-host COMMIT TARBALL WORKDIR OUTDIR" >&2
	exit 2
}

sha256() {
	if command -v sha256sum >/dev/null 2>&1; then sha256sum "$1"; else shasum -a 256 "$1"; fi |
		cut -d' ' -f1
}

stamp() {
	echo "commit=$1"
	echo "source=https://github.com/beebono/DSperate/archive/$1.tar.gz"
	echo "image=$image"
	echo "recipe=cmake shim (DSPERATE_SDL=OFF DSPERATE_CHEEVOS=OFF DSPERATE_NET=OFF DSPERATE_TESTS=OFF DSPERATE_PGO=OFF)"
	echo "device_tune=$device_tune"
	for p in "$here"/patches/*.patch; do
		[ -f "$p" ] || continue
		echo "patch=$(basename "$p") sha256:$(sha256 "$p")"
	done
	(cd "$here/shim" && find . -type f | sed 's|^\./||' | LC_ALL=C sort) | while IFS= read -r f; do
		echo "shim=$f sha256:$(sha256 "$here/shim/$f")"
	done
}

build() {
	commit="$1" tarball="$2" work="$3" out="$4"
	src="$work/DSperate-$commit"

	if ! command -v cmake >/dev/null 2>&1 || ! command -v ninja >/dev/null 2>&1 || ! command -v g++ >/dev/null 2>&1 || ! command -v patch >/dev/null 2>&1; then
		apt-get update -qq
		DEBIAN_FRONTEND=noninteractive apt-get install -y -qq cmake ninja-build g++ file patch >/dev/null
	fi

	rm -rf "$work"
	mkdir -p "$work" "$out"
	tar -xzf "$tarball" -C "$work"
	if [ ! -f "$src/src/core/nds.h" ]; then
		echo "$tarball does not hold DSperate-$commit/src/core/nds.h" >&2
		exit 1
	fi
	for p in "$here"/patches/*.patch; do
		[ -f "$p" ] || continue
		patch -s -d "$src" -p1 <"$p"
	done

	gcc_version="$(g++ -dumpfullversion)"

	cmake -S "$here/shim" -B "$work/build" -G Ninja \
		-DCMAKE_BUILD_TYPE=Release \
		-DDSPERATE_SRC="$src" \
		-DDSPERATE_SDL=OFF -DDSPERATE_WAYLAND=OFF -DDSPERATE_CHEEVOS=OFF -DDSPERATE_NET=OFF \
		-DDSPERATE_TESTS=OFF -DDSPERATE_HEADLESS=ON -DDSPERATE_JIT=ON -DDSPERATE_NEON=ON \
		-DDSPERATE_PGO=OFF \
		-DDSPERATE_TUNE="$device_tune"
	cmake --build "$work/build"

	cp "$work/build/slot_ds.so" "$out/slot_ds.so"
	cp "$work/build/dsperate/src/frontend/headless/dsperate-headless" "$out/dsperate-headless"
	stamp "$commit" > "$out/slot_ds.so.meta"
	echo "gcc=$gcc_version" >> "$out/slot_ds.so.meta"
}

host_stamp() {
	stamp "$1"
	echo "host=macos jit=off"
	for p in "$here"/patches-macos/*.patch; do
		[ -f "$p" ] || continue
		echo "macos-patch=$(basename "$p") sha256:$(sha256 "$p")"
	done
}

build_host() {
	commit="$1" tarball="$2"
	rm -rf "$3"
	mkdir -p "$3" "$4"
	work="$(cd "$3" && pwd)" out="$(cd "$4" && pwd)"
	src="$work/DSperate-$commit"

	tar -xzf "$tarball" -C "$work"
	for p in "$here"/patches/*.patch "$here"/patches-macos/*.patch; do
		[ -f "$p" ] || continue
		patch -s -d "$src" -p1 <"$p"
	done

	generator="Unix Makefiles"
	command -v ninja >/dev/null 2>&1 && generator="Ninja"
	cmake -S "$here/shim" -B "$work/build" -G "$generator" \
		-DCMAKE_BUILD_TYPE=Release \
		-DCMAKE_CXX_STANDARD=17 \
		-DDSPERATE_SRC="$src" \
		-DDSPERATE_SDL=OFF -DDSPERATE_WAYLAND=OFF -DDSPERATE_CHEEVOS=OFF -DDSPERATE_NET=OFF \
		-DDSPERATE_TESTS=OFF -DDSPERATE_HEADLESS=OFF -DDSPERATE_JIT=OFF -DDSPERATE_NEON=ON \
		-DDSPERATE_PGO=OFF \
		-DDSPERATE_TUNE=
	cmake --build "$work/build" -j "$(sysctl -n hw.ncpu 2>/dev/null || echo 4)"

	cp "$work/build/slot_ds.dylib" "$out/slot_ds.dylib"
	host_stamp "$commit" > "$out/slot_ds.dylib.meta"
}

[ $# -ge 1 ] || usage
case "$1" in
stamp) [ $# -eq 2 ] || usage; stamp "$2" ;;
build) [ $# -eq 5 ] || usage; build "$2" "$3" "$4" "$5" ;;
host-stamp) [ $# -eq 2 ] || usage; host_stamp "$2" ;;
build-host) [ $# -eq 5 ] || usage; build_host "$2" "$3" "$4" "$5" ;;
*) usage ;;
esac
