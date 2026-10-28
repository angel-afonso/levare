# Bundled fonts

Levare embeds these fonts in the executable, so none of them has to be installed.
Every file is the unmodified original.

## Bravura

Music font and engraving metrics by Steinberg Media Technologies GmbH.

- Pinned version: **1.392**, tag `bravura-1.392`.
- Source: <https://github.com/steinbergmedia/bravura/tree/bravura-1.392/redist>.
- Files: `otf/Bravura.otf`, `bravura_metadata.json`, `OFL.txt`.
- License: SIL Open Font License 1.1, in `OFL.txt`.

The app reads the engraving metadata at startup.

## Liberation Serif and Liberation Mono

Times New Roman and Courier New metric-compatible fonts, by Red Hat and Google.

- Pinned version: **2.1.5**.
- Source: `liberation-fonts-ttf-2.1.5.tar.gz` from the release at
  <https://github.com/liberationfonts/liberation-fonts/releases/tag/2.1.5>.
- Files: `LiberationSerif-{Regular,Bold,Italic,BoldItalic}.ttf`,
  `LiberationMono-Bold.ttf`, and `LICENSE` as `Liberation-LICENSE.txt`.
- License: SIL Open Font License 1.1, with the Reserved Font Name "Liberation".

Liberation Serif has the same advance widths and vertical metrics as Times New Roman
for every Latin, Latin Extended, and punctuation character, so score-layout's text
estimates hold for both. It is used for:

- score text on Linux, where Times New Roman is rarely installed;
- text in the PDFs the portable writer produces (Windows and Linux), embedded as a
  subset.

Liberation Mono Bold draws the bar counter on Linux. It is the only mono weight the app
uses.

## Cantarell

Sans-serif interface font from GNOME.

- Source: <https://github.com/google/fonts/tree/main/ofl/cantarell>.
- Files: `Cantarell-Regular.ttf`, `Cantarell-Bold.ttf`, and `OFL.txt` as
  `Cantarell-OFL.txt`.
- License: SIL Open Font License 1.1.

On Linux the interface asks GPUI for the system font. GPUI maps it to IBM Plex Sans and,
when that is missing, to the first installed family in its fallback list (Ubuntu,
Adwaita Sans, Cantarell, Noto Sans, DejaVu Sans…). With none of them installed, GPUI
panics on the first text it draws. Registering Cantarell means that list always finds a
font.
