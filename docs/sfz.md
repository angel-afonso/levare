# SFZ libraries in the Sonido tab

Levare can load several local SFZ libraries, all through the same sfizz engine. Install
each library and its manifest in a folder of its own under one of the `soundpacks`
folders Levare reads:

- its own folder: `Contents/Resources/soundpacks` inside the app bundle on macOS,
  `soundpacks` next to the executable on Windows and Linux;
- the `soundpacks` folder inside Levare's data folder.

An installed Levare reads no other folder: a `soundpacks` folder above the app (such as
`/Applications/soundpacks` or `C:\soundpacks`) is ignored, because those folders may be
writable by anyone. Only debug builds also search
every folder above the executable, so `cargo run` finds the repository's `soundpacks/`.

The app discovers each `manifest.json`, shows the valid packs in the **Sonido** tab of the
right panel and keeps the errors of the invalid ones so you can fix them. GeneralUser GS
stays available as a fallback for instruments or keys that a pack does not cover. The
libraries and their licenses belong to their respective authors.

## One engine for every pack

Every pack plays through the sfizz library that Levare ships (`Contents/Frameworks` in the
macOS bundle, next to the executable on Windows and Linux). A pack never brings code of its
own. In development, `LEVARE_SFIZZ` points to the library to use instead; debug builds also
look for the one in `soundpacks/vsco-ce/`, which `scripts/prepare_vsco.py` installs.

A manifest's `engine` field is a legacy field: older manifests carry it, and Levare
ignores it. It cannot choose or replace the library, even with an absolute path.

## Más sonidos

**Sonido › Más sonidos** installs packs from the catalog that comes inside the app, one at
a time and in the background, without interrupting your writing. Each pack shows its
download size, the space it takes once installed and its license; **Instalar…** shows the
license and **Aceptar e instalar** starts the install. The download is checked against the
SHA-256 published in the catalog, unpacked into a temporary folder and moved to
`soundpacks/<id>` in the data folder only when its manifest is valid; an update that fails
leaves the previous version in place. **Cancelar** leaves nothing half-installed.
**Quitar…** asks for confirmation and deletes only packs installed from here: packs you
copied by hand are not touched.

A pack brings only samples and SFZ text. When it is unpacked, libraries, scripts and any
file that is not audio, SFZ or documentation are discarded, and a path that tries to leave
its folder rejects the whole pack.

There are two kinds of entries. The ones Levare publishes, such as **VSCO CE · Alta
calidad**, are made only from libraries that allow redistribution; they are built with
`scripts/build_soundpack_vsco.py`, which converts the recordings Levare uses to lossless
FLAC and writes a reproducible zip with its SHA-256. Libraries whose license asks not to
be redistributed are installed with a recipe: Levare downloads them from the author's
website, checks the checksum and adds its own manifest. If the author changes the file,
the recipe fails until the catalog checksum is updated.

To publish a pack: build the zip, upload it as an asset of a GitHub release tagged
`sonidos-<id>-<version>` and add its URL, size and SHA-256 to
`crates/sound-store/src/catalog.json`. A new version of a pack raises `version`, and users
see **Actualizar**.

## Format 2 manifest

This example uses verified paths from a local installation of Virtual Playing Orchestra 3.
Paths are relative to the manifest, except the instrument ones, which are relative to
`samples`:

```json
{
  "format": 2,
  "kind": "sfz",
  "id": "vpo",
  "name": "Virtual Playing Orchestra · Performance",
  "samples": "Virtual-Playing-Orchestra3",
  "license": "Virtual-Playing-Orchestra3/Documentation/license.htm",
  "instruments": {
    "Violin": {
      "sustain": "Strings/1st-violin-SEC-PERF.sfz",
      "pizzicato": "Strings/1st-violin-SEC-PERF-pizzicato.sfz",
      "staccato": "Strings/1st-violin-SEC-PERF-staccato.sfz",
      "dynamics": "cc1",
      "spread_layers": false,
      "second": {
        "sustain": "Strings/2nd-violin-SEC-PERF.sfz",
        "pizzicato": "Strings/2nd-violin-SEC-PERF-pizzicato.sfz",
        "staccato": "Strings/2nd-violin-SEC-PERF-staccato.sfz"
      },
      "tremolo": "Strings/1st-violin-SEC-PERF-tremolo.sfz",
      "solo": {
        "sustain": "Strings/1st-violin-SOLO-PERF.sfz",
        "pizzicato": "Strings/1st-violin-SOLO-PERF-pizzicato.sfz",
        "staccato": "Strings/1st-violin-SOLO-PERF-staccato.sfz",
        "second": {
          "sustain": "Strings/2nd-violin-SOLO-PERF.sfz",
          "pizzicato": "Strings/2nd-violin-SOLO-PERF-pizzicato.sfz",
          "staccato": "Strings/2nd-violin-SOLO-PERF-staccato.sfz"
        }
      }
    },
    "Flute": {
      "sustain": "Woodwinds/flute-SOLO-PERF.sfz",
      "staccato": "Woodwinds/flute-SOLO-PERF-staccato.sfz",
      "dynamics": "cc1",
      "section": {
        "sustain": "Woodwinds/flute-SEC-PERF.sfz",
        "staccato": "Woodwinds/flute-SEC-PERF-staccato.sfz"
      }
    }
  }
}
```

| Field | Use |
| --- | --- |
| `format`, `kind` | Must be `2` and `"sfz"`. |
| `id`, `name` | Stable identifier and display name. The id may contain letters, digits, `-` and `_`; `sfz-combinadas` is reserved. |
| `samples` | Root folder of the SFZ files and of the samples they reference. |
| `engine` | Legacy field, ignored. Every pack plays through the sfizz Levare ships. |
| `license` | Optional path; if declared, the file must exist. |
| `instruments` | Keys are the instrument names of Levare's model. |
| `sustain` | Required SFZ for each declared instrument. |
| `pizzicato`, `staccato` | Optional SFZ files for those techniques. |
| `tremolo` | Optional bowed-tremolo SFZ, used for three-bar tremolos. |
| `dynamics` | `"velocity"` by default, or `"cc1"` for modulation-based libraries. |
| `spread_layers` | `false` by default; turns on VSCO's spreading of velocity layers where it applies. |
| `solo`, `section` | Optional recordings for a soloist part or a section part, with the same fields. Without them the main ones are used. |
| `second` | Optional recordings for the second part of the same instrument, such as the second violins, inside the main set, `solo` or `section`. |

`dynamics` and `spread_layers` are inherited when not repeated: `solo` and `section` take
the instrument's, and `second` takes its set's.

## Solo or section

Each string, woodwind or brass part has the **Sección | Solista** selector in its **Mezcla**
row (parts panel, on the left). By default, strings are sections if the score has more than
four string parts and soloists in chamber music; a part named «solista» or «solo», and any
woodwind or brass, is a soloist. The choice is saved in the score and can be undone. If two
parts of the same instrument play the same way, the second uses `second` when the pack
declares it, so two sections in unison do not repeat the same samples; a third goes back to
the main set.

When a part repeats another part's recordings (a second flute without `second`, or the
third violins), it is tuned a few cents apart: +5 for the first repeat, −5 for the next,
then ±9 and at most ±13. Two players in unison are never tuned exactly alike, and two
identical copies of a sample sound like a single one that shimmers. The offset is fixed
for each part, so two playbacks sound the same.

## Tremolo

A three-bar tremolo on strings is not measured: playback still writes thirty-second notes
for any bank, but if the instrument's patch declares `tremolo`, it holds that recording
for the whole note and lets the repetitions pass. One- and two-bar tremolos stay measured
attacks. Without a tremolo recording, or with pizzicato, the repetitions sound as before.

## Comparing sounds

`examples/ab-tremolo-y-unisonos.partitura` brings together flutes and horns in unison and
a string chord with a three-bar tremolo and then a two-bar one. To hear it with a given
pack:

```bash
cargo run --release --example export_audio -- examples/ab-tremolo-y-unisonos.partitura salida.wav vpo
```

The loader checks the engine, the declared license and every declared SFZ. To choose
patches, listen to their real registers and articulations: the file name is not enough.
In VPO, the *Performance* patches use `gain_cc1` as well as `xfin_locc1` crossfades, which
is why `dynamics: "cc1"` fits. Other libraries may use `amp_velcurve` or velocity layers
and need `"velocity"`. The CC1 control is sent before each attack when that voice's level
changes, even on the first attack at level zero. CC11 still controls the transport's
continuous expression.

**Más sonidos** installs Virtual Playing Orchestra 3 with a recipe: it downloads the 3.2
samples (616 MB, hosted on the author's Google Drive) and the 3.3 Performance scripts from
the author's website, checks their SHA-256, unpacks them into the same folder, as the
author asks, and adds this manifest, with soloists, sections, second violins and
tremolos. What follows describes a manual installation.

In the local VPO installation, the Performance SFZ files are in
`Virtual-Playing-Orchestra3/Virtual-Playing-Orchestra3/`, while their WAV files are in the
sibling folder `Virtual-Playing-Orchestra3 2/libs/`. A relative link
`Virtual-Playing-Orchestra3/Virtual-Playing-Orchestra3/libs` pointing to
`../Virtual-Playing-Orchestra3 2/libs` resolves the SFZ's `../libs` references without
moving or modifying the original files. Always check the paths of your own download. The
snare and cymbal patches available in this installation do not cover General MIDI keys 38
and 49, which Levare uses, so they are not declared and those notes play with GeneralUser.

## Priority and compatibility

With two or more packs, **Bibliotecas combinadas** picks the first one that has a patch for
each instrument; if none covers the attack, GeneralUser is used. The priority buttons in
the Sonido tab change that order. The list is saved in `sonidos.json` in the data folder as
`{"version":1,"priority":["vpo","vsco-ce"]}`; new packs not listed are added by identifier
in alphabetical order. Changing the bank or the priority stops playback and does not alter
the score.

The existing VSCO CE pack keeps its format 1 manifest (`kind: "vsco-ce"`) and its original
paths. [VSCO installation and details](vsco.md). Sonatina and other free SFZ libraries can
be added with a manifest of their own, after checking their real paths, note ranges,
articulations and license terms.
