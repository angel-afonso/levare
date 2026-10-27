#!/usr/bin/env bash
# Usage: build_sfizz.sh <out dir>
#
# Builds the sfizz engine the app bundle ships (Contents/Frameworks) as one
# universal library for Apple Silicon and Intel Macs, from the official
# sfizz 1.2.3 source (see docs/vsco.md), and writes:
#   <out dir>/libsfizz.dylib
#   <out dir>/licenses/sfizz/…   its license and its dependencies' notices
# Needs cmake and the Xcode command line tools.
set -euo pipefail

out="$(mkdir -p "$1" && cd "$1" && pwd)"
version=1.2.3
sha256=a9339eac7620d7f0f6b44bdfe860680fab73e66efad4b5f15b21198dd9436822
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

curl -fsSL -o "$work/sfizz.tar.gz" \
  "https://github.com/sfztools/sfizz/releases/download/$version/sfizz-$version.tar.gz"
echo "$sha256  $work/sfizz.tar.gz" | shasum -a 256 -c -
tar -xzf "$work/sfizz.tar.gz" -C "$work"
source="$work/sfizz-$version"

# Current Apple Clang rejects four redundant `template` qualifiers in the
# bundled atomic_queue; removing them changes no audio processing.
queue="$(find "$source/external" -name atomic_queue.h -path '*atomic_queue/*' | head -1)"
sed -i '' \
  -e 's/Base::template do_pop_any(/Base::do_pop_any(/g' \
  -e 's/Base::template do_push_any(/Base::do_push_any(/g' \
  "$queue"

# One build per architecture: sfizz picks its SIMD flags from
# PROJECT_SYSTEM_PROCESSOR, and `aarch64` keeps it from choosing 32-bit ARM
# flags for `arm64`.
for arch in arm64:aarch64 x86_64:x86_64; do
  build="$work/build-${arch%%:*}"
  cmake -S "$source" -B "$build" \
    -DCMAKE_BUILD_TYPE=Release \
    -DCMAKE_OSX_ARCHITECTURES="${arch%%:*}" \
    -DPROJECT_SYSTEM_PROCESSOR="${arch#*:}" \
    -DCMAKE_OSX_DEPLOYMENT_TARGET=11.0 \
    -DSFIZZ_JACK=OFF -DSFIZZ_RENDER=OFF -DSFIZZ_TESTS=OFF \
    -DSFIZZ_SHARED=ON -DENABLE_LTO=OFF \
    -DCMAKE_POLICY_VERSION_MINIMUM=3.5 >/dev/null
  cmake --build "$build" -j "$(sysctl -n hw.ncpu)" >/dev/null
done

lipo -create -output "$out/libsfizz.dylib" \
  "$work/build-arm64/library/lib/libsfizz.$version.dylib" \
  "$work/build-x86_64/library/lib/libsfizz.$version.dylib"
archs="$(lipo -archs "$out/libsfizz.dylib")"
if [[ "$archs" != *arm64* || "$archs" != *x86_64* ]]; then
  echo "libsfizz.dylib is missing an architecture: $archs" >&2
  exit 1
fi

# The same notices scripts/prepare_vsco.py keeps.
rm -rf "$out/licenses/sfizz"
(cd "$source" && find . -type f \( -iname 'LICENSE*' -o -iname 'COPYING*' -o -iname 'NOTICE*' -o -iname 'AUTHORS*' \)) |
  while read -r path; do
    mkdir -p "$out/licenses/sfizz/$(dirname "$path")"
    cp "$source/$path" "$out/licenses/sfizz/$path"
  done
echo "Built $out/libsfizz.dylib ($archs)"
