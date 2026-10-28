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

## Liberation Serif

Times New Roman metric-compatible font, by Red Hat and Google.

- Pinned version: **2.1.5**.
- Source: `liberation-fonts-ttf-2.1.5.tar.gz` from the release at
  <https://github.com/liberationfonts/liberation-fonts/releases/tag/2.1.5>.
- Files: `LiberationSerif-{Regular,Bold,Italic,BoldItalic}.ttf`, and `LICENSE` as
  `Liberation-LICENSE.txt`.
- License: SIL Open Font License 1.1, with the Reserved Font Name "Liberation".

Liberation Serif has the same advance widths and vertical metrics as Times New Roman
for every Latin, Latin Extended, and punctuation character, so score-layout's text
estimates hold for both. The portable PDF writer (Windows and Linux) embeds a subset of
it for score text.
