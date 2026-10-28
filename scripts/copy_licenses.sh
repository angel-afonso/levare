#!/bin/sh
# Copies the license files kept in the repository into a licenses folder, in
# the layout every download ships and Acerca de Levare › Ver licencias opens:
#   Levare-LICENSE.txt   Levare's own MIT license
#   fonts/               Bravura, Cantarell and Liberation (OFL)
#   GeneralUser-GS/      the bundled SoundFont's license and origin
# The Release workflow adds the Rust crates' notices
# (.github/scripts/collect-licenses.sh) and sfizz's; scripts/bundle_macos.py
# uses this alone for local bundles.
# Usage: scripts/copy_licenses.sh <licenses folder>
set -eu

root="$(cd "$(dirname "$0")/.." && pwd)"
dest="$1"
mkdir -p "$dest/fonts" "$dest/GeneralUser-GS"
cp "$root/LICENSE" "$dest/Levare-LICENSE.txt"
cp "$root/assets/fonts/OFL.txt" "$dest/fonts/Bravura-OFL.txt"
cp "$root/assets/fonts/Cantarell-OFL.txt" "$dest/fonts/Cantarell-OFL.txt"
cp "$root/assets/fonts/Liberation-LICENSE.txt" "$dest/fonts/Liberation-LICENSE.txt"
cp "$root/assets/fonts/README.md" "$dest/fonts/README.md"
cp "$root/assets/sounds/LICENSE.txt" "$root/assets/sounds/README.md" "$dest/GeneralUser-GS/"
