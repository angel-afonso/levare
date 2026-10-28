#!/bin/sh
# Adds Levare, run from this folder, to this user's applications menu.
# Nothing is copied outside ~/.local/share; Levare itself stays here, where
# it updates itself.
set -eu

here="$(cd "$(dirname "$0")" && pwd)"
# Named after Levare's app id, so the desktop matches its window to the entry.
entry=dev.partitura.editor.desktop
data="${XDG_DATA_HOME:-$HOME/.local/share}"
apps="$data/applications"
icons="$data/icons/hicolor/256x256/apps"
mkdir -p "$apps" "$icons"

cp "$here/levare.png" "$icons/levare.png"
# The desktop entry spec escapes ", `, $ and \ inside a quoted Exec argument.
exec_path="$(printf '%s' "$here/levare" | sed -e 's/[\\"`$]/\\\\&/g')"
while IFS= read -r line; do
  case "$line" in
    Exec=*) printf 'Exec="%s"\n' "$exec_path" ;;
    *) printf '%s\n' "$line" ;;
  esac
done < "$here/$entry" > "$apps/$entry"

update-desktop-database "$apps" >/dev/null 2>&1 || true
gtk-update-icon-cache -q "$data/icons/hicolor" >/dev/null 2>&1 || true
echo "Levare está en el menú de aplicaciones."
