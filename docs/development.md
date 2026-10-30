# Levare development

First delivery started on 2026-10-17 from `tools-and-integrations.md`.

The sections follow the order of the deliveries. Schema and clipboard version numbers
are the ones each delivery introduced; today the native schema is 29
(`score-io::SCHEMA_VERSION`) and the clipboard format is 6. Shortcuts are written with
the macOS glyphs; on Windows and Linux ⌘ is Ctrl and ⌥ is Alt, and
[shortcuts.md](shortcuts.md) lists every default per platform.

## Goal of this delivery

Close the minimum working loop: open the application, write a melody, select and
correct notes, undo, and save/open the document. Levare is built, tested and released
for macOS, Windows and Linux alike; the core crates have no platform code, and the
platform code in the app sits behind `cfg(target_os = …)` with a branch for each
system.

## Module boundaries

```text
levare (crates/app-gpui: GPUI, input, painting, dialogs, file tasks)
 ├─ score-toolbar (presentation model of the ribbon and Archivo; no GPUI)
 │   └─ score-keymap
 ├─ score-keymap (commands, profiles and shortcuts; no GPUI)
 ├─ score-document (session and saved state)
 │   ├─ score-edit (transactional commands and history)
 │   │   └─ score-core (model and invariants)
 │   └─ score-layout (geometry and per-bar cache)
 │       └─ score-core
 ├─ score-export (score and parts as PDF and MusicXML)
 │   ├─ score-pdf (print layout through Quartz or krilla)
 │   └─ score-io
 └─ score-io (.levare container, versioned native schema, MusicXML, recovery)
     ├─ score-core
     └─ atomic-write (safe file replacement, shared by every writer)
```

The interface also coordinates `score-playback` (immutable plans and tempo) and
`score-platform` (synthesizer, queue and device output). The audio thread never
receives the editable document or a reference to GPUI. The agent panel lives in
`agent-host` and `agent-tools`, updates in `levare-update`, and sound packs in
`score-sound-vsco` and `score-sound-store`.

GPUI is used only in `crates/app-gpui`. The `score-io → score-playback` dependency is
gone: when MIDI arrives, an event exchange will be defined that doesn't require
loading the playback engine.

## Decisions implemented

- **Integer musical time.** A whole note takes 192 units, avoiding floating-point
  errors. Durations go from the whole note to the thirty-second note, with a single dot
  and the 3:2 triplet (see *Dotted and triplet durations*).
- **Identity.** `slotmap 1.1.1` assigns generational keys inside each bar. The full
  reference is `(part ID, staff, bar index, key)`. Parts have persistent IDs that don't
  change when another part is removed. Appending bars and deleting notes keep the
  references of the other elements. Inserting and deleting bars (`score-core::measure_edit`,
  the `InsertMeasure` and `DeleteMeasures` commands) remaps every reference that holds a
  bar index in one operation: connections, piano arpeggios, comments and the per-bar
  maps. Bars have no identity of their own, so reordering them would need one; there is
  no command to reorder bars.
- **Snapshots.** `imbl 7.0.2` shares the sequence of bars and their indices. Each bar
  shares its event store through `Arc`; an edit copies only that small store. Undo
  restores score, cursor and selection. The history lives in memory and is freed when
  a document is opened or created.
- **Atomic commands.** An error publishes no changes and doesn't alter the history.
  Editing after undo discards the redo branch.
- **Layout.** Independent geometry, measured in logical units, up to four bars per
  system (six in part views) and minimum widths by density. Initial spacing
  proportional to the square root of the duration. A cache compares each bar's content
  and width; a change in distribution can invalidate other bars. Grouping and placing
  systems still walk the whole score. Slurs and hairpins find their notes in an index
  built once per layout, not by walking every bar; the same index of slur lanes serves
  spacing and hairpins. The editor gives each state of the score a revision, and
  `Document` keeps the last view by revision, mode and scale (and width, except in page
  view, whose width is the printed one). It shares that view without copying it while
  they don't change: playback and the pointer don't engrave the work again, and a zoom
  step rescales the engraving instead of laying it out. Playback keeps the frame's
  timeline until the revision, the tempo or the repeat mode changes; the playback
  position is computed once per frame and the ruler, transport and canvas use it. The
  canvas only paints what reaches the visible area (bars, slurs, hairpins, labels,
  braces and connectors). Reference: the 32-part, 84-bar orchestra draws at 60 fps also
  with `cargo run`, whose profile lightly optimizes the workspace and fully optimizes
  the dependencies.
- **Typography.** Bravura 1.392 and its metadata are included with the OFL. Thicknesses,
  notehead width and ledger line extension come from the JSON. `ttf-parser 0.25.1`
  extracts and caches the original SMuFL outlines, which are painted as GPUI paths with
  non-zero winding fill. This avoids a GPUI 0.2.2 limitation on macOS: it refuses fonts
  that lack the Latin letter `m`. Its composite glyphs resolve stems and initial flags.
  Each glyph is tessellated once per size and orientation with the same lyon and
  options as GPUI; painting an instance only translates its triangles.
  The ribbon and the right panel share these outlines for dynamics, accidentals,
  articulations, note values, dot, triplet, fermata and hairpins. The controls center
  the real ink box (compound dynamics included), keep the shortcuts in the interface
  font and use editorial italics for `pizz.` and `arco`.
  The native macOS menus use the same Bravura file for dynamics, articulations, hairpins
  and accidentals (including those of the key signatures). Their titles combine native
  text with glyphs whose ink box is clipped, to avoid SMuFL's large vertical metrics. The
  symbols are drawn at screen resolution and take the color of the menu state; actions,
  shortcuts and accessible labels keep their original names. The formatting is applied
  again when the key assignments change.
  The `music_font` repertoire is shared by engraving, UI and menus. Mixed texts convert
  Unicode accidentals to Bravura glyphs and inherit the text's size and color; letters,
  names, voice numbers and terms such as `pizz.` keep their text typeface. Braces use
  Bravura's original variants and brackets their terminals; extensible lines, slurs and
  hairpins use the font's engraving metrics. The tests walk every mark, the 30 key
  signatures, each musical group of the ribbon and the example scores to catch symbols
  without a loaded outline or replaced by generic icons.
  Score text is set in Times New Roman on macOS and Windows and in the bundled Liberation
  Serif on Linux, which rarely has Times and whose font matching wants exact names; both
  have the same widths, so layout estimates hold (`app-gpui::fonts`). The bar counter
  uses Menlo, Consolas or the bundled Liberation Mono. On Linux Levare also registers
  Cantarell, so GPUI always finds an interface font from its fallback list.
- **Files.** Save and Save As write a `.levare` file (`score-io::container`): a ZIP
  whose first entry, `mimetype`, is stored uncompressed and reads
  `application/vnd.levare.score`, so the file identifies itself, followed by
  `score.json`, deflated. `score.json` holds the versioned JSON envelope (`format`,
  `version` and `score`), written compact: indentation alone made orchestral scores four
  times larger. An 84-bar orchestral score goes from about 3 MB to under 20 KB. Unknown
  entries are ignored, so later versions can add to the container.
  Files are recognized by their contents, not their extension (`score-io::open`): a ZIP
  whose `mimetype` names Levare is a `.levare`; a file whose first non-blank character
  is `{` is the JSON of an older `.partitura`; anything else goes to the MusicXML
  importer, including compressed MXL, whose `mimetype` names MusicXML. Reading caps the
  file at 64 MiB, the decompressed `score.json` at 256 MiB whatever the ZIP headers
  claim, and the container at 64 entries. Plain JSON is capped at 256 MiB, enough for
  the largest score the model allows (48 parts of 4096 bars). `.partitura` files keep
  opening and Save never overwrites them: saving a score that came from a `.partitura`
  file or from a MusicXML import asks for a new `.levare` beside it, named after the
  original. A name typed without an extension gets `.levare` (a dot inside a title such
  as «Op. 2» doesn't count as one). The schema and the invariants are validated before
  opening; a file written by a newer Levare is refused with a message that asks to
  update. File errors have a kind (not found, no permission, read-only, damaged or not a
  score, newer version, too large, disk full, other) and a Spanish message; the
  technical cause goes to the log.
  Scores, exports, the recovery file and the settings files (shortcuts, panels, recent
  files, updates, sound priority) are replaced through the `atomic-write` crate: a
  temporary file in the same folder, flushed to disk, then renamed over the target. The copy takes the replaced
  file's permissions, a symbolic link is written through and stays a link, the folder is
  flushed after the rename on Unix, and on Windows the rename is retried for about two
  thirds of a second while another program (OneDrive, Defender, the indexer) holds the
  file. A read-only target is refused.
  Schema 7 adds per-event marks and slurs with stable endpoints to schema 6, which
  stores parts, instruments, staves, pitch sets, accidentals and an initial key
  signature. Schema 11 adds maps of meter and key changes by bar index; earlier schemas
  keep their initial values with no changes in between. Schema 12 adds an initial tempo
  and per-bar markers; earlier ones use quarter = 120 with no changes. Formats 1–6 open
  without expression marks; 1–5 use C major to keep their previous sound. Format 4
  migrates with natural pitches. Schema 3 migrates each note to a list of one pitch and
  each rest to an empty list, keeping their IDs. Opening schema 1 creates a piano part;
  opening schema 2 keeps each instrument. Both keep the original piano notes in `Upper`
  and add an empty `Lower`. Schema 15 adds the second event order per bar and divided
  parts; schemas 1–14 open with one voice and no division. Migration never modifies the
  original file.
- **Interface.** Small controls built from GPUI primitives; `gpui-component` waits until
  panels, forms and palettes need it. Save/open use the system's file dialogs (on Linux,
  through xdg-desktop-portal; without one, Levare says so in Spanish) and do the reading
  and writing off the interface thread. While an operation is pending, editing is
  blocked so a consistent version is saved.
- **Recovery.** `score-io::Recovery` owns a serial writer and a system lock
  (`File::try_lock`) on `recovery.lock`, a separate file beside the recovery file, so
  the recovery file can still be replaced while the lock is held (Windows refuses to
  rename over a locked file). The system releases the lock when the process ends,
  crashes included; a second Levare window finds it taken and runs without autosave.
  Edits queue shared snapshots; 750 ms debounce with a 3 s maximum. The writer keeps
  `recovery.partitura` in the data folder: a versioned envelope with the native
  document and the original path, through `atomic-write`. The document is the same
  compact JSON as `score.json`, not compressed. Save/discard queue the cleanup on the
  same writer; no earlier write can resurrect a discarded copy. On start, the old
  native recovery without an envelope is also accepted and restored as a modified
  document. Errors show in the interface; the original file only changes on an explicit
  save.
  When the recovery file can't be read or restored, it moves aside before the first edit
  autosaves over it: to `Recuperación sin abrir <date>.partitura` in the same folder, a
  name nothing in Levare writes to, and a window («No se pudo recuperar la partitura sin
  guardar») says where it is and can show it. If it can't be moved, autosave stays off
  for the session so the file is left alone.
  A quit from outside the window (the Dock, logging out) skips the question about
  unsaved changes, and GPUI 0.2.2 can't cancel it: `on_app_quit` runs once the quit is
  decided. The app callback holds a handle to the writer, writes the autosave one last
  time and waits for it (`app-gpui::autosave`). When that fails, or there is no
  autosave, the unsaved score is saved as `Copia de emergencia <date>.levare` in the data
  folder, never over another file, and added to «Abrir reciente». Restarting into an
  update also writes the autosave and waits for it; if it fails, the update stays for
  the next start and the status line asks the musician to save first. Dates in these
  names are UTC, as `2026-10-28 15-30-12`, without the colons Windows forbids.
- **Log and crash reports** (`app-gpui::diagnostics`). Release builds have no console on
  Windows, and a Mac app opened from Finder only reaches the unified log, so warnings
  and failures go to `registro.log` in the data folder as well as to stderr: one
  timestamped UTC line each, from any thread, never failing or panicking. Past 1 MiB the
  log becomes `registro.1.log`, replacing the previous one. A panic hook, installed
  before GPUI starts, writes a crash report, `fallo-<Unix seconds>.txt`, with version,
  system, thread, location, message and backtrace, and keeps the ten newest. On the
  next start Levare says once that it closed unexpectedly («Levare se cerró
  inesperadamente»), after any other question it asks at startup, and can show the
  report; `fallos.json` remembers the newest report already announced. If the window
  can't open (no usable GPU, a broken driver), the reason is logged and reported,
  Windows shows a message box, and Levare exits with an error. Autosave, recovery,
  emergency copy and file errors are logged too. Under `cfg(test)` the log only writes
  to stderr, so tests that walk these failure paths leave the musician's log alone.
- **macOS development.** The `runtime-shaders` feature is on by default because the
  machine has Xcode but not the Metal Toolchain. Ahead-of-time compilation is available
  with `--no-default-features --features vsco` once that component is installed. The
  `vsco` feature (the VSCO CE adapter) is also on by default; without installed samples
  it doesn't appear and doesn't change the sound.

Versions are pinned in `Cargo.lock`. The latency targets in `tools-and-integrations.md` are
still proposals: they haven't been measured yet.

## Ensemble score

`Score` keeps a sequence of parts with stable identity and instrument. Each part has
staves with identity `Upper`/`Lower`; the piano keeps both and the other instruments
only `Upper`. All of them have the same number of bars. Note IDs include the staff to
tell apart identical local keys in the two hands. Adding or removing a part is a
transactional edit, with cursor, selection and history. The interface lets the musician
choose it from the parts list, by clicking its notes or staff, and with ⌥↑/⌥↓
(neighboring staff). Changing part shows its staff. The last part can't be removed.

Engraving shares a grid of musical starts and ends per bar across all parts. Notes that
start at the same instant align by time, not by note index; noteheads of different
widths are also centered. Treble, alto and bass clefs place the real pitches, with names
on the left and a line joining the staves of each system. The width adapts to the
window; the parts of a bar keep the same width. The cache includes part ID, content,
clef, grid and width. Changing a pitch doesn't recompute the other parts; changing the
rhythm can affect the shared grid. The piano grand staff has treble/bass clefs, a brace
and joined barlines; its cache includes the staff's identity. Key signatures are
computed per part through `written_key_at`.

The audio plan keeps intervals per staff, channels per instrument and a common clock.
Both hands share channel and timbre. Overlapping unisons become a single keypress until
the last end, avoiding double attacks and a NoteOff from one hand cutting the other. The
search for sustained notes is prepared in the control and delivered in a fixed array of
up to 1,536 notes (96 staves × 2 voices × 8 pitches, `MAX_SOUNDING_NOTES`); it doesn't
walk the score in the callback. Restored notes are deduplicated by channel/key.
Polyphony is reserved when the synthesizer is created for 256 voices, RustySynth's
maximum, including sample layers and release tails; with large ensembles voices may be
stolen. The total duration is the latest end among all parts. When starting from a
selection, notes of other parts that span that instant are also attacked and released
at their original end. This keeps their remaining duration; it doesn't yet rebuild the
previous point of the sound's envelope. Notes of all parts are highlighted at the same
time. Following shows the whole ensemble if it fits vertically; in tall scores it
follows the part being worked on and uses another active part during its gaps. Before
following a single staff it tries to keep both hands of the active piano together. The
parts list and clicks update the followed part without restarting the transport.

## Chords

`Event` keeps a duration and `Pitches`, an ordered set of up to eight distinct diatonic
positions in fixed storage. An empty list is a rest. Adding or removing the highest
pitch keeps the ID, the duration and the cursor; the commands are transactional and
reversible. Transposition affects the whole chord. The event selection keeps a pitch
index in the history to apply accidentals to a single note. Each staff accepts two
independent rhythmic voices.

Engraving uses independent SMuFL noteheads, a shared stem and a flag by duration.
Seconds alternate the side of the noteheads and reserve room in the shared grid.
Selection and following cover all their noteheads. The plan schedules each pitch at the
same instant and keeps the deduplication of unisons between hands. Audition delivers up
to eight keys in a fixed command; starting from a selection restores every sustained
pitch without allocations in the callback. Format 10 also opens formats 1–9.

## Accidentals

`Pitch` keeps its diatonic spelling and an `Option<Accidental>`: sharp, flat or natural.
`None` follows the current key signature; `Natural` forces the natural pitch with an
explicit natural sign, even if the key signature alters that degree. MIDI conversion
applies ±1 semitone; diatonic transposition keeps the accidental. Chords allow one note
per diatonic position and accept enharmonics between different positions. Enharmonic
unisons are deduplicated both in the plan and in audition.

Engraving resolves signs per bar and staff. The effective pitch is resolved with the
key signature and the note's own sign. After a single accidental, the right
cancellation is printed if the next note returns to the key signature at that position.
Explicit signs are kept even when they repeat. The columns leave 35 units of vertical
separation and the grid reserves room before each start, also before the first. Following
includes their glyphs. Selection tells noteheads apart by pitch and marks the note that
takes the accidental, without changing the rhythmic selection used for playback. The
input selector is independent from the command that alters the chosen note.

## Meter and key signature per bar

`KeySignature` validates −7 to 7 fifths and tells major from minor mode; relatives share
accidentals. The default is C major. A `Pitch`'s own sign prevails over the key
signature; `None` inherits the degree in every octave. Changing the key reinterprets the
notes that inherit, keeps explicit spellings and doesn't transpose their positions.
`SetKeySignatureAt` and `SetTimeSignatureAt` take part in the history without changing
IDs, cursors, selection or the chord's note index. The private maps `key_changes` and
`time_changes` keep markers after bar zero; `key_signature_at` and `time_at` resolve the
inheritance. Each change affects every part up to the next marker. Bar zero keeps the
initial fields of older files.

`measure_start_tick`, `measure_at_tick` and `total_ticks` share the position arithmetic
with the editor, clipboard, audio and transport. A meter change validates the capacity
of every staff before modifying the document. If any bar exceeds the new duration, it
fails atomically. Connections are validated again when the rhythm or the effective pitch
of their endpoints changes. When pasting, notes are split by the local meter and keep
their sound across key signatures.

`Pitches::in_key` resolves a fixed copy without allocations. The audio plan resolves both
its messages and the sustained intervals; audition applies the same method before
converting to MIDI. The callback receives prepared pitches. The notation starts each bar
with the state of the key signature and draws cancellation signs when a note's own
accidental changes. Clef and key signature repeat at the start of each system. The
reserved space comes before the time signature and the first note, and the cache
includes the key even when switching between two key signatures of the same width.
Intermediate changes reserve room for the new signature and the naturals that cancel the
previous key signature, also when they coincide with a system break. Treble, alto and
bass clefs are supported at concert pitch.

The interface offers 15 keys per mode, an editor in the right panel and a native menu.
**Armadura** is the input with no accidental of its own; **Seguir armadura** restores it
on the chosen note. The names of the selected notes show their effective pitch. Both
selectors can apply from the start or from the chosen bar and remove a marker. `T` opens
the meter and `K` the key. There are no different key signatures per part nor automatic
raised degrees for harmonic/melodic minor.

The signature change tests cover inheritance, initial changes that keep later markers,
atomic refusal of overfull bars, history, ties across keys, clipboard, schema 10
compatibility, engraving in both hands and across system breaks, audio timing with a
fermata, and search in a last bar of 1/16. The check in the window confirmed the meter
panel and the initial key signature; automated click control was intermittent and didn't
allow completing the manual sequence of intermediate changes.

## Tempo markings

`score-core::TempoMark` keeps one-line text (at most 80 characters), a reference note
value and BPM (20–400). `TempoBeat` accepts half, quarter and eighth, dotted or not. The
initial tempo and `tempo_changes` are inherited per bar in every part. `SetTempoAt` and
`RemoveTempoChange` are history transactions that keep notes, IDs, cursor and selection.
Schema 12 validates and keeps these markers; earlier schemas open with quarter = 120.

Engraving reserves a top band and width for each explicit marking. `Layout.tempos` stays
outside the cached local geometry; its positions follow the translations of systems and
pages. The note value is painted with Bravura; the text keeps its text typeface and hands
any musical symbols to the same Bravura repertoire. Hit testing lets the musician open
the selector from the written marking.

`Timeline` integrates segments of seconds per tick, split at tempo changes and at the
ends of fermatas. `frame_at`, `seconds_at` and `tick_at` share this map. The reference
note value sets the duration of each beat; the `Tempo` argument of the audio API acts as
a relative speed (120 = follow what is written). Hairpins are subdivided at those same
boundaries to keep their musical slope. The audio callback still receives a prepared
plan without allocations.

The header and the transport open the bar shown; Texto → Tempo opens the cursor's bar.
The selector fixes that target while typing, and lets the musician switch to the start
without losing the original target. The −/+ buttons edit the tempo shown and keep the
tick when stopping the previous plan. Text fields use GPUI's native handler with
selection, clipboard, IME and horizontal scrolling; their keys don't run music-writing
commands.

In an isolated graphical session, «Andante cantabile» and 84 BPM were typed in the native
fields, applied with Enter, and the Bravura marking above the staff was checked. A
temporary file was saved and its version 12, text, note value and BPM were verified. The
tests cover reopening and compatibility, per-bar changes, history, dotted note values,
fermatas, inversions of the clock and crescendos over tied notes that cross different
tempi.

For isolated graphical tests, `LEVARE_DATA_DIR` can name an alternative absolute folder
for recovery, recent files, shortcuts, panels, the log and crash reports, without
changing score files or the usual session's data. `PARTITURA_DATA_DIR` is kept as an
alias for older scripts.

## Built-in playback

The second delivery lets the musician hear what they write and play the score:

- **Input:** after a valid insertion or transposition, a 250 ms audition sounds with the
  active instrument; it keeps its natural release tail. Rests and refused commands
  trigger no notes. Audition can be turned off.
- **Transport:** Space or the button starts from the selected note/rest, or from the
  beginning if there is no selection. Space toggles pause/resume; Esc or Detener cancel
  the transport and keep the selection. While stopped, Esc clears the selection. Editing,
  undoing, changing tempo, opening or creating a document stops the previous plan and
  discards the pause.
- **Pause and selection:** the plan's intervals keep absolute positions. The selected
  start is found before Play is queued; the callback skips messages before that
  boundary. During the pause it outputs zeros and freezes both the musical clock and the
  whole synthesizer state, including its sample block, phase and envelopes. Resuming
  doesn't need to rebuild the plan nor to trigger the note again. Pause also works while
  Play is still queued. Stop invalidates the paused transport, even with a full queue.
  Changing the selection while paused doesn't move its position: to jump to another
  note, stop and start again.
- **Rhythm:** `score-playback` converts musical units to sample positions with integer
  arithmetic, using quarter notes per minute. It keeps the gaps of incomplete inner bars
  and leaves out empty trailing bars. It includes intervals to highlight notes and rests
  without changing the selection. At the same time boundary, NoteOff comes before NoteOn.
- **Sound:** `rustysynth 1.3.7` and the original GeneralUser GS 2.0.3 bank, with revision
  and SHA-256 in `assets/sounds/README.md`. Its own license is kept. The catalog of 54
  profiles selects General MIDI programs and percussion notes. Up to 48 parts use
  independent routes of 128 internal channels, spread over eight preallocated
  synthesizers. Local channels 9/15 are skipped for parts; host channel 15 is kept for
  audition. External MIDI is still pending.
- **Device:** `cpal 0.16.0` opens the default output and adapts the stereo mix to its
  rate and format. It supports 32/64-bit floats and 16/32-bit integers, mono/stereo and
  extra muted channels. Without a device or after an error, editing goes on and
  reconnecting the audio is offered. A recoverable stream error (ALSA after a suspend,
  say) gets one reopen on the same renderer, keeping the sounds and the place. On
  Windows, which keeps a stream on the device that was default when it opened, Levare
  moves to a new default output while nothing plays; macOS and Linux follow the
  system's choice by themselves. The `audio_thread_priority`
  feature gives the audio thread real-time priority on every system (Linux reaches it
  through D-Bus). The playhead subtracts the output latency the device reports, so it
  stays with the sound.
- **Real time:** `rtrb 0.4.0` carries commands in a fixed queue. The callback owns its
  synthesizer and buffers; it doesn't block, open files or do interface work. Plans are
  prepared outside the callback, and the control keeps a strong reference until the
  audio releases its own, so that destroying the vectors also happens outside the
  callback. Owners are deduplicated when the same plan is used again. Stop changes an
  atomic generation and cancels even starts still in a full queue.
- **Checking:** `assert_no_alloc 1.1.2` intercepts allocations and frees in the tests of
  the real callback with the bundled bank: playback, audition, completion, stale plans,
  saturation, concurrent commands and output conversion. It also compares sample by
  sample the resumption of a paused note against uninterrupted playback, and checks the
  pause before the first callback, inside a rest, and its cancellation by Stop. The
  `audio_smoke` example verified the real output at 44,100 Hz, starting from a selected
  note, the clock frozen during the pause, its resumption and completion. Physical
  latency and underruns haven't been measured yet.
- **Controls:** 120 BPM initially, range 30–240, volume and audition configurable during
  the session. Changing tempo stops the plan; changing volume applies on the next block.
  These options aren't part of the native file yet.

The view automatically follows the active note or rest on both axes, with a margin for
the highlight, stems and flags. It only scrolls the canvas when the element leaves the
visible area. It uses the canvas's real painted origin and the area of the scrollable
container, so it includes the title, the padding and size changes without relying on
fixed heights. It compensates scroll changes between frames to avoid double advances.
Pausing or stopping releases manual scrolling; resuming shows the active note again.
This following doesn't modify the selection or the document. The tests cover successive
lines, returning to an earlier note, wide bars, extreme pitches, rests and the limits of
the view.

The interface queries the sample clock every 16 ms to update the highlight and the view
following. That timer neither schedules nor generates audio.

`app-gpui::playhead` converts that absolute clock to ticks, bar and time to share one
position between score, ruler and counter. The layout keeps the shared rhythmic grid and
interpolates between its anchors, also in gaps without events. The playhead and its thin
line are painted before the musical ink and replace the two earlier indicators.
Horizontal following tracks that position; the pause keeps the clock and allows manual
scrolling. Active notes and rests change color without individual backgrounds; stems
identify their event to share the color, while staves and ledger lines stay neutral. The
ruler shares its eight-pixel margin geometry with click detection and snaps the position
to the nearest denominator beat. The far end picks the last playable beat. `Playback`
keeps that tick while stopped without changing the editing cursor or the selection; the
transport's plan includes the implicit rests up to the end of the ruler.
`Control::play_from_frame` uses the same prepared start as playback from a selection,
recovering sustained notes and expression. A jump keeps the playing or paused state;
stopping or editing discards the position chosen on the ruler. The piano's natural
release may carry on briefly when the last note ends. Events reach the synthesizer at the
computed boundaries, with the internal granularity of its 64-sample block.

## Dotted and triplet durations

`Duration` holds a `NoteValue` (whole to thirty-second note) and two booleans: one dot
and the 3:2 triplet. The whole note takes 192 ticks and the quarter 48, so both
modifications and their combination resolve without fractions or rounding. The minimum
value is 4 ticks (`Duration::MIN_TICKS`, a thirty-second triplet). The maximum capacity of
16/1 allows 768 events per voice (`MAX_EVENTS_PER_MEASURE`), and the validation limit
derives from these values. Frames are computed from the absolute tick, avoiding drift
accumulated over long chains of triplets. Older scores keep their real timing.

Plain durations keep their earlier JSON string; modified ones use
`{value, dotted, triplet}` with validated fields and types. Schema 10 opened 1–9; the
native format doesn't store ticks, so changing the resolution doesn't require rewriting
bars, links or IDs. Clipboard 4 does convert the offsets of 1–3 by multiplying them by
three; those old formats refuse the new modifiers. The application layer normalizes on
reading and the editor also accepts old passages.

`SetDotted` and `SetTriplet` act on the whole selected range atomically, keep the range
and update the capacity through `Score::set_duration`. The input settings keep the
modifiers when the base note value changes. Pasting keeps note values unsplit; when
crossing barlines it uses a bounded decomposition table for compatible plain or triplet
values, with ties and marks at their corresponding ends. Gaps use the full palette. A
boundary that can't be represented produces an error and leaves no partial events or
bars. No hidden durations or rounding are used to make the passage fit.

Engraving adds Bravura dot and number-3 glyphs. Chord dots use a column with distinct
vertical spaces, never on a line; spacing reserves their width. Local groups gather up to
three consecutive notes of the same modified value; a new bar or a change of value
separates them and incomplete groups stay visible. Brackets sit over the original ink of
the notes and marks, without feeding back on earlier brackets. Gaps between dense
groups, ink bounds, cache, hit testing, zoom and following are kept. Nested groups and
triplets with mixed values aren't modeled yet.

Validation of this delivery: 149 workspace tests passed, format and Clippy without
errors. Exact ratios, atomic history, range preservation, chords/rests, tied chains, no
drift over 96 bars of triplets, old/new clipboard, unrepresentable boundaries without
partial effects, the duration codec and saving the maximum bar of that time (384 events,
with sixteenth notes as the shortest value) were checked. The engraving tests include
chord dots with seconds and eight dense groups without steps or contact between
brackets. In a temporary app without session recovery, the drawing, the dot and triplet
shortcuts, the menu, undo/redo and the file saved in schema 10 were verified. The
CoreAudio test at 44,100 Hz aligned two mixed bars with the left hand, with
pause/resume. The final review left no important problems.

## Automatic beams

`score-layout::beaming` derives the groups from the written rhythm and the meter,
without modifying the model, native schema 10, clipboard 4 or the audio plan. Each staff
is processed separately. Simple meters group by the denominator unit; compound ones by
three units. The short irregular meters use 5/8 = 3+2 and 7/8 = 2+2+3; editing these
subdivisions by hand is pending. Rests, long values, notes that cross a beat and
barlines interrupt beams. Triplets reuse the groups of equal values from rhythmic
engraving and are never joined with the neighboring group.

The plan fixes a common stem direction before shifting the seconds of chords. Engraving
replaces the full glyphs with noteheads and stems that reach a beam of limited slope,
leaving room above the whole chord. Sixteenth notes generate full or partial secondary
beams. GPUI paints the polygons with the selection and playback colors; ink bounds,
articulations, dynamics, slurs and following include the new stems and beams. A complete
triplet under one shared beam shows the number 3 without a redundant bracket; incomplete
groups or groups with rests keep the bracket.

The existing cache already includes note values and meter, so an edit rebuilds the
corresponding group. The tests cover both staves, simple, compound and irregular meters,
mixed dotted rhythms, rests, triplets, chords with seconds, treble/alto/bass clefs, ties,
selection and following with zoom. The example `examples/barras.partitura` gathers these
rhythms for visual inspection.

Validation: 157 workspace tests passed, format and Clippy without errors. The
independent review found no important problems. In a temporary app without session
recovery, the drawing of eighths, sixteenths, mixed dotted rhythms and triplets with
single and double beams was checked, including up and down stems and both piano hands.

## Hairpins and dynamic curves

`ConnectionKind::{Crescendo, Diminuendo}` shares the stable endpoints of the slur model.
It accepts notes, chords or rests on one staff, ordered in time; hairpin intervals can't
overlap and may touch. Deleting an endpoint removes the link; the history restores it.
Schema 9 keeps these kinds and migrates from 8 without changing the connections.
Hairpins entered the clipboard in format 3; the current format 4 still reads 1–3, copies
inner hairpins and remaps their end to the first fragment if the end note is split when
pasting. Outer hairpins aren't copied, like slurs; their starting dynamic is
approximated by the nearest mark. The context on insertion uses the curve's left
boundary.

`score-core::DynamicEnvelope` prepares linear segments per staff between the written
marks. Without an arrival mark, the target is two positions of `Dynamic::ALL` above/below
the hairpin's last mark, limited to ppp–fff. The final level persists. Inner marks are
anchors and explicit marks prevail over the graphic direction.

`score-playback::dynamics` calibrates the velocity of each attack to the maximum of its
sounding interval and computes CC11 as `level / reference * 127`. Outside the curves,
this keeps exactly the earlier velocity and full expression. A sustained crescendo
reserves only the amplitude it will need before releasing the note; articulations keep
their boost. The times of each CC transition are obtained by inverting the original
slope, before limiting it to 127, so that rests and staccato gates don't artificially
stretch the curve. The plan recovers the current expression and the original attack when
starting inside a tie. Pause freezes the curve, the envelopes and the synthesizer's
clock.

The two staves of piano, harp, celesta and organ with hairpins use separate internal
routes. Each group of 14 parts takes 32 channels: the main route skips local channel 9
and the lower one adds 16. Eight banks cover the 48 parts without reusing channels. The
banks share a clock, reset on stop and are mixed without allocating memory in the
callback. When a plan starts, CC11 of every route is restored to avoid leftover volume
from another score.

Engraving produces two strokes per hairpin segment, with a continuous opening across
systems, below notes, slurs and dynamics. The reserved room is computed per row without
invalidating the local engraving of bars. Following includes the active note's hairpin
and works with zoom. Ribbon, menus and the H/Shift+H shortcuts share commands; a hairpin
can be created from a passage selection or by marking start/end in two steps.

Validation of this delivery: 139 workspace tests passed, format and Clippy without
errors. The tests verify audio of sustained and tied chords, crescendos and diminuendos,
restoring volume and timbre outside the curve, gain references across rests/staccato,
starting from a selection, exact pause and 28 hand routes in 14 pianos. In the temporary
app without session recovery, the strokes, creation in two steps with H, creation over a
passage with Shift+H, removal, undo/redo and saving were checked; the file kept the
endpoints in schema 9. The CoreAudio test at 44,100 Hz played a tied cello with
crescendo, diminuendo and pause/resume. The final review left no important problems.

## String techniques

`Notation.technique: Option<PlayingTechnique>` stores `Pizzicato` and `Arco` on notes,
chords or rests. Only instruments of the `Strings` family accept these marks. Schema 8
keeps the technique; migrating from 7 leaves the marks empty and keeps the original
sound. The setter and the validation of files and insertions share this restriction. The
history keeps each edit.

The plan resolves the technique per staff and prepares program changes before the
attacks: pizzicato uses GM preset 45 (zero-based), arco restores the instrument's preset.
The changes take no extra channels. Tied notes keep the program and velocity of their
original attack, starts from a selection included. Messages are ordered release →
program → attack. The callback only receives prepared data and doesn't allocate.
Audition uses the current technique. The tests check the bank's real preset, sound
differences, exact resumption after a pause, isolation per part and the four strings.

Engraving uses italic serif text labels, with horizontal room and ink bounds of their
own above stems and articulations. Slurs follow the outer outline of these labels.
Clipboard version 2 keeps the inherited technique and places the mark only on the first
note when it's split on paste; it still reads version 1 with arco by default. A paste
into an incompatible instrument fails before modifying the document.

Validation of this delivery: 127 workspace tests, Clippy and format without errors. In a
temporary app without session recovery, the quartet, the italic labels and the switch to
arco from the native menu were checked; the saved file kept `Arco` in schema 8. The
CoreAudio output was checked at 44,100 Hz with cello pizzicato, pause/resume and return
to arco. The code review found no important or critical problems.

## Musical clipboard and passage selection

`score-edit::selection` keeps a stable anchor and focus per staff inside the editor's
snapshot. Clicking and normal navigation go back to a simple selection; Shift+click/arrows
extend the range and Seleccionar todo walks the active staff. Cortar and «Eliminar notas
y su tiempo» remove the range; Delete turns the notes into rests and keeps the time. The
history restores anchor, focus and events.

`app-gpui::selection` turns a drag of at least five pixels into the same editor range. It
can start on a note or on the space of a written bar; a brief click keeps the earlier
single selection and input cursor. The target is approximated by line and time inside
the source staff, regardless of the pitch of the notes or of other instruments. Zoom and
the canvas's shifted origin are taken into account when finding the ends. Releasing the
button or running a command ends the gesture. Painting joins contiguous bars into one
band per line, empty bars in between included, and the selection bar shows the count
with Copiar/Cortar actions. The menus include the clipboard actions and Seleccionar todo
with their current shortcuts.

`score-edit::clipboard` exposes a portable, versioned passage: events with offsets in
ticks, source key signature, current dynamic and indices of inner slurs. It contains no
document IDs. Pasting clones the snapshot, keeps the pitches across a change of key,
turns gaps into rests and splits the notes that cross barlines, creating ties. The ends
of slurs are remapped to the new notes. Dynamics/accent stay at the start of a split
note; staccato/tenuto go at the end. Playback applies the final gate to the whole chain
to keep the original sound. The operation is only committed when all the music and its
links are valid; a capacity or link collision leaves no added bars or partial edits.

The GPUI layer uses `ClipboardItem` with a text summary and Levare's own JSON metadata,
limited to 16 MiB and validated before pasting. It keeps no internal clipboard that could
paste old data after copying text from another application. Cut verifies that the
written content reads back before deleting the notes, except on Wayland, which answers
with the previous offer: there Cut only checks that a Levare window has focus, since
GPUI's Wayland client writes the clipboard only then. The actions share
registry, menus, ribbon and configurable shortcuts. They don't act on the score while
Preferencias is open.

## Score order and grouping

`InstrumentFamily` and `Instrument::score_order` share the presentation order between
editor, parts list and engraver. `Score::arrange_parts` sorts whole parts stably: it keeps
IDs, the voices of both hands, events and connections. The session normalizes the order
on opening and editing applies it when adding instruments. The saved snapshot uses that
same order to avoid a false changes indicator. The file schema wasn't modified: the
sequence of parts was already serializable.

The engraver computes the limits of each family per system after placing the staves. It
generates one bracket per group and extends barlines only within that family; the
opening barline runs through the system. Piano braces stay separate from brackets. The
outlines are painted with scaled display coordinates and note positions keep their hit
testing.

Removing from the parts list runs the same registered command as the native menu. At
least one part is kept and the neighbor is selected after removing. The history recovers
the whole part and its slurs. Playback stops on edit and rebuilds its map of
channels/timbres from the new order.

## Reference interface

`app-gpui::ui` holds the presentation: dark palette, shared components, tabbed ribbon,
parts panel (left), Selección · Sonido · Revisión panel (right), position strip, sheet,
transport and status bar. `ui::icons` draws the icons as native vectors. `main.rs` keeps
the actions, the dialogs, the document state and its safeguards. The look follows the
reference the user provided; controls without an available musical function are marked
«Pendiente». The interface colors are in `ui/theme.rs`, with their contrast checked.

### Side panels

Design in `docs/superpowers/specs/2026-10-27-panels-design.md`. Each panel answers one
question and no panel repeats what another already shows. With the ribbon there is
repetition on purpose: the passage and note actions in Selección run the same commands as
their ribbon buttons, at hand next to what is being looked at.

- **Left, «what is in the score?»** (`ui/instrument_panel.rs`, `ui/parts_view.rs`,
  `ui/mixer.rs`). `parts_view` computes without GPUI the families in score order, their
  rows (name and a note on clefs and transposition) and the search filter, which ignores
  case and accents and shows the families unfolded. Lista and Mezcla share the rows;
  Mezcla adds `mixer::part_controls` (volume and pan, per session; Sección | Solista as
  `Command::SetPlayers`, which is saved and undone).
- **Right, «what do I have selected and what do I change on it?»**. `RightTab` tabs:
  Selección (`ui/selection_view.rs` computes without GPUI what is selected from the
  editor: score, note, rest, passage or open editor; `ui/selection_panel.rs` draws it and
  hosts the clef, key, meter, tempo, text, lyrics, chord symbol and information editors),
  Sonido (`ui/sound_panel.rs` with `ui/mixer.rs` for the output) and Revisión
  (`ui/review.rs` and `ui/comments.rs`). Opening an editor shows the panel and the
  Selección tab. A click on the score or a navigation key closes, through the same path
  as `Esc` (`cancel_editor`), the editors for which `EditorKind::survives_selection_move`
  is false; the own advance of lyrics and chord symbols doesn't go through there.
  Removing a mark from its ✕ row runs the mark's command and is undone like any edit.
- **Top** (`#score-readouts` in `ui.rs`): the only position strip (Compás, Tiempo, Tempo,
  Métrica, Tonalidad, Parte · Voz and a summary of the selection). Tempo, Métrica and
  Tonalidad open their editor in the right panel.
- **Bottom** (`ui/status_bar.rs`): `status_bar(&Facts)` returns only what the bar says
  (status message, page navigation in Página view, zoom, autosave and save state); a test
  names every field so that voice, key, meter, instruments or view mode don't come back
  unnoticed.
- **Memory** (`panels.rs`): `paneles.json` in the data folder keeps the open panels
  (`show_parts`, `show_right`), the right tab (`right`), Lista or Mezcla (`parts_mode`)
  and the folded families (`folded`). `panels::load` returns the defaults if the file is
  missing or damaged (missing fields take them too); `panels::save` writes through
  `atomic-write` and is only called when something remembered changed since the last
  write.

Shortcuts follow the Sibelius profile described in **Shortcut engine**; the labels of the
ribbon, the panels and the menus are read from the active map.

The layout keeps its geometry and cache in musical coordinates. `Layout.scale` applies
when painting, selecting, placing the cursor and following playback; bars aren't copied
or rewritten when the scale changes. The compact mode of `LayoutEngine` computes vertical
bounds with the real boxes of Bravura, stems and ledger lines; it keeps them with the
cache and reserves a margin between staves. The standard mode keeps its earlier spacing.
`score-layout::views` adds three presentations on the same engraving. Continua adapts the
width of the systems to the window; Pentagrama removes the breaks and uses natural widths
for every bar. Página keeps a width of 1000 units and a √2 sheet ratio, with an opening
header, margins, page number and space between sheets. An indivisible system that is
wider or taller enlarges every sheet of the document consistently. Pagination keeps both
piano hands and whole families together.

`Layout.systems` records bar ranges and vertical bounds; `pages` records the sheets and
their ranges. The translation moves notes, labels, braces, brackets and barlines from
their original positions in a single pass. Curves and hairpins are recomputed afterwards
with the final systems. The cache keeps the local geometry without the page position.
The header uses the same scale as the score. The canvas paints only the bars that cross
the visible area.

In the app, the selector without TODO, Continua, the switch to Página with its counter and
Pentagrama with five aligned instruments were verified. Window control failed
intermittently when testing the full sheet fit; that manual interaction stayed
unconfirmed. The distribution, selection and navigation across several pages at 28 %,
70 % and 150 % are covered by automated tests.

Views are interface state: they don't change the document, history, IDs, selection,
cursor or audio clock. Page navigation centers the sheets that fit and aligns large ones
to the top margin, with scroll limits. `⌘0` switches to Página and fits the sheet to the
window. The tests cover breaks, large sheets, translation of ornaments, hit testing with
zoom, connections across sheets, view changes and back-and-forth navigation without
accumulated drift.

The visible clock is updated from the position already published by the audio control;
the interface neither queries nor modifies the synthesizer to draw it.

## Toolbar

The parts panel lives in `ui/instrument_panel.rs` (what it lists, in `ui/parts_view.rs`)
and the add dialog in the same file. The dialog keeps a draft of quantities per
instrument, limited to the free places among the 48 parts; neither selecting nor
cancelling modifies the document. Confirming runs `Command::AddParts` as one
transaction: if one addition fails, none is kept, and Undo takes the whole set back. The
modal context blocks note input and global commands until confirming or cancelling.
The button of the chosen part removes by `PartId` through `RemovePartById`, keeping the
chosen passage and note when they belong to another part. If the active part is removed,
its neighbor is chosen; Undo recovers the original staves and connections.

The ribbon and the Archivo menu follow layers with dependencies pointing down:

```text
ui/toolbar.rs (GPUI)  paints ToolbarView and MenuRow; each click emits a command
   ▼
score-toolbar         TABS and FILE_MENU (data) · project() · fit() · project_menu()
   ▼
score-keymap          registry (labels, pending) and active map (keys)
AppView::run          single point of execution · ui/command_state.rs implements CommandStates
```

- **Description** (`spec.rs`): eight tabs by task (`TabId`: Escribir, Articulaciones,
  Dinámicas, Texto, Partitura, Reproducción, Revisión, Vista), their groups and controls.
  Each control is a `CommandId` with a size (large or tile), a function (`Role`: choice,
  toggle or action), a glyph, a short name and a hint. It contains no status texts or
  keys.
- **State** (`CommandStates`): the only question the model asks the app: whether a
  command is active, enabled or has a temporary label («Terminar», «Pausar»), never wider
  than the control's name: the tile measures its name and doesn't move. The app answers
  it in `ui/command_state.rs`; the tests use a fake.
- **Projection** (`project`): joins description, registry, map and state. Each control is
  a tile: glyph and, below, its name and its key. It measures with fixed values
  (`Metrics`, widths per character that cover the widest text of the ribbon). With room,
  a group widens and shows the names; without it, `fit` removes them (glyph and key; the
  large control of a group keeps its own) and then folds groups by priority (the lowest
  priority and, on a tie, the rightmost) until the row fits. The ribbon never scrolls; a
  folded group is a button with a dropdown.
- **Hover hint**: the ribbon's controls and the right panel's commands show a short hint
  with their name and the active assignment after GPUI's delay (500 ms). The dropdowns
  and Preferencias keep the keys as a reference; the panels no longer repeat the whole
  list of shortcuts.
- **Archivo** (`FILE_MENU`): one list generates the window's dropdown and the native
  macOS menu, with the «Abrir reciente» submenu. GPUI doesn't draw native menus on
  Windows and Linux, so there the dropdown also lists the update commands, Preferencias
  and, last, Salir de Levare; the other commands that only live in native menus are
  reached through the command search.
- **Recent files** (`score-io::RecentFiles`): `recientes.json` in the data folder, at most
  10, without duplicates, newest first; it forgets files that no longer exist. It is
  updated on opening and saving, and when Levare keeps an emergency copy.

The tests cover that each control points to a registered command, that labels, keys,
state and pending flags come from their source, that every tab fits unfolded at 1420 px,
the folding order, the Archivo menu, the persistence of recent files and that the music
font has every ribbon glyph preloaded.

## Shortcut engine

`score-keymap` doesn't depend on GPUI and gathers everything that was previously spread
over `bind_keys`, a hand-made key translator, fixed labels in the ribbon, the shortcuts
panel, the menus and the README:

- **Command registry** (`commands.rs`). Each command has a stable ID
  (`"entrada.nota.c"`), category, label, scope, whether it repeats when the key is held
  and a *ready* or *pending* status. The `id::` constants are used in the profile, in the
  interface and in the single `match` of `AppView::run`, so a mistyped ID doesn't compile.
  Future extensions can register commands of their own.
- **Keys** (`keys.rs`). Its own model of keystroke and sequence, with names reserved for
  the numeric keypad (`numpad4`, `numpadenter`) although GPUI 0.2.2 doesn't tell them
  apart. A shortcut is stored with `secondary` (⌘ on macOS, Ctrl elsewhere) and labeled
  per platform: `⇧⌘Z` on macOS, `Ctrl+Mayús+Z` elsewhere. `Layout` translates `⇧1`–`⇧9`
  to the character the active layout produces, because GPUI reports ⇧2 as `"` (Spanish)
  or `@` (US) and drops Shift. The app asks the system for those characters
  (`UCKeyTranslate` on macOS, `ToUnicodeEx` on Windows; on Linux, where GPUI keeps its xkb
  state to itself, a table of common layouts by name, with US as the fallback) and
  installs the shortcuts again when the layout changes.
- **Profile** (`profile.rs`). *Sibelius · portátil*: Sibelius' *Notebook* set, which works
  the same with or without a numeric keypad. It was checked against the official
  Sibelius 2018.4 tutorial (3 eighth, 4 quarter, 5 half, 7/8/9 ♮/♯/♭, 0 rest, `.` dot,
  ↩ tie, F7–F12 layouts, ⇧1–9 intervals in laptop mode); the other single-letter keys
  follow the SibAccess list (Berklee). Keys without a function in Levare are reserved as
  *pending*. Defaults differ per platform only where conventions do: macOS keeps
  Sibelius' ⌥⌘ combinations, while Windows and Linux use Ctrl+Mayús instead, because
  Ctrl+Alt is AltGr on Windows and often a desktop shortcut on Linux; full screen is F11
  there, and Linux replays with Alt+Mayús+Space, since desktops keep Alt+Space.
- **User changes** (`user.rs`). `atajos.json` in the data folder
  (`~/Library/Application Support/Levare` on macOS, `%LOCALAPPDATA%\Levare` on Windows,
  `~/.local/share/Levare` on Linux) keeps, per command, the full list that replaces the
  profile's; an empty list leaves it without a shortcut. A shortcut saved with ⌘ on macOS
  uses Ctrl elsewhere; one with Ctrl+Alt and a character is refused when recording on
  Windows and Linux and skipped there when loading, and Preferencias says so. If the
  older `Partitura` folder exists and `Levare` doesn't yet, the app keeps using the older
  one to preserve shortcuts, recent files, panels and recovery. In the same folder,
  `paneles.json` remembers the panels (see *Side panels*). Atomic writing and a version
  number.
- **Resolution** (`keymap.rs`). Profile + changes + layout give the active shortcuts.
  The user's win; conflicts, unknown IDs and keys that can't be detected are reported in
  Preferencias instead of failing.
- **Documentation** (`docs.rs`). [docs/shortcuts.md](shortcuts.md) is generated from the
  registry and a test fails if it goes stale.

In the app, `shortcuts.rs` turns each shortcut into a GPUI `KeyBinding` with a single
action, `Run(CommandId)`, and the context predicate of the command's scope: `Levare` for
global commands, `Levare && !Modal && !TextInput` for score commands (the single keys)
and `Levare && Modal` to close Preferencias. The root adds `Modal` while Preferencias is
open. The native macOS menus only show combinations with ⌘/⌃ (`MenuRun` for the rest,
with the key in the text), because a single-key equivalent would let macOS fire the menu
before the contexts and the Preferencias recorder. A GPUI interceptor sees each
keystroke before the shortcuts: it marks automatic repeats (commands that toggle don't
repeat) and hands the combination to the recorder.

**Shortcut preferences** (⌘, or the *Atajos* button) list the commands by category with
their keys, status and whether they were changed. They allow changing, adding, removing
and restoring one or all. Recording a key that is taken asks whether to reassign it.
There is no text search yet. (When this was written GPUI 0.2.2 had no text field; Levare
now has its own, `ui/text_input.rs`.)

Pending: a *Sibelius · escritorio* profile with numeric keypad (requires GPUI to tell
those keys apart), the F7–F12 layouts of the on-screen keypad, multi-keystroke sequences
in the recorder and a search in Preferencias.

## Musical expression

`score-core::expression` defines `Notation` (optional dynamic and articulation) and
`Connection` (tie/slur with two serializable `ElementId`s). A tie requires musical
adjacency, no gaps and the same set of positions and resolved semitones; an explicit and
an implicit ♮ are equivalent. Phrases are ordered and don't overlap within a staff.
Files are validated, and links invalidated by edits are removed, preserving the undo
snapshots.

`score-layout::expression` engraves Bravura marks outside noteheads and stems and
generates curves per system. It reserves lanes for curves and room under the tie line for
the ascenders of forte. The curve geometry is computed after placing the bars; local
marks follow the incremental cache. The ends follow the musical identity when lines are
redistributed or the zoom changes.

`score-playback` keeps the written `Span`s for selection and following, and generates
distinct sounding intervals for staccato and tied chains. Each staff has its persistent
dynamic. Ties keep a single attack; unisons between hands are combined before computing
the attack intervals per channel/key. Recovery from a selection uses the intensity of
that effective attack, even if the original event ended before the later unisons. The
pause keeps the synthesizer's state and introduces no allocations in the callback.

The UI offers controls in the ribbon and native menu, and removes them from the
Selección tab. ↩ toggles only the outgoing tie; S marks the start/end of a phrase and Esc
cancels. The provisional state is cleared on edit, undo, redo, or when creating or
opening another document. The four articulations have no default key (as in Sibelius
laptop) and can be assigned in Preferencias. Ties between single pitches inside
different chords are still pending.

### Mark catalog

`score-core::marks` describes each mark once. `MarkKind` lists the 19 marks (four
articulations, fermata, eight dynamics, pizz./arco, tie, slur, crescendo and diminuendo)
with their short and long name, their family (`MarkFamily`), their Bravura glyph, the
button symbol and the staff side (`Placement`). A note carries at most one mark of each
point family; `set_point_mark` replaces the one of its family and `Command::ToggleMark`
writes, changes or removes the selection's.

`MarkRef` identifies a written mark (the note of a point mark or the connection of a
slur or hairpin) to select it, delete it (`Command::RemoveMark`) or change it
(`Command::ReplaceConnection`). `score-layout` generates a `MarkBox` per point mark, fitted
to the Bravura boxes, and `Layout::hit` chooses between note and mark.

`score-toolbar::marks` is the only mark ↔ command table. The ribbon (`spec::mark`), the
Selección tab, the **Expresión** menu, the menu symbols, the highlighted state and the
app's dispatch (`apply_mark`) walk it instead of repeating each mark. A test requires
each mark to have its own command, ready and with its name in the label.

To add a point mark: its variant in `MarkKind` (and the field in `Notation` if it is a
new family, with `serde(default)` so the format doesn't change), its command in
`score-keymap` and `score-toolbar::marks`, its placement in
`score-layout::expression::marks` and, if it sounds different, its rule in
`score-playback`.

### Fermata

`Notation.fermata` is only stored when active, so older files open without migration. It
accepts notes and rests, independent of the articulation; when pasting a note that is
split it stays on the last fragment. It is drawn centered above the note, outside the
articulation, and pushes the technique up.

In playback, `score-playback::Timeline` turns musical time into audio: the stretch of
each fermata lasts `FERMATA_HOLD` (twice as long) for **every** part, and what follows
starts afterwards. Simultaneous fermatas of several parts merge into one stretch. The
plan, the hairpins, the ruler's jumps, the clock and the transport's seconds use the same
timeline (`tick_at` is its inverse).

## Verification

The clipboard was checked in a temporary app with the piano example: Shift+arrow
selected two tied half-note chords; they were copied with ⌘C and pasted into a ninth bar
of the left hand. They were cut, the cut was undone/redone and they were pasted again.
The saved file kept C4/E4/G4 in both chords, their mp dynamic and the tie between the new
IDs. Refusing an occupied target without modifying the score was also checked. The
shortcut filter allows repeating Pegar for consecutive pastes. The automated tests cover
reverse selection, passage history, gaps, aligned growth, crossed bars, slurs, keeping
the sound across keys, current dynamic, refusing external content and rollback. Playback
of a split staccato note matches the original in attacks, intensity and MIDI timing. The
independent review found no important problems.

Score grouping was checked with a deliberately reversed temporary copy of
`cuarteto-en-sol.partitura`: on opening, flute, violins, viola and cello showed with
separate woodwind and string brackets. The window showed the persistent «Eliminar
seleccionado» control. From the native menu an oboe and C4 were added, the part was
removed and its removal undone/redone. The saved file confirmed the oboe with its note,
placed after the flute. The tests cover stable order, identity of events and
connections, undoing parts, normalization and saving, brackets per system, barline
breaks between families, piano braces and spatial selection. The independent review
found no concrete problems.

The expression delivery was checked visually with `examples/piano-expresion.partitura`:
curves, three ties of a chord, dynamics in both hands and the four articulations. In a
temporary app a ninth bar C4–C4–E4 was written, a tie and a phrase were added with T/L,
staccato with S and ff from the native menu. After undoing/redoing the dynamic, the saved
file kept the three notes, the marks and both ends of the slurs in format 7. The tests
cover transactional refusal, restoring links with history, reading schemas 1–6, curves
across systems, room under forte, no new attacks in chains, unisons between hands and
intensity recovery. Audio continuity with pause is compared sample by sample without
allocations in the callback. The independent review checked the fixes and found no
important problems. `audio_smoke` completed slurs, dynamics and articulations together
with both hands, flute and cello on the real output at 44,100 Hz, with selection, pause
and resumption.

The reference interface was checked in a separate temporary app, with
`examples/cuarteto-en-sol.partitura`: five visible staves, repeated instrument names,
panels, ribbon and transport. With N off, input added no notes; with it on, 5 produced a
sixteenth chord and 4 an eighth rest. The native dialog saved this data, including a
single flat. After fixing how the keyboard character was read, `=` applied and saved a
natural on the selected note. The tests also check selection and following with zoom,
the separation of extreme chords in compact spacing and the new durations. The
independent review found no important defects.

The G major key signature was checked visually with its piano example: a sharp in both
clefs, room before the time signature and cancellation/restoration on the F notes. In a
temporary session E♭ major was chosen from the native menu, the change was undone and
redone, and chords were written in a fifth bar. The native dialog saved format 6, three
flats, C4/E4/G4 without their own accidental in the right hand and D3/F3/B♮3 in the left.
Reading the file confirmed key, IDs and the single natural. The tool's screenshots went
stale after the interactions; the modified key signature was verified through the file
and the engraving tests. `audio_smoke` completed D major with both hands, flute and cello,
selection, pause and resumption on the real output at 44,100 Hz. The tests cover the 30
key signatures, every octave, treble/alto/bass, repetition per system, restoration,
cache, history, validation, migration of 1–5 and sustained notes. The independent review
found no concrete defects.

The accidentals delivery was checked with `examples/piano-alteraciones.partitura`:
sharps, flats, cancelling naturals, signs in chords and columns for seconds. In a
temporary session a fifth bar was written in both hands and ⌥S/⌥F/⌥N, ⌥← and ⌥0 were
used to alter single notes, besides undo and redo. The native dialog saved C♯4/E♭4/G♮4
and C3/E♯3/G♭3 with their accidentals. The independent review found no concrete defects.
The tests cover single selection, cancellations per bar and staff, dense columns,
following, format 5 with migration of earlier versions, MIDI semitones, enharmonics and
exact audio continuity without allocations. The real output at 44,100 Hz completed the
test of altered chords, selection, pause and resumption.

The chords delivery was checked with `examples/piano-acordes.partitura`: noteheads,
shared stems, shifted seconds and both hands. In a temporary session C–E–G was written
in the fifth bar of each hand, with Shift+letters in the right hand and Acorde mode in
the left. The native dialog saved two quarter-note chords, C4/E4/G4 and C3/E3/G3, on
their staves. The view showed the new system. `audio_smoke` verified chords in both
hands together with flute and cello on the real output at 44,100 Hz, with a selected
start, pause and resumption. The automated tests cover limits, duplicates, history,
migration and saving, stems, seconds, selection, following, unisons and exact
resumption of the audio without allocations or frees in the callback.

The grand staff was verified with `examples/piano-dos-manos.partitura`: treble/bass
clefs, brace, shared barlines, alignment and selecting the left hand with ⌥2. In a
temporary session a fifth bar was added, C3 was written in the left hand and E4 in the
right with ⌥1, and it was saved through the native dialog. The resulting file kept both
notes on their staves. `audio_smoke` checked both hands together with flute and cello on
the real output at 44,100 Hz, with a selected start, pause, resumption and completion.
The tests include identity per staff, history, migration of schemas 1/2, malformed
files, unisons, separation of positions and 14 pianos with 28 active hands. The callback
keeps the zero-allocation check.

The ensemble delivery adds tests for identity between parts, growth of bars, selecting
and undoing instruments, migration of schema 1, saving schema 2, clefs, alignment across
durations, minimum spaces, independent channels and starting during sustained notes of
other parts. The 13 timbres of the catalog of that time have a verified preset in the
bank and produce different audio. The pause of an ensemble is compared sample by sample
with uninterrupted playback, under the callback's zero allocations/frees check.

`examples/trio.partitura` was checked visually, with the instruments panel, aligned
staves, treble/bass clefs and simultaneous highlighting of the three parts. `audio_smoke`
verified piano, flute and cello on the real output at 44,100 Hz, including a selected
start, pause, resumption and completion.

The tests cover bar capacity, out-of-range notes, stable references, isolation of
snapshots, transposition across octaves, history, transactional refusal of changes,
edit/undo/redo sequences, save state, invalid and future files, real saving to disk,
spatial selection and reuse of the layout. CI runs format, tests and Clippy on macOS,
Windows and Linux, and opens the app on all three.

Manual check of the delivery:

1. Run `cargo run`; enter `C D E F G` and check the bar break.
2. Choose an eighth with `3`, enter `A B`, insert a rest with `0`.
3. Select a note with a click, transpose with the arrows and undo/redo.
4. Save, create a new score and open the saved file.
5. Try to close with changes and check that cancelling keeps the document.

Note and rest input, the bar break, the typography, saving with the native dialog,
creating a new document, reopening the example and cancelling the unsaved-changes
prompt were verified visually. Spatial selection and history are verified by tests. The
specific manual test of quitting from the Dock and recovering on restart is still to be
done.

In the audio delivery, writing with audition, starting with Space, highlighting during
playback, automatic completion and stopping with Esc were verified in the window.
`audio_smoke` also checked the progress and completion of the stream on the machine's
real output at 44,100 Hz.

For following, a temporary 32-bar score was checked visually: starting from an
off-screen selection showed the active note, and the advance to later lines kept the
highlight visible. The selection stayed in its original position. The checks of manual
scrolling during the pause and of horizontal limits are covered by tests; the window
control tool didn't allow completing those manual interactions.

## MVP block: editing, recovery and PDF

`score-core::measure_edit` holds the structural edits: insertion and deletion across all
staves, reassigning event indices and the meter/key/tempo maps, connections and piano
arpeggios. When deleting local changes, the effective state of the first retained music
is kept. Deleting everything creates a single empty bar. The editor stores each operation
in a snapshot. `set_durations` validates the final totals and only then updates
durations and prunes connections, avoiding losses from intermediate states of the
passage.

Clicking a note turns input off. A–G replaces the chosen pitch, keeping identity and
duration; note values act on the selection. Delete removes the sound, keeping the time
and the marks valid for rests. The deletion that shortens the time is still available
for Cortar and the Edición menu.

`score-pdf` is an adapter without GPUI: it builds `ViewMode::Page`, maps each sheet to A4
and draws lines, beams, curves with Bravura metrics, hairpins, texts and the font's
original outlines through one painter. macOS prints through Quartz/CoreText with the
system's Times New Roman; Windows and Linux use the portable writer (`krilla`) with the
bundled Liberation Serif, which has the same widths, so the pages match. Musical texts
share `score-layout::repertoire` with screen and menus; ordinary text is shaped in whole
runs to keep Unicode. The number of pages is validated before the atomic replacement
through `atomic-write`. The target requires a PDF extension and the interface prevents
replacing the open document. Background export keeps the snapshot and the save state.
Packaging the app wasn't part of this block; [releasing.md](releasing.md) describes it.

Tests: pitch replacement/collisions, deletion keeping time and deletion shortening time,
overflowing duration, insertion/deletion with local changes and slurs, undo, recovery
writing, session exclusion, ordered cleanup, errors that keep the previous copy and a
child process killed by force before reopening. PDF checks piano, strings and multi-page
documents, as well as invalid targets and preserving files. Visual check through
Poppler: piano with arpeggios across hands and a two-page quartet.

## Two voices and divisi

`Voice::{One, Two}` identifies two sequences per bar with a common store of `EventId`.
`order` keeps the original voice and `second_order` adds the second without changing old
IDs. `timed_iter` computes the start within each voice; the bar's capacity is the maximum
of its durations, not the sum. Validation refuses duplicate IDs between sequences and
overflows of each voice. Cursor, range selection, clipboard, input, deletion and
navigation keep the voice. Rhythmic connections and ties must join events of the same
voice; phrases of different voices can overlap.

Engraving uses a shared time grid, opposite stems, beams and triplets per voice,
separate rests and horizontal shifting of coinciding seconds. Accidentals are resolved
chronologically for the whole staff. When clicking coinciding unison noteheads the active
voice is preferred. Lower articulations and fermatas use their original Bravura glyphs.
Texts and dynamics reserve room in front of the stems of both voices.

`Part::divisi_changes` (schema 17) stores boundaries by bar and local tick. Migrating
schema 16 turns each bar change into a change at tick zero. `DivisiPoint` normalizes the
end of the bar to the start of the next one; ranges are half-open and end when the last
selected note finishes. `div.`/`unis.` and the passage commands use these same positions.
Deleting and inserting bars remap whole maps without losing their inner offsets; the
deletion keeps the initial state of the first retained bar.

The document keeps its voices, note values and identities. In each bar with a divisi
interval, voices 1 and 2 are shown on separate full staves; «div.» and «unis.» are placed
at their real ticks. Presentation per bar avoids line breaks, artificial continuations
and diagonal ties for sustained notes. `display_source`, `displayed_staff` and
`VoiceRegion` share that criterion for engraving, cursor, insertion, selection and
following. Time boundaries stay in the document's map and don't affect the audio. The
cache keeps the regions and the written boundaries, also when a mark doesn't change the
state. No temporary copy with fragmented note values is needed. Cuts are still validated
on the available rhythmic grid before the operation is committed; the original slurs and
hairpins keep their IDs.

Divisi changes don't force system breaks: a row can hold bars with one or two staves.
Each entry of the lower row reserves its clef and key signature; inactive bars create no
cells or click targets. The layout of ensembles of three or more parts uses a staff of
about 5.3 mm on A4. Bars reserve width according to the ink of the markings, long
dynamics included. Hairpins share the height of the dynamics when it fits, respect low
notes and leave horizontal gaps around the dynamics.

Inherited event markings are cleared when setting a passage so they don't come back when
overwriting or removing boundaries. Schemas 1–16 keep the earlier presentation. Legacy
parts with `Part::divided` keep their two physical staves; editing a passage normalizes
them to the two voices, remapping selection and endpoints in one transaction. It refuses
to merge more than two voices and to convert a piano. Dynamics, hairpins and techniques
stay shared by the section; the piano's hands keep their independent expression.

The playback plan creates one track per voice with its own intervals, attacks and
sustains, aligned on the common clock. Preparing the expression considers simultaneous
sounds and keeps the ranges per track; groups that overlap reserve a common margin. The
active elements of up to 192 tracks (`MAX_STAVES` × 2 voices) and up to 1,536 sustained
pitches (`MAX_SOUNDING_NOTES`) fit in fixed buffers. The division test compares every
scheduled message before dividing, after dividing and after joining, including
technique, dynamic and hairpin. Edit/undo, capacity per voice, copying with a
simultaneous dynamic, coinciding noteheads, chronological accidentals, loading/recovery
and export of the `voces-y-divisi` and `divisi-dos-pentagramas` examples are tested too.

## Next deliveries

1. **Musical editing:** enharmonics and double accidentals, voices 3–4 and editing of
   general irregular groups.
2. **Safety of the work:** more tests with `proptest`, document fixtures and fuzzed
   importers.
3. **Engraving:** refining curves and collisions, and visual snapshots. Add `kurbo`,
   `rstar` and `rayon` when the operations that justify them exist.
4. **Interchange:** MIDI with `midly` and paper options; widen the existing MusicXML
   compatibility.
5. **Sound:** external MIDI (the library is still to be chosen) and tests of device
   disconnection and change.
6. **Measurement and distribution:** representative benchmarks before claiming
   performance budgets, and profiling; a Windows installer with a code signature, and
   manual testing on Windows and Linux. [portability-audit.md](portability-audit.md)
   tracks what is still open on those two systems.

Bravura, GeneralUser GS, Liberation Serif, Liberation Mono and Cantarell are included
with their original licenses. No instrument data has been copied from MuseScore or
LilyPond.


## Arpeggios

`Notation.arpeggio` accepts `Normal`, `Up` and `Down` on chords of at least two pitches.
It is a catalog mark, with stable reference, selection, deletion, replacement and a
history transaction. Removing the last extra note also removes the mark; snapshots bring
it back on undo. Schema 13 keeps arpeggios and opens earlier schemas with simultaneous
chords. The clipboard keeps them on the first fragment of a split note.

Engraving composes the Bravura segments `wiggleArpeggiatoUp/Down` and their arrows
(EAA9/EAAA/EAAD/EAAE), using the original `repeatOffset` and advances. `Glyph.rotated`
turns the whole by 90 degrees without distorting outlines. Ink metrics, hit testing,
cache, following and rhythmic separation include that geometry. The controls use the
complete Bravura symbols E63C/E634/E635.

The plan and audition share a bounded sequence: sorted MIDI pitches, deduplicated
enharmonic unisons, 30 ms per step, at most 120 ms and half a sounding gate. The sounds
of a tied chain are attacked only once. Seeking recovers only the sounds already
attacked. The renderer splits blocks at the audition's pending attacks, with fixed
storage; replacing, playing or stopping cancels the earlier pending attacks.


## Arpeggio across piano hands

Schema 14 adds `Score.piano_arpeggios`, connections of two stable IDs (upper and lower)
and a direction. It only joins simultaneous attacks of the same Piano part, intervals
with one note per hand included. It is edited from either end; it replaces local marks
and is pruned if an edit moves or deletes an end. Undo restores the relationship. The
clipboard, which copies a single staff, turns the shared marking into a local one when it
copies a chord; it exports no references to notes missing from the passage.

`Layout.piano_arpeggios` is computed after placing the staves and computed again after
pagination. Its Bravura segments, selectable box and following use final coordinates.
The grid reserves room in both hands. The plan sorts up to 16 pitches together with the
earlier limits; each hand keeps its levels, techniques and ends, and tied continuations
add no attacks. Audition has 16 fixed slots with intention and end per note; the
callback splits blocks at each attack and release without allocating memory.

### Natural and artificial harmonics

`Notation::harmonic` is a point mark independent of `PlayingTechnique`.
`Harmonic::Natural` is written at sounding pitch with `stringsHarmonic` (E614).
`Artificial` keeps a single stopped note: `harmonic_touch` computes the position and
accidental of its perfect fourth, and `sounding_pitches` resolves the key before raising
two octaves. The `noteheadDiamondWhite` notehead (E0DD) is derived: it has its own
selection as `MarkRef::Harmonic`, never a note identity or a MIDI attack. The double flats
of the derived fourth are engraved without widening the model of input accidentals.
Their local state is cancelled on the following notes.

The core validates instrument, presence of notes and range before inserting, editing or
opening documents. The changes of a passage are validated and applied together, keeping
the slurs that are valid in the final result. Deleting the sound removes the harmonic;
copying a long note keeps it on all its tied fragments. The native format was then 19,
with migration from 18, and the clipboard 6, to prevent an earlier app from silently
losing the new marks. Plan, audition and recovery of sustained notes share the sounding
pitch. `NoteEvent::harmonic` keeps the intention from the plan or audition to the
provider, sustained notes included when seeking a position. The built-in engine
generates the timbre through additive synthesis at the already resolved pitch: a
dominant fundamental, few upper partials, light bow noise and distinct envelopes for
arco and pizzicato. It doesn't use General MIDI's guitar harmonics program nor transpose
the note again.

`platform::sound_engine::harmonics` prepares a sine table, frequencies and 512 voices
before the callback. Partials fade out before Nyquist; bowed notes sustain and plucked
ones decay even while they are still written. CC7/10/11, rests, pause, seek and reverb
use the common contract. There are no allocations or file loads in attacks or rendering.
VSCO CE refuses these attacks because it has no specific samples; the built-in engine
covers them. A future provider can interpret `NoteEvent::harmonic` with recordings. The
synthesis is an approximation of the timbre, not an orchestral recording.

Unisons merge by channel, pitch and timbre class (normal/harmonic), so that the two
voices or a divisi keep both sounds. `HarmonicOff` and `SoundEngine::harmonic_off` release
only the harmonic voice; the normal note uses the usual `NoteOff`. Providers that accept
harmonics must implement that independent release; the default is inert for providers
without those samples. The quadratic velocity and CC11 curves keep a dynamic's
calibration when a hairpin reserves volume headroom.


## MusicXML import

`score-io::open` detects `.levare` documents, older native JSON, partwise/timewise XML
and MXL containers by their contents. It returns `OpenedScore` with the `imported` flag
and warnings; `load` keeps its native contract for recovery and existing tools. The
`musicxml/import` reader positions events with divisions, backup and forward; the
builder turns those positions into voices, fills gaps and splits notes with ties when
needed. `Score::new` builds instruments and empty bars through the domain API, without
depending on the serialized representation.

The input and the decompressed XML are limited to 16 MiB; container.xml to 64 KiB, the
MXL to 4096 entries and the tree to a million nodes. No files are extracted and no DTDs
or external entities are downloaded. A document that declares entities in an internal
DOCTYPE subset is refused, since the parser would expand them in full. ZIP only enables
DEFLATE. Errors keep the open document. The interface imports with `Document::recover`
and without a native path, presents the warnings and uses the normal Save As flow, which
writes a new `.levare`. The original XML is never the autosave's target. The tests cover
MXL, timewise, external voices and chords, transposition, pickups, expressions and the
export/import round trip of the 22 examples of that time, comparing attacks, pitches,
durations, harmonics and techniques.


## Exporting score and parts

`score-export` coordinates the choice of documents (full score and individual `PartId`s)
and the PDF/MusicXML formats. `Score::extract_part` creates a copy with stable
identifiers and keeps the whole part: staves, voices, divisi, marks, connections and the
relevant piano arpeggios. Global meter, key and tempo changes remain in the copy. An
optional part name, with a default value for earlier documents, keeps the «Violín II»
identity when the instrument stands alone; `part_name` shares that name between
engraving and MusicXML. Extraction doesn't transform pitches or modify the original
score.

The export dialog uses the existing modal state, blocks editing, stops the audio and
offers selecting all/none, the score and individual parts, and PDF/MusicXML at once.
Enter continues and Esc cancels. The target is a new folder: all files are prepared in a
temporary folder beside the target, and only afterwards is the final folder created.
`create_dir` prevents replacing earlier sets even if they appear during rendering. If
publishing fails, the files published by that operation are removed. Cancelling the
target or a failed export returns the same selection with an explanation when there is
one. Names are sorted and built by `score-io::file_name`, with the same rules on every
system: separators, control characters and the characters Windows forbids (`<>:"|?*`)
become `_`, leading and trailing spaces and dots are trimmed, names Windows reserves for
devices (`CON`, `NUL`, `COM1`…) get a `_`, and the name is cut to a number of UTF-16
units (as Windows counts its 260-character path limit) and never beyond 255 UTF-8 bytes.
The
operation doesn't touch the document's path, history or save state.

The tests compare the extraction of every part of the examples with the original, voices,
global changes, divisi, connections and piano arpeggios included. They also verify names
and order of sections, subsets, native format, MusicXML of each part, invalid selection,
protection of existing targets and the joint PDF/MusicXML delivery of the Menuetto.


## Click to place playback and pitch drag

`playhead::score_tick` translates canvas coordinates to musical time on the same rhythmic
grid that draws the playhead: a notehead/rest uses its exact onset, a triplet included;
empty space uses beats of the local denominator. It accepts the three views, zoom,
voices, divisi and empty bars. Normal clicks call the existing transport `seek`, keeping
playback or pause; they don't force a view scroll while a possible mouse gesture starts.
Shift keeps the transport's position and keeps extending the passage.

`NoteDrag` tells the initial vertical movement from the horizontal one with the
selection's five-pixel threshold. The vertical one edits a single notehead of the chord;
the horizontal one starts the time movement of the whole note and Shift keeps
`PassageDrag`. A persistent copy of `Score` lets the pitch be previewed without touching
document, history or recovery. Engraving uses a `LayoutEngine` of the gesture's own and
the tentative pitch for the highlight. `layout_pitch_preview` keeps the systems, widths,
rhythmic grid and origins of every staff of the initial layout: the notehead follows the
mouse even if its ink envelope grows. Only on confirming is room reserved and the score
paginated again. The movement uses half a staff space per diatonic step and compensates
the canvas's shifted origin. Explicit accidentals are kept; the key signature decides the
notes without an explicit accidental. Notehead collisions and harmonic limits are
refused with an explanation.

On release, `ReplacePitch` applies a single edit, with audition if it is on. Undo also
restores the connections removed for pitch incompatibility. Returning to the original
pitch creates no history. Escape, loss of focus, releasing outside the window or another
command cancel the preview. Starting the pitch drag stops the audio; selecting/clicking
without dragging keeps its state.

The tests verify gesture resolution, zoom, scroll, chords, limits, harmonics and a single
undo/redo with the right save state. Clicks are compared with the engraved onsets of every
event of the piano, the Menuetto and inner divisi in the three views and four scales;
empty bars with meter changes and coordinates outside the score are tested too.


## Horizontal drag between beats and bars

`Score::move_event` modifies a copy and only publishes after validating. It keeps the
onsets of the neighboring notes and the source voice/staff. It fills gaps with
representable rests (minimal decomposition by dynamic programming), trims target rests
without markings and swaps whole notes if the occupied stretch has the same duration. It
refuses overlaps, targets beyond capacity, rests with marks and unrepresentable gaps
without touching the original. The IDs of elements that stay in the bar are kept; when
crossing a bar only the moved IDs are reassigned and their links remapped before pruning
the incompatible ones. Arpeggios across hands that lose simultaneity become local marks
when there are chords. Pitches are rewritten across key signatures to keep their sound.
Bars, tempo, key, divisi, other voices and instruments keep their positions.

`MoveSelectedEvent` updates selection/cursor and creates a single history entry.
`NoteDrag` fixes the axis after five pixels: vertical changes a notehead, horizontal moves
the whole note. It computes the target on the initial layout, compensating zoom, scroll
and the distance from the grabbed point to the rhythmic grid. It snaps to the grid of
sixteenths or triplets according to the duration, and to the onsets of the original
voice. It checks the move on a copy and draws a blue note with Bravura and a marker at
the target; it doesn't use the pitch-preview layout for changed durations/onsets. Escape,
loss of focus or releasing outside cancel. Shift doesn't arm `NoteDrag` and keeps the
selection.

The tests cover free beats, swapping chords and marks, split rests, a move overlapping
its own earlier position, atomic refusal, key signatures, independent voices and hands,
links, a single undo/redo action and mouse targets in the three views, with zoom, scroll
and a projected divisi voice.


## Orchestra and transposition

`Instrument` gathers 54 profiles and an `ORCHESTRA` template of 34 parts. The selector
sorts by `score_order` and accepts quantities for each instrument.
`Part::transposition` is the interval from written to sounding pitch (diatonic steps and
semitones). Events store written pitches; the global key stays at concert pitch.
`written_key_at` derives the key signature for each part; `sounding_pitches` resolves the
written key signature, the harmonics and then the instrument's interval. Audio, audition
and the search for sustained notes consume that result.

Schema 19 uses a zero interval by default when reading old documents; it doesn't change
their notes or sound. New parts get the catalog's interval. Clipboard 6 keeps the source
interval and converts by concert pitch when pasting into another instrument. MusicXML
exports written pitches and key signatures together with `transpose`, octaves included,
and groups channels through `midi-device`/`port`. Import resolves first to concert pitch
and converts to the target's written profile, keeping the initial explicit interval.
Enharmonic key signatures are limited to seven accidentals; notes that would need double
accidentals are rewritten with an equivalent simple enharmonic. Unpitched percussion
uses its fixed General MIDI note, Bravura clef E069, a central position and an
independent bank 128 on each route.

The orchestra tests cover 48 routes, the full catalog, key changes, compatibility with
files of schema 18, external and exported MusicXML, copying between profiles, the three
views and no clipping when printing tall systems.

For orchestras of 12 parts or more, page view also adapts its width to the tallest
system, avoiding a narrow column when the whole score is reduced. The full-score PDF uses
A3; parts and small ensembles keep A4.

The selector reserves a flexible width for the name and keeps the segments of musical
text on a single line, with stable row heights and quantity controls outside the text
area.
