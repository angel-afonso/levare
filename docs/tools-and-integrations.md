# Levare: tools and integrations

An inventory of the pieces Levare needs to be a solid alternative to Sibelius: complete
features, high stability and, above all, speed.

> **Status:** reconnaissance inventory (2026-10-17). The first delivery is already
> implemented; see [development.md](development.md) for its scope, the pinned versions
> and the next stages. The sections below keep the full vision.
>
> **★** = no library solves it; it has to be built.

---

## 0. Starting point

The workspace began as a skeleton. State after the first delivery (a snapshot; the
current state is in [development.md](development.md)):

| Crate | Intended role | State |
|---|---|---|
| `levare` (`crates/app-gpui`) | GPUI application | Dark interface with ribbon, automatic ordering by family, visible instrument removal, inspector, transport, zoom and pending controls |
| `score-core` | Musical model | Parts, staves, instruments, chords of up to eight notes, sharps/flats/naturals, major/minor key signatures, ties and slurs, dynamics, crescendo/diminuendo hairpins, articulations, pizzicato/arco, rests, dotted and 3:2 triplet durations, bars and IDs |
| `score-edit` | Commands and undo/redo | Passage selection, copy/cut/paste with ties and marks, transactional edits and persistent snapshots |
| `score-document` | Document/session | Editor, layout cache and save state |
| `score-layout` | Engraving/positioning engine | Automatic beams by beat and triplet, double/partial beams, family brackets, grouped barlines, piano braces, Bravura marks and curves across systems, display scale and compact spacing by ink bounds, key signatures per system, accidentals in columns, chords with a shared stem, piano grand staff, aligned staves, treble/alto/bass clefs and a cache per staff and bar |
| `score-playback` | Interpretation → MIDI events | Dots and triplets on an exact integer clock, hairpins with continuous expression and independent piano hands, persistent pizzicato/arco, persistent dynamics, staccato/accents, tied chains, a common timeline, channels per part and attack recovery when starting from a selection |
| `score-io` | File formats | Versioned native JSON, validation and atomic save |
| `score-platform` (`crates/platform`) | System audio (cpal) | CPAL audio, RustySynth/GeneralUser GS piano, rtrb queue; external MIDI pending |

`assets/fonts` includes Bravura and its license; there are development instructions
and CI in `.github/workflows`.

### Architecture adjustments found and made in the first delivery

- Removed the `score-io → score-playback` dependency; the event interface for MIDI
  will be defined when playback/export is implemented.
- Added the **document/session** layer with score, selection, cursor, history and save
  state. The interface coordinates playback through an immutable plan, stops stale
  plans when editing and keeps the selection separate.
- Added the **input** layer: keyboard shortcuts → editing commands.
- Added the **drawing** layer: `score-layout` geometry → GPUI painting.

---

## 1. Interface

| Piece | Tool | Purpose |
|---|---|---|
| Framework | **gpui** | Windows, events and GPU drawing (Metal on macOS, DirectX on Windows, Vulkan on Linux). The basis of the interface's speed. |
| Widgets | **gpui-component** | Menus, dockable panels, inputs, dialogs and palettes. GPUI only brings primitives. |
| Score canvas | ★ own, on GPUI's painting API | Staves, glyphs, curves, selection and cursor. |
| File dialogs | GPUI's API (fallback: **rfd**) | Open and save. |

## 2. Music typography

| Piece | Tool | Purpose |
|---|---|---|
| Font | **Bravura** or **Leland** (SMuFL standard, OFL license) | Notes, clefs, accidentals, dynamics and articulations. |
| Metrics | The font's JSON metadata + **serde_json** | Exact anchors (stems, edges, recommended thicknesses). Without this the result looks amateur. |
| Outlines | **skrifa** or **ttf-parser** | Extract the glyphs to paint and cache them. |

## 3. Musical core (`score-core`, `score-edit`)

| Piece | Tool | Purpose |
|---|---|---|
| Model | ★ own | Score, instruments, staves, bars, voices, notes, rests, tuplets, ties and slurs, lyrics, chords, key, meter and tempo. |
| Stable IDs | **slotmap** | References to elements that survive edits. |
| Undo/redo | **imbl** (persistent structures) | Cheap snapshots: instant, unlimited undo, and safe reads from other threads (layout, playback). |
| Music theory | ★ own | Transposition, enharmonics, keys, instrument ranges. |
| Instrument database | ★ own data | Names, clefs, transposition, ranges, General MIDI program. (MuseScore's lists are GPL; they can't be copied into an MIT project.) |
| Fast input | ★ command and shortcut layer | Sibelius style: letter = note, number = duration, arrows = move/transpose. It is the heart of "fast to use". |

## 4. Layout engine (`score-layout`): the hardest part

| Piece | Tool | Purpose |
|---|---|---|
| Horizontal spacing | ★ Gourlay-style algorithm | Space notes by duration, with vertical alignment across staves. |
| Line and page breaks | ★ Knuth-Plass-style optimization | Spread systems in a balanced way. |
| Collisions | ★ "skylines" | Avoid clashes between dynamics, articulations, ties, slurs and text. |
| Curves | **kurbo** | Slurs, ties and hairpins as Béziers. |
| Mouse selection | **rstar** (spatial index) | Know which element is under the pointer in microseconds. |
| Parallelism | **rayon** | Compute systems in parallel. |
| Reference | Verovio (C++), LilyPond, MuseScore | Only to study algorithms. Verovio is not interactive; MuseScore and LilyPond are GPL. |

**Performance key:** **incremental** layout. Editing a bar recomputes only the affected
system (and the following ones if the line distribution changes), never the whole work.

## 5. Playback and sound (`score-playback`, `score-platform`)

| Piece | Tool | Purpose |
|---|---|---|
| Interpretation | ★ own | Expand repeats, D.C., D.S. and Coda; dynamics → velocity; articulations → duration; swing; tempo map; synchronized playback cursor. |
| Audio output | **cpal** | Send audio to the sound card. |
| Synthesizer | **rustysynth** (SoundFont, pure Rust) or **oxisynth** | Built-in sound without external dependencies. |
| Sounds | **GeneralUser GS** or **MuseScore_General** (permissive licenses; confirm before distributing) | General MIDI bank shipped with the app. |
| External MIDI | Not chosen yet | Send to other programs or devices and record from a MIDI keyboard. |
| Real-time audio thread | **rtrb** (lock-free queue) + **assert_no_alloc** | Audio never drops out: the audio thread neither blocks nor allocates. |
| Professional sound (later phase) | **clack** (CLAP host) or VST3 hosting | Use third-party sound libraries. |

## 6. Files (`score-io`)

| Format | Tool | Purpose |
|---|---|---|
| Own format | **serde** + **postcard**/JSON inside a **zip**, with schema versioning | Fast saving; files keep opening in future versions. |
| MusicXML | **quick-xml** | Import and export with Sibelius, MuseScore, Dorico and Finale. Essential to be a real alternative. |
| MIDI | **midly** | Read and write `.mid` files. |
| PDF | **krilla** | Print-quality printing and export. |
| SVG/PNG | **tiny-skia** + **resvg** | Export images. |
| Audio | **hound** (WAV), **flacenc** (FLAC) | Export the mix. |

## 7. Stability

| Piece | Tool | Purpose |
|---|---|---|
| Autosave and recovery | ★ own | Lose no work when the app closes unexpectedly. |
| Layout tests | **insta** (snapshots) | Catch any unintended visual change. |
| Model tests | **proptest** | Random sequences of edits and undos that must never break invariants. |
| Robust importers | **cargo-fuzz** | A malformed MusicXML or MIDI file must never bring the app down. |
| Diagnostics | **tracing** (+ optional **sentry**) | Structured logs and crash reports. |
| CI | GitHub Actions | Build, `clippy`, tests and benchmarks on every change. |

## 8. Measurable performance

| Piece | Tool | Purpose |
|---|---|---|
| Profiling | **tracy** (through `tracing-tracy`) | See the cost of each frame and each layout phase. |
| Benchmarks | **divan** or **criterion** | Protect fixed budgets in CI. |

Proposed budgets (to be confirmed):

- Pressing an input key → note visible on screen: **< 16 ms**.
- Redrawing the visible score: **< 8 ms** (120 fps).
- Opening a large orchestral score: **< 1 s**.
- Playback start: **< 50 ms**.

## 9. Additional features

| Piece | Tool | Purpose |
|---|---|---|
| Individual parts | ★ own | Extract parts from the full score, with their own layout and linked to the original. |
| Plugins / scripting | **mlua** (Lua) or **wasmtime** (WASM) | The equivalent of Sibelius' ManuScript. |
| Languages | **fluent** | A translatable interface. |
| Preferences | **directories** + **toml** | Settings and per-system paths. |
| Distribution | **cargo-packager**, signing and notarization (macOS), auto-update | Installers and updates. |

---

## Risks

1. **GPUI is young:** its API changes between versions and it has little documentation.
   Pin the version (as `Cargo.toml` already does) and keep its use inside `levare`
   (`crates/app-gpui`) so the rest doesn't depend on it.
2. **The layout engine** holds most of the real work: it is what separates a
   professional score from an amateur one.
3. **Licenses:** MuseScore and LilyPond are GPL. They are for studying ideas, not for
   copying code or data. Check the license of each font and SoundFont before
   distributing it.
4. **Real-time audio:** any lock or allocation on the audio thread causes dropouts; it
   has to be designed that way from the start.

## Settled questions

- Platforms: macOS, Windows and Linux alike; CI builds, tests and opens the app on all
  three, and each release ships for all three.
- Sound: built-in sounds (GeneralUser GS and VSCO CE); external MIDI is still pending,
  with no library chosen (an unused `midir` dependency was removed).
- Scope of each delivery: set by [development.md](development.md).
