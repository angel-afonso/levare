#!/bin/sh
# Adds Levare, run from this folder, to this user's applications menu and
# makes it the app that opens .levare and .partitura scores (and one that can
# open MusicXML). Nothing is copied outside ~/.local/share; Levare itself
# stays here, where it updates itself.
#   ./install.sh               installs
#   ./install.sh --uninstall   removes what it installed
set -eu

here="$(cd "$(dirname "$0")" && pwd)"
# Named after Levare's app id, so the desktop matches its window to the entry.
entry=dev.partitura.editor.desktop
mime_package=levare-mime.xml
data="${XDG_DATA_HOME:-$HOME/.local/share}"
apps="$data/applications"
icons="$data/icons/hicolor/256x256/apps"
mime="$data/mime"

refresh() {
  if command -v update-mime-database >/dev/null 2>&1; then
    update-mime-database "$mime" >/dev/null 2>&1 || true
  fi
  if command -v update-desktop-database >/dev/null 2>&1; then
    update-desktop-database "$apps" >/dev/null 2>&1 || true
  fi
  if command -v gtk-update-icon-cache >/dev/null 2>&1; then
    gtk-update-icon-cache -q "$data/icons/hicolor" >/dev/null 2>&1 || true
  fi
}

case "${1:-}" in
  --uninstall)
    rm -f "$apps/$entry" "$icons/levare.png" "$mime/packages/$mime_package"
    refresh
    echo "Levare ya no está en el menú de aplicaciones. Esta carpeta sigue aquí."
    exit 0
    ;;
  "") ;;
  *)
    echo "Uso: $0 [--uninstall]" >&2
    exit 2
    ;;
esac

mkdir -p "$apps" "$icons" "$mime/packages"
cp "$here/levare.png" "$icons/levare.png"
cp "$here/$mime_package" "$mime/packages/$mime_package"
# The executable's absolute path, not whatever `levare` PATH finds. The
# desktop entry spec escapes ", `, $ and \ inside a quoted Exec argument; %F
# passes the files to open.
exec_path="$(printf '%s' "$here/levare" | sed -e 's/[\\"`$]/\\\\&/g')"
while IFS= read -r line; do
  case "$line" in
    Exec=*) printf 'Exec="%s" %%F\n' "$exec_path" ;;
    *) printf '%s\n' "$line" ;;
  esac
done < "$here/$entry" > "$apps/$entry"

refresh
echo "Levare está en el menú de aplicaciones y abre las partituras .levare y .partitura."
