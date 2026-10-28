#!/usr/bin/env bash
# Usage: build-site.sh <output-dir> [tag]
#
# Assembles the GitHub Pages site: the files in site/, the app icon and the
# Bravura font, with %TAG% and %VERSION% replaced by the latest release (or the
# given tag). Without a tag it needs gh with a token, and it refuses to publish
# download links that the release does not have.
set -euo pipefail

out="$1"
tag="${2:-$(gh release view --json tagName --jq .tagName)}"
version="${tag#v}"

if [[ -z "${2:-}" ]]; then
  assets="$(gh release view "$tag" --json assets --jq '.assets[].name')"
  for file in \
    "levare-$tag-universal-apple-darwin.dmg" \
    "levare-$tag-x86_64-pc-windows-msvc.zip" \
    "levare-$tag-x86_64-unknown-linux-gnu.tar.gz" \
    SHA256SUMS; do
    if ! grep -qxF "$file" <<<"$assets"; then
      echo "::error::Release $tag has no $file; the download links would be broken."
      exit 1
    fi
  done
fi

rm -rf "$out"
mkdir -p "$out/assets"
cp -R site/. "$out/"
cp assets/branding/levare/icon-macos-v2.png "$out/assets/levare.png"
# Bravura is published unmodified: its licence reserves the name for the
# original font, so it is not subset.
cp assets/fonts/Bravura.otf assets/fonts/OFL.txt "$out/assets/"
touch "$out/.nojekyll"

find "$out" -name '*.html' -exec sed -i.bak -e "s/%TAG%/$tag/g" -e "s/%VERSION%/$version/g" {} +
find "$out" -name '*.bak' -delete

if grep -rqE '%(TAG|VERSION)%' "$out"; then
  echo "::error::A placeholder was left unreplaced."
  exit 1
fi
echo "Built the site for $tag in $out"
