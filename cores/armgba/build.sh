#!/bin/sh
set -eu

here="$(cd "$(dirname "$0")" && pwd)"

device_cflags="-mcpu=cortex-a53"
cmake_version="3.28.6"

usage() {
	echo "usage: $0 stamp COMMIT | build COMMIT TARBALL WORKDIR OUT" >&2
	exit 2
}

sha256() {
	if command -v sha256sum >/dev/null 2>&1; then sha256sum "$1"; else shasum -a 256 "$1"; fi |
		cut -d' ' -f1
}

stamp() {
	echo "commit=$1"
	echo "source=https://github.com/MagnominusGit/armGBA/archive/$1.tar.gz"
	echo "recipe=cmake -DCMAKE_BUILD_TYPE=Release, target armgba_libretro"
	echo "device_cflags=$device_cflags"
	for p in "$here"/*.patch; do
		echo "patch=$(basename "$p") sha256:$(sha256 "$p")"
	done
}

build() {
	commit="$1" tarball="$2" work="$3" out="$4"
	src="$work/armGBA-$commit"

	rm -rf "$work"
	mkdir -p "$work"
	# The build image has no cmake, and the bullseye package no longer installs.
	command -v cmake >/dev/null 2>&1 || {
		curl -fsSL "https://github.com/Kitware/CMake/releases/download/v$cmake_version/cmake-$cmake_version-linux-$(uname -m).tar.gz" |
			tar -xz -C "$work"
		PATH="$work/cmake-$cmake_version-linux-$(uname -m)/bin:$PATH"
	}
	tar -xzf "$tarball" -C "$work"
	if [ ! -f "$src/CMakeLists.txt" ]; then
		echo "$tarball does not hold armGBA-$commit/CMakeLists.txt" >&2
		exit 1
	fi

	for p in "$here"/*.patch; do
		patch -p1 --batch --forward -d "$src" <"$p"
	done

	cmake -S "$src" -B "$work/build" -DCMAKE_BUILD_TYPE=Release \
		-DCMAKE_C_FLAGS="$device_cflags" -DCMAKE_CXX_FLAGS="$device_cflags"
	cmake --build "$work/build" --target armgba_libretro -j"$(getconf _NPROCESSORS_ONLN)"

	mkdir -p "$(dirname "$out")"
	cp "$work/build/armgba_libretro.so" "$out"
	stamp "$commit" >"$out.meta"
}

case "${1:-}" in
stamp)
	[ $# -eq 2 ] && [ -n "$2" ] || usage
	stamp "$2"
	;;
build)
	[ $# -eq 5 ] && [ -n "$2" ] && [ -n "$3" ] && [ -n "$4" ] && [ -n "$5" ] || usage
	build "$2" "$3" "$4" "$5"
	;;
*)
	usage
	;;
esac
