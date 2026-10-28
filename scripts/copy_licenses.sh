#!/bin/sh
# Copies the licenses of what Levare embeds (its fonts and the GeneralUser GS
# sounds) into a release's licenses folder, which Acerca de Levare › Ver
# licencias opens. sfizz brings its own notices (scripts/build_sfizz.sh).
# Usage: scripts/copy_licenses.sh <licenses folder>
set -eu

root="$(cd "$(dirname "$0")/.." && pwd)"
dest="$1"
mkdir -p "$dest"
cp "$root/assets/fonts/OFL.txt" "$dest/Bravura-OFL.txt"
cp "$root/assets/fonts/Liberation-LICENSE.txt" "$dest/Liberation-OFL.txt"
cp "$root/assets/fonts/Cantarell-OFL.txt" "$dest/Cantarell-OFL.txt"
cp "$root/assets/sounds/LICENSE.txt" "$dest/GeneralUser-GS.txt"
