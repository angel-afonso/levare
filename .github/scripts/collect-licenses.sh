#!/usr/bin/env bash
# Usage: collect-licenses.sh <out dir>
#
# Writes the licenses every download ships in its licenses/ folder (macOS:
# Levare.app/Contents/Resources/licenses; Windows and Linux: next to the
# executable), except sfizz's, which come with its build:
#   Levare-LICENSE.txt             Levare's own MIT license
#   THIRD_PARTY_NOTICES.html       the Rust crates built into Levare, from
#                                  cargo-about with about.toml and about.hbs
#   fonts/                         Bravura, Cantarell and Liberation (OFL)
#   GeneralUser-GS/                the bundled SoundFont's license and origin
# Needs cargo-about on PATH. Fails if a crate's license is not accepted in
# about.toml or cannot be read.
set -euo pipefail

root="$(cd "$(dirname "$0")/../.." && pwd)"
out="$(mkdir -p "$1" && cd "$1" && pwd)"

# The files kept in the repository, shared with local macOS bundles.
sh "$root/scripts/copy_licenses.sh" "$out"
(cd "$root" && cargo about generate --locked --fail about.hbs -o "$out/THIRD_PARTY_NOTICES.html")
