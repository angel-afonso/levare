#!/usr/bin/env bash
# Usage: build_sfizz.sh <out dir>
#
# Builds the sfizz engine Levare ships, from the official sfizz 1.2.3 source
# (see docs/vsco.md), for the system it runs on, and writes:
#   macOS:   <out dir>/libsfizz.dylib  one universal library for Apple Silicon
#                                      and Intel Macs (Contents/Frameworks)
#   Linux:   <out dir>/libsfizz.so     x86_64, shipped next to `levare`
#   Windows: <out dir>/sfizz.dll       x64 with the static C runtime, shipped
#                                      next to `levare.exe`; needs nothing else
#   <out dir>/licenses/sfizz/…         its license and its dependencies' notices
# Needs cmake and a C++ compiler: the Xcode command line tools on macOS, GCC
# on Linux, Visual Studio on Windows (run from Git Bash).
set -euo pipefail

out="$(mkdir -p "$1" && cd "$1" && pwd)"
version=1.2.3
sha256=a9339eac7620d7f0f6b44bdfe860680fab73e66efad4b5f15b21198dd9436822
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

case "$(uname -s)" in
  Darwin) os=macos ;;
  Linux) os=linux ;;
  MINGW* | MSYS* | CYGWIN*) os=windows ;;
  *)
    echo "build_sfizz.sh does not know how to build on $(uname -s)" >&2
    exit 1
    ;;
esac

if command -v sha256sum >/dev/null; then
  sha256_check=(sha256sum -c -)
else
  sha256_check=(shasum -a 256 -c -)
fi
jobs="$(getconf _NPROCESSORS_ONLN 2>/dev/null || echo 4)"

curl -fsSL -o "$work/sfizz.tar.gz" \
  "https://github.com/sfztools/sfizz/releases/download/$version/sfizz-$version.tar.gz"
echo "$sha256  $work/sfizz.tar.gz" | "${sha256_check[@]}"
tar -xzf "$work/sfizz.tar.gz" -C "$work"
source="$work/sfizz-$version"

# Current Apple Clang rejects four redundant `template` qualifiers in the
# bundled atomic_queue; removing them changes no audio processing. The same
# source is built everywhere. `-i.orig` works with both BSD and GNU sed.
queue="$(find "$source/external" -name atomic_queue.h -path '*atomic_queue/*' | head -1)"
sed -i.orig \
  -e 's/Base::template do_pop_any(/Base::do_pop_any(/g' \
  -e 's/Base::template do_push_any(/Base::do_push_any(/g' \
  "$queue"
rm "$queue.orig"

# What every build shares: the library alone, without JACK, the renderer,
# tests or link-time optimization.
common=(
  -DSFIZZ_JACK=OFF -DSFIZZ_RENDER=OFF -DSFIZZ_TESTS=OFF
  -DSFIZZ_SHARED=ON -DENABLE_LTO=OFF
  -DCMAKE_POLICY_VERSION_MINIMUM=3.5
)

case "$os" in
  macos)
    # One build per architecture: sfizz picks its SIMD flags from
    # PROJECT_SYSTEM_PROCESSOR, and `aarch64` keeps it from choosing 32-bit
    # ARM flags for `arm64`.
    for arch in arm64:aarch64 x86_64:x86_64; do
      build="$work/build-${arch%%:*}"
      cmake -S "$source" -B "$build" \
        -DCMAKE_BUILD_TYPE=Release \
        -DCMAKE_OSX_ARCHITECTURES="${arch%%:*}" \
        -DPROJECT_SYSTEM_PROCESSOR="${arch#*:}" \
        -DCMAKE_OSX_DEPLOYMENT_TARGET=11.0 \
        "${common[@]}" >/dev/null
      cmake --build "$build" -j "$jobs" >/dev/null
    done

    library=libsfizz.dylib
    lipo -create -output "$out/$library" \
      "$work/build-arm64/library/lib/libsfizz.$version.dylib" \
      "$work/build-x86_64/library/lib/libsfizz.$version.dylib"
    archs="$(lipo -archs "$out/$library")"
    if [[ "$archs" != *arm64* || "$archs" != *x86_64* ]]; then
      echo "$library is missing an architecture: $archs" >&2
      exit 1
    fi
    built="$archs"
    ;;

  linux)
    build="$work/build"
    # The C++ runtime linked in, so the library needs only glibc beside it
    # and no particular libstdc++.
    cmake -S "$source" -B "$build" \
      -DCMAKE_BUILD_TYPE=Release \
      -DPROJECT_SYSTEM_PROCESSOR=x86_64 \
      -DCMAKE_SHARED_LINKER_FLAGS="-static-libstdc++ -static-libgcc" \
      "${common[@]}" >/dev/null
    cmake --build "$build" -j "$jobs" >/dev/null

    # Levare loads it by path, so the versioned file goes in under the plain
    # name.
    library=libsfizz.so
    cp "$build/library/lib/libsfizz.so.$version" "$out/$library"
    strip --strip-unneeded "$out/$library"
    if ldd "$out/$library" | grep -E 'libstdc\+\+|libgcc_s|not found'; then
      echo "$library needs a shared library besides glibc" >&2
      exit 1
    fi
    built="$(file -b "$out/$library")"
    ;;

  windows)
    build="$work/build"
    # The static C runtime (/MT), so the DLL needs no Visual C++
    # redistributable beside it.
    cmake -S "$source" -B "$build" -A x64 \
      -DPROJECT_SYSTEM_PROCESSOR=x86_64 \
      -DCMAKE_MSVC_RUNTIME_LIBRARY=MultiThreaded \
      "${common[@]}" >/dev/null
    cmake --build "$build" --config Release -j "$jobs" >/dev/null

    library=sfizz.dll
    dll="$(find "$build" -name "$library" -path '*Release*' | head -1)"
    if [[ -z "$dll" ]]; then
      echo "The build made no $library" >&2
      exit 1
    fi
    cp "$dll" "$out/$library"
    # Import names are plain text in the DLL: a dynamic C runtime would show
    # up here.
    if grep -aqiE 'vcruntime[0-9]*\.dll|msvcp[0-9]*\.dll|ucrtbase\.dll|api-ms-win-crt' "$out/$library"; then
      echo "$library links the dynamic C runtime" >&2
      exit 1
    fi
    built="$(file -b "$out/$library" 2>/dev/null || echo x64)"
    ;;
esac

# The same notices scripts/prepare_vsco.py keeps.
rm -rf "$out/licenses/sfizz"
(cd "$source" && find . -type f \( -iname 'LICENSE*' -o -iname 'COPYING*' -o -iname 'NOTICE*' -o -iname 'AUTHORS*' \)) |
  while read -r path; do
    mkdir -p "$out/licenses/sfizz/$(dirname "$path")"
    cp "$source/$path" "$out/licenses/sfizz/$path"
  done
echo "Built $out/$library ($built)"
