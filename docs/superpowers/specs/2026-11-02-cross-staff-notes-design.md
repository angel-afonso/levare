# Cross-staff notes

Piano music moves notes between the hands' staves all the time: an arpeggio that climbs
from the left hand into the right, a melody shared by both hands. Sibelius and MuseScore
write them with `⌘⇧↑` / `⌘⇧↓` (Ctrl+Shift+↑/↓): the note keeps its hand, rhythm and voice
but is drawn on the other staff, and a beam may cross between the staves. This document
fixes how Levare does it before any code is written.

## Why it is not a small change

- Each staff of each bar is engraved alone (`engrave`, `crates/score-layout/src/lib.rs`)
  and cached by `CacheEntry` in its own coordinates.
- The gap between a piano's two staves is not fixed: `layout_with_spacing` stacks them from
  the ink of each system (high notes, dynamics, lyrics) after engraving.
- A note drawn on the other staff, and above all a beam joining notes of both staves,
  needs that gap. So it can only be drawn after both staves of a system are placed.

## Decisions

### Model

- `Notation::cross: bool`, saved only when set: the note or chord is drawn on the other
  staff of its grand-staff part (piano, organ, harp, celesta, marimba, vibraphone). The
  event stays where it is for everything else: rhythm, voice, playback, selection, copy
  and paste, ties and slurs.
- Validation refuses `cross` on rests and on parts without two staves. A divided part's
  second staff is not a hand and does not cross.
- A chord crosses whole. Splitting one chord between the hands is left out (see below).

### Commands

- `CROSS_STAFF_UP` («Pasar al pentagrama superior», `⌘⇧↑`, `Ctrl+Mayús+↑` elsewhere) and
  `CROSS_STAFF_DOWN` (`⌘⇧↓`), as in Sibelius, on the selected notes of a grand staff.
  Up from the lower staff sets `cross`; down from the upper staff sets it; the other
  direction clears it. Both keys are free in the Sibelius profile.
- In the ribbon, next to Transportar and Invertir in **Inicio › Notas**.

### Layout, in three stages

1. **Engraving stays per staff and cached.** The source staff engraves a crossing note as a
   spacer: it keeps its time position and takes part in beam grouping, but draws no head,
   stem or accidental. `MeasureLayout` gains `crossing: Vec<CrossingNote>` (element, x,
   pitches, duration, voice) and each `BeamGroup` a `crosses` flag; groups that cross are
   not drawn by the staff.
2. **Placement reserves room.** When a system holds a crossing beam, the gap between the
   hands is at least eight spaces, enough for a beam between them; otherwise the gap
   comes from the ink as today.
3. **A system pass draws what crosses** (new `crates/score-layout/src/crossing.rs`), once
   both staves of a system are placed, before `paginate` moves systems:
   - each crossing head is placed by the target staff's clef and key, with its ledger lines
     and accidental, at the target staff's height;
   - a crossing beam is laid over the heads of both staves, with stems from each head to
     it: up from the lower staff and down from the upper one, the beam between them;
   - a crossing note outside a beam takes the stem that points back to its own staff.

   The pass writes its glyphs, lines, beams and `EventPosition`s into the source bar's
   `MeasureLayout` (through `Arc::make_mut`), translated into that bar's coordinates. Hit
   testing, selection colours, the app's painter and the PDF then need no change. Only bars
   with crossing notes are copied, and only in the layout being built; the per-staff cache
   keeps its clean engraving.

### Accidentals

A crossing note's accidental belongs to the staff where it is drawn, as Gould
prescribes: it is read against the target staff's earlier notes in the bar, and later
notes of that staff see it. `measure_accidentals` takes the incoming crossing notes into
account for the target staff.

### Interchange

- **MusicXML export:** the note's `<staff>` is the staff where it is drawn; its `<voice>`
  stays the source voice.
- **MusicXML import:** a note whose `<staff>` differs from the staff its voice otherwise
  uses in that part becomes a crossing note of that voice.
- **Agent:** reads and writes `cross` later, in its own commit.

## Tests

- **Model:** crossing a note keeps its rhythm, voice and sound. Rests and single-staff
  parts are refused. Undo restores the note.
- **Layout:**
  - a crossing head sits at the target staff's pitch height;
  - a crossing beam joins heads of both staves with stems that meet it;
  - the gap between the hands holds the beam;
  - clicking the drawn head selects the note;
  - the per-staff cache is not polluted;
  - every view and the PDF show it.
- **Accidentals:** a crossing F♯ followed by an F in the target staff gets a natural.
- **MusicXML:** export and import round-trip a crossing arpeggio.

## Commits

1. `feat(score): move piano notes to the other staff`: model, commands and ribbon, drawn
   with the system pass for notes outside crossing beams.
2. `feat(layout): beam notes across a piano's staves`: crossing beams and the reserved gap.
3. `feat(file): keep cross-staff notes in MusicXML`.

## Left out

- **Splitting one chord between the hands.**
- **Cross-staff rests.**
- **Crossing into another instrument's staff.**
- **Moving notes between a divided part's staves.**
