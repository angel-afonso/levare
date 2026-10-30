# Levare

A native music notation editor written in Rust with GPUI. Scores for several
instruments with aligned staves, keyboard entry, undo history, local files and
ensemble playback with built-in sounds.

Levare runs on macOS, Windows and Linux. Each release is built, tested and published
for all three from the same commit. The score model, editing and layout do not depend
on GPUI.

## Download

Releases are on [GitHub Releases](https://github.com/angel-afonso/levare/releases)
([docs/releasing.md](docs/releasing.md#what-a-release-publishes) lists every file):

- **macOS** (11 or newer, Apple Silicon and Intel): a universal `Levare.app`, as a
  `.dmg` to drag into Applications and as a `.zip`. Unless the release was notarized,
  macOS refuses it the first time: allow it in System Settings → Privacy & Security →
  "Open Anyway" (on macOS 14 and earlier, right-click → Open).
- **Windows** (x86_64): a `.zip` with a `Levare` folder holding `levare.exe`,
  `sfizz.dll` and `licenses`. Unzip it anywhere you can write and run `levare.exe`; it
  needs no Visual C++ redistributable. The download is not code-signed, so SmartScreen
  may warn the first time it runs.
- **Linux** (x86_64, glibc 2.35 or newer: Ubuntu 22.04, Debian 12, Fedora 36 or later):
  a `.tar.gz` with a `levare` folder. Run `./levare` from it; `./install.sh` adds Levare
  to the applications menu and registers the `.levare` and `.partitura` file types
  (`./install.sh --uninstall` removes both). It needs ALSA, `libdbus-1-3`, a Vulkan
  driver and xdg-desktop-portal; the `README.txt` in the folder has the details.

Levare updates itself in place (see [Updates](#updates)), so keep it in a folder you
can write to.

## Running from source

With the stable Rust toolchain that `rust-toolchain.toml` pins:

```sh
cargo run
```

- **macOS:** Xcode's developer tools.
- **Windows:** the MSVC toolchain (Visual Studio Build Tools with the C++ workload).
- **Linux:** the development packages GPUI, ALSA and D-Bus link against. On Debian and
  Ubuntu, as CI installs them:

  ```sh
  sudo apt-get install pkg-config libfontconfig-dev libfreetype-dev \
    libwayland-dev libxkbcommon-dev libxkbcommon-x11-dev libx11-xcb-dev \
    libxcb1-dev libvulkan-dev libasound2-dev libdbus-1-dev
  ```

GPUI is pinned to `0.2.2`. By default its `runtime_shaders` option is on, which on
macOS allows development without downloading Xcode's Metal Toolchain component. To
compile the shaders during the build instead (this needs that component), turn the
default features off and keep the VSCO package:

```sh
cargo build --release --no-default-features --features vsco
```

Levare's icon is included with `cargo run` too. To build or refresh
`target/Levare.app` with that icon in Finder and the Dock (macOS):

```sh
python3 scripts/bundle_macos.py
```

Add `--release` to bundle the optimized build. The app and its title bar use
`assets/branding/levare/icon-macos-v2.png`, with a transparent margin and corners
fitted to the Dock. The script prepares the macOS and Retina sizes before compiling;
`icon-v1.png` keeps the original design.

Levare saves scores as compressed `.levare` files, opens existing `.partitura`
documents and keeps the musical clipboard compatible. Earlier installations keep using
their shortcuts, recent files and recovery data from the old `Partitura` data folder;
new ones keep that data in `Levare` (see [Levare's data folder](#levares-data-folder)).

## Opening scores from the file manager

Double-clicking a score, or using «Abrir con», opens it in Levare. It goes through the
same steps as **Archivo › Abrir…**: MusicXML is imported, and if several files are
opened at once, the first one opens.

What happens when Levare is already running depends on the platform. On macOS, Finder
hands the file to the open window, which asks about unsaved changes first. On Windows
and Linux, the file manager starts a second Levare window with the file. That window has
no autosave or crash recovery, because the first one holds them; close it and open the
file from the first window to keep them.

- **macOS:** Finder knows `.levare` and `.partitura` as Levare's own documents, and
  offers Levare for MusicXML (`.musicxml`, `.mxl`) as another app that can open it.
- **Linux:** after `./install.sh`, the file manager opens `.levare` and `.partitura`
  files with Levare and lists it for MusicXML.
- **Windows:** «Abrir con» and choosing `levare.exe` works. The download has no
  installer, so the file types are not associated with Levare automatically.

## The editor's interface

The interface follows the **Editor · Entrada de notas** visual reference: a dark theme,
a ribbon of tools, parts grouped by family on the left, a light page, a panel for the
selection, sound and review on the right, and the transport and status bar at the
bottom. The score shows its real data, with no sample documents or simulated MIDI
connections. You can open
[`examples/cuarteto-en-sol.partitura`](examples/cuarteto-en-sol.partitura) to see the
five-instrument ensemble.

- **Archivo:** new, open, open recent, save, save as, export to PDF or MusicXML, score
  information and shortcut preferences. On macOS it is in the menu bar and in the
  window; on Windows and Linux the window's **Archivo** menu also holds what macOS keeps
  in the **Levare** menu: the update commands, **Acerca de Levare…** and quit. The title
  bar icons save, undo and redo.
- **Menus and command search:** on macOS the menu bar also has **Edición**,
  **Expresión**, **Repeticiones**, **Instrumentos**, **Tonalidad**, **Reproducción** and
  **Vista**. Windows and Linux have no menu bar: the title bar's **Buscar comandos…**
  field (`⌘F` on macOS, `Ctrl+F` on Windows and Linux) finds every command by name,
  including those that are only in the macOS menus, such as selecting all or full
  screen. Where this README names a menu other than **Archivo**, use the search there.
- **Ribbon:** eight tabs, one per task (`⌥⌘1`–`8`, or `Ctrl+Mayús+1`–`8` on Windows and
  Linux, open them).
  **Escribir** gathers what you choose for each note: note input and octave, duration,
  dot, triplet and rest, accidental, tie and slur, voice, and chords by interval.
  **Articulaciones**: articulations and fermata, grace notes, arpeggios, tremolos and
  string technique. **Dinámicas**: *ppp* to *fff*, hairpins, lines (8va, pedal, trill,
  glissando) and divisi. **Texto**: title and credits, tempo, staff text, lyrics and
  chord symbols, rehearsal marks and their style. **Partitura**: instruments, bars,
  changes of clef, key and time signature, repeats, da capo and segno, and system and
  page breaks. **Reproducción**: play, loop, half speed, repeats, audition, mixer and
  sounds. **Revisión**: check, warnings, instrument range and comments. **Vista**: page
  mode, zoom and collapsing the ribbon.
  Every control is drawn the same way: no background at rest, blue when active or
  chosen, and mutually exclusive options (duration, accidental, voice) share a track.
  Each symbol has its name and key below it, and each group has its title underneath.
  In narrow windows the controls lose their names (the main one of each group keeps
  it) and, if space is still short, the least used groups fold into a dropdown button.
  To the right of the tabs, a pill says what will be written («Escribiendo · Negra con
  puntillo · Voz 2»), with a blue dot when note input is on; without it, it says
  «Seleccionando» and the key that starts writing. If it does not fit, it drops the
  modifiers first and is then truncated.
  The clipboard is in the **Edición** menu and its shortcuts; undo and redo are in the
  title bar. There are no «Quitar» buttons: select the note in the score and press the
  active mark again to remove it, or press Backspace to delete the selection.
- **Parts (left panel):** answers "what is in the score?". The header says **Partes**
  with their count and a single **+** button (Añadir instrumentos, key `I`). Below it
  are a **Lista | Mezcla** switch and the **Buscar parte** field, which filters by name
  ignoring case and accents. Parts are grouped by family; each family is a collapsible
  header with its number of parts, and a search shows them expanded.
  In **Lista**, each part takes one row: its name on the left and, in grey, its clef
  and transposition («Si♭ · Sol», «Fa · 8vb»). Clicking the row selects it; only the
  chosen one shows, if it has two staves, its chips (M. derecha / M. izquierda or
  Divisi I / II) and the delete button. In **Mezcla** (key `M`; pressing it again goes
  back to Lista), each row adds volume and stereo position per part (with several
  parts, each starts at its usual place on stage) and, for strings, woodwinds and
  brass, **Sección | Solista**. The sliders respond to click and drag, even during
  playback, and clicking the value shown on their right resets it to the usual one;
  their settings belong to the session, are kept when switching sound bank and are
  not saved in the score. Solista or Sección is saved and can be undone
  ([details](docs/sfz.md#solista-o-sección)). The footer says which part is chosen and
  which keys change part (`⌥↑` `⌥↓`) or, in Mezcla, the master volume. No editors open
  here any more.
- **Selection, Sound and Review (right panel):** answers "what have I selected and
  what do I change about it?". Three tabs on one line.
  **Selección** changes with what is selected. With nothing selected it shows the
  **Partitura**: editable title, subtitle and composer, the Tonalidad, Métrica and
  Tempo inicial rows (they open their editor) and the Compases, Partes and Duración
  data. With a **note or chord**: the part's name, «Compás N, tiempo T · voz V», the
  written pitch and the duration, «Suena …» if the part transposes, the range warning
  if there is one, **Marcas en esta nota** (one row per mark, with a ✕ that removes it
  and can be undone) and **Texto en esta nota** (Letra, Cifrado, Comentario); with note
  input on, it adds the accidental for the next notes. A **rest** is the same, without
  a pitch. With a **passage** it shows the parts and bars it spans, how many notes,
  parts and marks it contains, **Hacer con el pasaje** (up or down a step or an
  octave, copy, turn into rests, check the selection) and **Marcas en el pasaje**, each
  with its bar and its ✕. The clef, key, time signature, tempo, text, lyrics, chord
  symbol and score information editors open in this tab, with **Aplicar** and
  **Cancelar** (`Esc`), and opening them from the ribbon, a menu or the top strip shows
  the panel and the tab; if an editor is still open while the panel is hidden or on
  another tab, `Esc` shows it first.
  Changing the selection with a click or the navigation keys closes the text, lyrics,
  chord symbol, tempo and information editors; the clef, key and time signature editors
  stay open because they follow the cursor's bar.
  **Sonido** gathers the output (master volume and a soft reverb with amount, room size
  and on/off), the chosen part's sound (bank, sound, natural or literal
  interpretation) and **Más sonidos**, which installs additional packs from the catalog
  with their license ([details](docs/sfz.md#más-sonidos)). **Revisión** has check,
  warnings, instrument range and comments. Long hints appear on hover. The title bar
  buttons show and hide each panel, and the app remembers which were open, the right
  panel's tab, Lista or Mezcla and the collapsed families (`paneles.json` in the data
  folder).
  **Mezcla** (opens Mezcla on the left) and **Sonido** (opens the Sonido tab) are on the
  **Reproducción** ribbon tab; pressing them again goes back to Lista or Selección.
  Mezcla is also in the **Reproducción** menu.
- **Position strip:** above the score, **Compás**, **Tiempo**, **Tempo**, **Métrica** and
  **Tonalidad** (the last three open their editor in the right panel), the active part
  and voice, and a summary of the selection. It is the only place that tells you where
  you are.
- **Status bar:** the status message, the ‹ › arrows with «Página X de Y» and its bars
  (Page view only), the zoom, the autosave and whether there are unsaved changes. Update
  progress and «Reiniciar para actualizar a X.Y.Z» appear here too.
- **Transport:** play/pause, stop and start from the beginning without clearing the
  selection. The clock and ruler follow the real musical position.
- **View:** a ribbon tab with page, continuous scrolling or a single system, and zoom
  from 10 to 200 %, with selection and following fitted to the scale. The initial zoom
  is 70 %.
- **Agent:** a conversation with an AI agent that edits the open score; the title bar
  icon shows it (see [Agent panel](#agent-panel)).
- **About:** **Acerca de Levare…** (in the **Levare** menu on macOS, the **Archivo** menu
  on Windows and Linux, and the command search) shows the version and Levare's MIT
  license, credits Bravura, GeneralUser GS, Liberation, Cantarell and sfizz with their
  licenses, and says whether automatic update checks are on. **Ver licencias** opens the
  licenses folder that ships with Levare.

Controls marked **Pendiente** (or with a dashed border on the ribbon) do not work yet:
several document tabs, search, direct printing, MIDI/real-time input, voices 3–4,
manual stems, per-note velocity/sounding duration, mute/solo and timbre changes.
Saving updates the chosen file; automatic recovery also keeps a copy of the session to
reopen it after an unexpected quit.

## Agent panel

The speech bubble icon in the title bar, or **Vista › Panel de agente**, opens on the
right a conversation with an AI agent that reads and edits the open score with the
editor's own actions. Ask it for what you would do by hand, as you would ask a
copyist: «pon una ligadura en los tiempos 1 y 3 del compás 4 y un trino en el
siguiente».

- **Agents:** Claude Code, Codex or Gemini CLI, with your own subscription; Levare uses
  no API keys. Claude Code and Codex also need Node.js (`npx`); Codex uses the version
  its adapter brings, not the one you have installed. The agent is chosen under the
  text field; in the list, one that is not installed appears dimmed and says what is
  missing.
  The first time, sign in from a terminal (`claude`, `codex login` or `gemini`);
  otherwise the panel reminds you.
- **Conversation:** type in the bottom field and press **Enviar** or Enter; Esc goes
  back to the score. The replies and the tools the agent uses appear in order. With
  Claude Code and Codex, Levare's tools do not ask for permission; if the agent asks
  permission for anything else, the panel opens: choose one of its options or
  **Rechazar**.
  When the agent changes or the session restarts, the panel marks the new conversation.
- **While it works:** the score is locked for editing, undo and redo, but you can play,
  scroll and zoom. **Detener** stops it at any moment and keeps what it already did; if
  the agent does not stop within a few seconds, or you press **Detener** again, Levare
  closes it.
- **Undo:** everything the agent does in reply to one message is undone with a single
  ⌘Z (Ctrl+Z on Windows and Linux). When it finishes, the cursor and selection go back
  to where you left them.
- There is one conversation per document: opening another or closing the window ends
  it, and changing agent starts a new one. Levare's tools never save, open or close
  files; if the agent asks to use the terminal or files, you can refuse.

### Known issues

- **Claude Code** needs a valid signed-in session: if it has expired, the panel asks
  you to sign in; run `claude` in a terminal and try again.
- **Gemini CLI** with a personal account now replies that this client is no longer
  supported (Google is moving it to Antigravity); Levare shows that message as is.
  Gemini also does not say which server each tool belongs to, so every call to Levare's
  tools shows a permission card.
- If you play during a turn, you hear the score as it was when playback started,
  without the changes the agent makes afterwards.

## Usage

The shortcuts follow Sibelius's **Sibelius · portátil** (laptop) profile and work the
same with or without a numeric keypad: the number row acts as the keypad. The full
list for each platform, generated from the code, is in
[docs/atajos.md](docs/atajos.md). Change them in **Preferencias de atajos…** (`⌘,` on
macOS, `Ctrl+,` on Windows and Linux; in the **Levare** menu on macOS and the
**Archivo** menu elsewhere) or with the **Atajos** button in the title bar.

This README writes keys as on macOS. On Windows and Linux, ⌘ is Ctrl, ⌥ is Alt and ⇧
is Mayús, with a few exceptions where the platforms differ: ⌥⌘ shortcuts become
Ctrl+Mayús (`⌥⌘T` is `Ctrl+Mayús+T`), **Ir al compás…** is `Ctrl+G`, full screen is
`F11`, the playback line goes to the start with `Ctrl+Inicio`, and replaying the last
playback is `Mayús+Alt+Espacio` on Linux. [docs/atajos.md](docs/atajos.md) has each
key per platform. A shortcut you record with Ctrl+Alt and a letter, digit or symbol is
refused on Windows and Linux, where Ctrl+Alt is AltGr or a desktop shortcut, and
Preferences suggests Ctrl+Mayús instead; such shortcuts saved on macOS are skipped
there and listed in Preferences.

The ribbon and the right panel show each control's name and current shortcut on
hover. The menus and the **Atajos** window keep the keys visible for reference,
without repeating them permanently next to the buttons.

| macOS | Windows and Linux | Result |
|---|---|---|
| `N` | `N` | Turn note input on / off |
| `⌥1` / `⌥2` | `Alt+1` / `Alt+2` | Select voice 1 / 2 of the active staff |
| `A`–`G` | `A`–`G` | Write A–G with the current octave and duration |
| `⇧A`–`⇧G` | `Mayús+A`–`G` | Add that note above the selected chord |
| `⇧2`–`⇧9` | `Mayús+2`–`9` | Add a second … ninth above (following the key signature) |
| `1` `2` `3` `4` `5` `6` | the same | Thirty-second, sixteenth, eighth, quarter, half, whole |
| `7` / `8` / `9` | the same | Natural / sharp / flat; repeating it removes it |
| `0` | `0` | Rest with the current duration |
| `↩` | `Intro` | Tie to the next note or chord |
| `S` | `S` | Start and then end of a slur |
| `R` | `R` | Repeat the selected note, chord or rest |
| `↑` / `↓` | `↑` / `↓` | Up / down a step |
| `⌘↑` / `⌘↓` | `Ctrl+↑` / `Ctrl+↓` | Up / down an octave |
| `⌥↑` / `⌥↓` | `Alt+↑` / `Alt+↓` | Upper / lower note of the chord and, at the end, the neighboring staff |
| `←` / `→`, `⇥` / `⇧⇥` | `←` / `→`, `Tab` / `Mayús+Tab` | Previous / next element |
| `⌫` / `⌦` | `Retroceso` / `Supr` | Delete the selected element |
| `Espacio` / `P` | `Espacio` / `P` | Play, pause or resume / play from the selection |
| `Esc` | `Esc` | Stop; if already stopped, clear the selection |
| `⌘B` | `Ctrl+B` | Add a bar at the end |
| `I` / `K` / `M` | `I` / `K` / `M` | Add instruments / key signature / mix |
| `T` | `T` | Change the time signature from the start or the selected bar |
| `Q` | `Q` | Change the clef of the active instrument or staff |
| `⌥A` / `⌥⌫` | `Alt+A` / `Alt+Retroceso` | Chord mode / remove the highest note (Levare's own) |
| `⌘Z` / `⌘Y` (or `⇧⌘Z`) | `Ctrl+Z` / `Ctrl+Y` (or `Ctrl+Mayús+Z`) | Undo / redo |
| `⌘+` / `⌘-` / `⌘1` | `Ctrl++` / `Ctrl+-` / `Ctrl+1` | Zoom in / zoom out / 100 % |
| Click on a note | the same | Select it; input continues after it |
| Click on an empty bar | the same | Move the input point there |

Sibelius keys with no function yet (`L` lines, `Z` symbols, `⌥3`/`⌥4` extra voices,
`⇧⌘F` real-time input, `⌘P` print) are reserved and show «Pendiente». Articulations
have no default shortcut, as in Sibelius's laptop profile; assign them in Preferences
if you use them often.

A full bar moves on to the next one when you enter another note value. If there is no
next bar, it is created. A value longer than the remaining duration is refused without
changing the document. Changing the duration affects the next entry. You can open
[`examples/primera-melodia.partitura`](examples/primera-melodia.partitura) as an
example.

## Notes outside the instrument's range

Notes outside the instrument's usual range appear **in red** in the score. In a chord
each note is marked separately. When you select it, the **Selección** tab of the right
panel shows the warning and the range limits in sounding pitch (C4 = middle C). The
key signature, accidentals, the part's transposition and string harmonics are taken
into account. The color also updates when you drag a note, edit it, undo or redo.

The warning is a guide: you can keep writing and playing the note. Unpitched
percussion has no warning. The warning colors belong to the editor and do not appear
in PDF or MusicXML. The ranges are professional reference values for common models;
they cannot be tailored to a difficulty level yet. You can adjust the limits per part
from Revisión. The conventions and sources are in
[Instrument ranges](docs/instrument-ranges.md).

## Reviewing the score

**Revisión › Comprobar › Toda la partitura** checks the whole work; **Selección**
checks the events of the selected passage and the form marks of its bars. It looks for
notes out of range, repeat starts without an end and jumps that lack a Segno, Fine or
To Coda; it also checks that a To Coda has a later target and a jump that triggers it.
Empty bars and implicit rests are valid.

**Revisión › Avisos › Mostrar** opens the Revisión panel. Each row explains the warning
and its instrument and bar; clicking it selects the exact note of the chord or shows
the bar of the mark. **Anterior / Siguiente** step through the filter's results,
wrapping to the start or end. You can filter by instrument; warnings about repeats and
jumps are global. Long lists have pages. The full check updates when you edit or undo.
A selection check asks to be run again after editing, so that it does not attribute
warnings to a passage that has changed.

**Revisión › Registro del instrumento › Mostrar** shows or hides the warning color
without turning the check off. **Editar…** adjusts the active part's limits in sounding
pitch; it accepts names such as `C4`, `Do4`, `F♯3` or `Si♭5`. The limits are inclusive,
also apply to the sound of its harmonics and are saved with the document.
**Restablecer** brings back the usual range. Applying and resetting support Undo/Redo.
Unpitched percussion needs no limits. The color and the review results are session
settings and are not exported.

## Review comments

Select a note, chord, rest or passage and press **Revisión › Comentarios › Nuevo**.
Write the remark and press **Guardar** or Enter; Esc cancels the draft. The link is
fixed when the draft opens, even if you change the selection afterwards. The field
holds a remark of up to 2000 characters. There is a limit on the total space for
comments and their links; if it is reached, the draft keeps the text so you can shorten
the passage or free space in the list.

**Revisión › Comentarios › Mostrar** opens the list with the filters **Todos**,
**Pendientes** and **Resueltos**. Clicking a remark selects its music and shows the
start of the passage, also in another voice or piano hand. **Resolver** marks it as
done; **Reabrir** makes it pending again. You can also edit the text or delete the
comment. Long lists have pages.

Comments are saved in the native file and in automatic recovery; creating, editing,
resolving, reopening and deleting support Undo/Redo. They follow their events when
notes move, the divisi changes or bars are inserted and deleted. If all their music is
deleted, the remark is kept **sin vínculo musical** (without a musical link): you can
edit it and use **Usar selección actual** to attach it to other music. If the score
changes while you write a draft, update its link before saving it; the text is kept.
Remarks belong to the document: they are not duplicated when copying music and are
not included in PDF or MusicXML. Changing their text or state keeps the check warnings
and does not change playback.

## Clef changes

In **Partitura › Cambios › Clave…** (`Q`) you can choose treble, bass, alto or tenor
clef for the active instrument. Choose the start of the piece or the current bar: the
clef holds until the next change. On piano and other two-staff instruments, each staff
keeps its own clef; the branches of a divisi section share the instrument's clef. Notes
keep their pitches and their sound. You can restore the previous clef, undo and redo;
the changes are saved and kept in PDF and MusicXML. Changes are placed at the start of
a bar.

## Copying, cutting and pasting music

- **Shift + drag over the notes, or drag from the empty staff**, to select a whole
  passage, forwards or backwards, even across bars and lines. The blue band marks the
  passage and the top bar shows how many elements it contains, with **Copiar** and
  **Cortar** buttons. **Shift + drag** selects a passage in any direction. A click
  selects a note and places playback at its entry.
- **Drag a notehead up or down** to change its pitch. The engraving shows a preview
  while you move the mouse; releasing applies a single change you can undo with
  **⌘Z**. **Esc** cancels. In a chord only the chosen head moves. Duration, voice and
  rhythmic position are kept. Rests keep the passage selection gesture. If you are
  playing, starting to change a pitch stops the audio; on release you hear the edited
  note if audition is on.
- **Drag a note sideways** to move the whole value (the whole chord if it has several
  heads) to another beat or bar, within its voice and staff. A blue copy shows the
  destination; releasing applies a single edit. **Esc** cancels and **⌘Z** undoes. The
  duration is kept and rests are left at the origin, without shifting the other notes.
  The destination snaps to sixteenths (thirty-seconds if the value is a thirty-second),
  their triplets for triplet values, or existing entries. If there is a note/chord of
  the same duration, the values swap. Incompatible overlaps or a value that does not
  fit are refused with an explanation, without changing the work. When crossing a key
  signature, the accidentals needed to keep the sounding pitches are added. Ties and
  slurs incompatible with the new beats are removed and come back on undo; an arpeggio
  across separate hands stays local where there is a chord.
- **Shift + click** or **Shift + ←/→** extends the selection within the same staff;
  **⌘A** selects all its written elements.
- **⌘C** copies, **⌘X** cuts and **⌘V** pastes. They are also in the **Edición** menu,
  with configurable shortcuts.
- Pasting goes **after the selection**, or at the cursor of the empty bar you choose.
  You can switch hand, instrument or document before pasting.
- Chords, rests, duration, accidentals, dynamics, articulations and the ties and slurs
  whose two ends are in the passage are kept. Sounding pitches are kept when pasting
  into another key; the dynamic in force goes along with the start.
- The time spacing between bars is kept, filling implicit gaps with rests. If a value
  crosses a barline, it is split and tied; aligned bars are created for all
  instruments when needed.

Pasting does not replace existing music: if the passage does not fit in the
destination bars, the reason is shown and the document stays intact. Choose an empty
bar or a point with enough room. Each paste or cut is undone in a single step.

The musical clipboard is the system's and works between Levare windows. Plain text or
images from other applications are not read as notes. Passage selection covers one
staff at a time; you can copy each piano hand separately. Ties and slurs with ends
outside the passage are not copied.

## Dots and triplets

In **Escribir › Duración** choose the value and in **Escribir › Modificar** turn on
**Puntillo** or **Tresillo**. Default shortcuts: **`.`** and **⌘3**, configurable in
Preferences. They are also in the **Edición** menu. The modes stay on for the following
notes and rests until you press them again; changing the base value keeps the modes.

With a note or chord selected its duration changes; with a passage selected it applies
to all its values in a single operation. The dot adds half (for example, dotted
quarter = quarter + eighth). The triplet uses a **3:2** ratio: three triplet eighths
last exactly one quarter. You can combine dot and triplet. Playback, ties, hairpins and
both piano hands use these exact durations.

The engraving places the dots outside the lines and groups up to three consecutive,
equal triplet values with a **3**, above the beam when the group is complete or with a
bracket otherwise. Incomplete groups stay visible while you write; a change of value or
a new bar starts another group. This covers triplets of equal values, without nested
groups or other tuplet numbers.

Changes are saved, support undo/redo and are kept when copying and pasting. If an edit
makes the duration longer than what fits in the bar, it is refused entirely. Pasting
keeps the original values that fit; those that cross a barline are split with ties
when the destination allows writing the split. If it cannot be written with the
available values, choose an empty bar or a destination aligned with the passage.
Example: [`examples/ritmos.partitura`](examples/ritmos.partitura).

## Automatic beams

Eighths, sixteenths and thirty-seconds are beamed automatically by beat, also in
chords and in each piano hand separately. Sixteenths use a second beam and
thirty-seconds a third, with the stem half a space longer; mixed rhythms add partial
beams. Rests, barlines and the bounds of each triplet separate the groups. An isolated
note keeps its own flag.

In 4/4 they group by quarters; in 2/2, by halves; in 6/8, 9/8 and 12/8, by groups of
three eighths. The defaults for 5/8 and 7/8 are **3+2** and **2+2+3**. Complete
triplets of eighths or sixteenths show only the **3** by their beam; incomplete groups
or groups with rests keep the bracket.

They are recalculated when writing, editing, pasting or undoing, and when opening older
scores. They change neither durations nor playback. Example:
[`examples/barras.partitura`](examples/barras.partitura).

## Ties, slurs, dynamics and articulations

The ribbon (**Escribir** for ties and slurs, **Articulaciones** for articulations,
arpeggios and technique, **Dinámicas** for dynamics and hairpins), the **Selección**
tab of the right panel (to remove them) and the **Expresión** menu edit these marks on
notes and chords of any staff. All changes are saved and support undo/redo. Example:
[`examples/piano-expresion.partitura`](examples/piano-expresion.partitura).

- **Tie (`↩`):** select the first of two equal, consecutive notes or chords. The sound
  is held without a new attack, also across bars. Chords are tied whole; the sounds
  must match in every pitch. Pressing `↩` again removes only the tie to the next one.
  Chains can continue over several notes.
- **Slur (`S`):** select a passage and press `S`: the phrase goes from its first to
  its last note; repeating it on the same passage removes it. You can also select the
  start, press `S`, select the end note on the same staff and press `S` again. Esc
  cancels the operation. Pressing `S` with the first note of a phrase selected removes
  it. There are more ways to draw it in
  [Drawing slurs and hairpins](#drawing-slurs-and-hairpins). The phrase is drawn with a
  curve, even across systems. The ends belong to the same voice; phrases in different
  voices may coincide, but two in the same voice may not overlap. In playback, the
  notes of the phrase join with a short overlap (legato); the last one is separated as
  usual, and a staccato inside the phrase sounds portato (75 %).
- **Dynamics:** ppp, pp, p, mp, mf, f, ff and fff. They apply from the marked position
  to the next marking on the same staff. Each piano hand keeps its own dynamic. With no
  marking, mf is used (MIDI velocity 88). A dynamic can be placed on a rest to prepare
  the next entry. Pressing the note's active dynamic again removes it.
- **Articulations:** with no marking, each note sounds for 92 % (with a gap of at most
  60 ms) to separate it from the next; staccato shortens the sound to 50 %; tenuto
  keeps the full duration; accent and marcato strengthen the attack (+16/+24 velocity,
  up to 127).
  They affect every note of the chord, without changing its written duration. In a
  tied chain the initial attack is kept; a final staccato shortens the chain's combined
  sounding duration to 50 %. This keeps the sound when a value is split while pasting.

- **Arpeggios:** select a chord and choose **Articulaciones › Arpegio** (also in the
  **Expresión** menu). There are normal, upward and downward markings. The Bravura line
  fits the chord's height and sits to the left of its accidentals. Normal and upward
  sound from low to high; downward, from high to low. The notes are staggered without
  lengthening the bar; a tied chain keeps a single attack.
  **Ambas manos** joins the notes that start together on both staves of the same piano.
  Select a note or chord in either hand and turn that option on; there may be a single
  note in each hand. The line crosses both staves and all the notes form a single
  arpeggio. Changing the direction from either hand updates the shared mark. Turning
  **Ambas manos** off leaves a local arpeggio on the selected chord; if it is a single
  note, the marking is removed.
  Pressing the same direction option again removes the arpeggio. If the chord is left
  with a single note, the marking is removed; undo brings it back. When pasting a value
  split across bars, the arpeggio stays on the first fragment.
  Example: [`examples/piano-arpegios.partitura`](examples/piano-arpegios.partitura).

- **Fermata:** in **Articulaciones › Articulación** or the **Expresión** menu. It goes
  on notes or rests, alongside any articulation. In playback, that moment lasts twice
  as long in every part and the music continues afterwards; the ruler and the clock
  account for it.

- **Grace notes:** in **Articulaciones › Nota de adorno**, select the note and choose
  **Acciaccatura** (small, slashed, almost no time) or **Appoggiatura** (leans on the
  note and takes half its value). The grace note starts at the note's pitch; `;` adds
  another, up to four, written beamed with two beams. The new one stays selected:
  **↑/↓** change its pitch (**⌘↑/⌘↓**, by octave) and **Backspace** deletes it. A click
  on any of them selects it alone. They take no time in the bar, sound within their
  note's time, reserve their space on the left and are exported and imported in
  MusicXML.

**Editing marks in the score:** a click on an arpeggio, articulation, fermata,
dynamic, technique, slur, hairpin or line selects only that mark (it is highlighted in
blue; the note is not). **Backspace** deletes it without touching the note. Pressing
another mark of the same family on the ribbon changes it (for example, staccato to
accent, *p* to *f*, or crescendo to diminuendo), and pressing the same one removes it.
Esc goes back to the note; with a mark selected, the arrows do not transpose the note.
A click on the notehead still selects the note.

**Moving dynamics, techniques, fermatas and hairpins:** their position in time decides
how they sound, so when you drag them sideways they jump to the nearest note or rest
of their voice and, on release, sound from there. A hairpin moves whole and keeps its
duration. While you drag, a dashed line joins the mark to the note or notes it belongs
to. Vertically only the height changes. With `⌘` (Ctrl on Windows and Linux) held they
move without changing note, like the other marks. With the mark selected, `⌥←` and
`⌥→` take it to the previous or next note. If the destination already has a mark of
the same family, the status bar says so and nothing changes on release.

The active articulations, dynamics, techniques and accidentals are highlighted on the
ribbon; pressing them again removes them. The **Selección** tab can also remove, with
the ✕ of **Marcas en esta nota**, the markings and the ties and slurs that touch the
chosen note; undo brings them back.
If a pitch edit, an insertion or a deletion invalidates a tie, that tie is removed;
undo brings back both the notes and their marks. Starting playback from a selection
recovers the dynamic in force and the tied notes.

## Drawing slurs and hairpins

Slurs and hairpins can be written in four ways:

- **Over a passage:** select it with Shift+click, Shift+arrows or by dragging from a
  gap in the staff (dragging a note moves it), and press `S` (slur), `H` (crescendo)
  or `⇧H` (diminuendo). If the passage starts or ends on rests, the slur fits to its
  notes.
- **While writing:** with note input on, press `S` or `H` on the note just written. The
  mark grows with each new note and on a rest it waits for the next note. Press the
  same key again, or Esc, to finish it. Undoing the last note takes the mark back to
  its previous end.
- **With the mouse:** hold `⌥` (`Alt` on Windows, `Ctrl` on Linux) and drag from one
  note to another of the same voice for a slur; `⌥⌘` (`Ctrl+Alt`) draws a crescendo and
  `⌥⇧⌘` (`Ctrl+Mayús+Alt`) a diminuendo. Linux does not use Alt alone because Xfce,
  Cinnamon and MATE move the window on Alt+drag; the full table is in
  [docs/atajos.md](docs/atajos.md#mouse). While you drag, the score shows the result;
  if the end is not valid, the status bar explains why and nothing changes on release.
- **Stretching an existing one:** click on a slur or hairpin; two round handles appear
  at its ends. Drag a handle to another note to lengthen or shorten it. Dragging the
  rest of a hairpin moves it whole to other notes; on a slur, it shifts it. Any manual
  offset it had is kept.

All support Undo/Redo, and Esc cancels a drag in progress.

## Crescendo and diminuendo with hairpins

Select a passage with **Shift+click** or **Shift+arrows** and press **H** for a
crescendo (`<`) or **Shift+H** for a diminuendo (`>`). You can also select the first
note, press the hairpin, select the last one and press the same button or shortcut
again. **Esc** cancels the pending start. The actions are in **Dinámicas › Regulador**
and the **Expresión** menu. The shortcuts can be changed in Preferences.

The hairpin goes from the start of the first note to the start of the last one. It can
cross bars, lines and rests. Written dynamics set the passing and arrival levels: for
example, **p → f**. If you write none at the end, it rises or falls two levels from the
hairpin's last dynamic, within ppp–fff. The level reached holds until the next marking.
Explicit marks take priority over the symbol's direction.

The volume changes while notes and chords sound, including tied chains, with
pause/resume and start from selection. Each instrument and each piano hand keeps its
own curve. Adding a hairpin keeps the volume and timbre of the passages outside it.

To remove it, select a note inside the hairpin (except the last, from which another
may start) and press **Crescendo** or **Diminuendo** again. It is saved with the score
and supports undo/redo. Copying a passage keeps the hairpins whose two ends are
selected; those that leave the passage are left out. When copying from inside one of
the latter, the initial dynamic is approximated to the nearest written level.
Overlapping hairpins on the same staff are not allowed; they may share an end.
Example: [`examples/reguladores.partitura`](examples/reguladores.partitura).

## Lines: 8va, 8vb, pedal, trill and glissando

On the ribbon, in **Dinámicas › Líneas**, select a passage (or a single note) and
choose the line. It is drawn from the first note to the end of the last one, breaks at
line changes and each kind takes its own lane, outside the notes, slurs and dynamics.

- **8va / 8vb:** the writing does not change, but the passage sounds an octave higher
  or lower. A dashed line with a hook at the end; it may start or end on a rest.
- **Pedal:** «Ped.» and a line until it is released. The notes keep sounding until the
  end of the line; it may cover rests.
- **Trill:** «tr» above the note. On a passage, or on a note of a half or longer, the
  wavy line is added. It sounds alternating with the upper note of the key, also under
  an 8va. Notes only.
- **Glissando:** with a note selected it goes to the next one; with a passage, from the
  first to the last. It sounds in semitones, or on white keys on keyboards and harp.

To remove a line, click on it and press **Backspace**, or select a note of the line and
press its button again. Two lines of the same kind do not overlap on a staff (8va and
8vb count as one kind). They are saved with the score, support undo/redo and are
exported and imported in MusicXML (`octave-shift`, `pedal`, `trill-mark` with
`wavy-line`, and `glissando`).

## Expression and technique text

Select a note or a rest and press **⌘E** (expression text) or **⌘T** (technique text),
or use **Texto › Sobre el pentagrama** on the ribbon. The editor opens in the Selección
tab of the right panel, with the field and the usual markings; **Enter** applies,
**Esc** cancels and an empty field removes the text. About 40 characters fit.

- **Expression** («dolce», «espress.», «cresc.»): italics below the staff, on the
  dynamics line and after the dynamic of the same note («*p* dolce»). Hairpins leave
  room for it.
- **Technique** («con sord.», «sul pont.»): above the staff, in the pizz. and arco lane.

Some words also sound: «cresc.» and «dim.» change the volume like a hairpin until the
next dynamic (or over two bars if there is none); «con sord.» softens and darkens the
sound until «senza sord.»; «legato» joins the notes until «non legato»; «ord.» returns
to the ordinary mode. On strings, writing «pizz.» or «arco» sets the actual technique.
Other words are only written.

A click on the text selects it: **Backspace** deletes it, dragging it takes it to
another note of the staff and pressing ⌘E or ⌘T again edits it. Texts are copied with
their notes, support undo/redo and are exported and imported in MusicXML as `words`
(the expression below and in italics, the technique above).

## Lyrics

Select the first note and press **⌘L** (or **Texto › Letra y cifrado › Letra**). Type
the syllable and continue with:

- **Space**: the word ends; move to the next note.
- **Hyphen (-)**: the word continues; a hyphen is drawn up to the next syllable.
- **Underscore (_)**: the syllable is held over the following notes, with an extender
  line up to the last note before the next syllable.
- **Enter** applies and closes; **Esc** closes.

Entry skips rests and the arrival notes of a tie. The **Estrofa 1–4** buttons change
verse; a click on a syllable selects it (Backspace deletes it) and ⌘L opens it in its
verse. Each verse takes a line along the system, below the notes and slurs; dynamics
move below the lyrics. A syllable holds about 20 characters. Lyrics are not placed on
rests, are copied with their notes and are exported and imported in MusicXML; imported
elisions are written with a space.

## Chord symbols

Select a note or rest and press **⌘K**. Type the chord in letter names (**C**,
**F#m7**, **Bb7/D**, **Gsus4**, **Cmaj7**, **Dø**, **G7(b9)**…) or with solfège names
(**Do**, **Fa#m7**, **Sib7/Re**, **Lam**); it is always written with letters and with
musical flats and sharps. The editor suggests the chords of the bar's key. **Tab**
applies and moves to the next note, **Enter** applies and closes, an empty field
removes the symbol. Chord symbols go above the staff, on their own line above the
techniques; they are dragged to another note and deleted with Backspace like the other
marks.

In playback, each chord symbol sounds as a soft accompaniment until the next symbol on
the staff: on keyboards, guitar and harp with their own sound; on other parts, with a
piano. It is written as the musician reads it, so on a transposing instrument it sounds
transposed like its notes. **Reproducción › Sonido › Cifrados** mutes it; the choice is
saved with the score. When notes are transposed, their chord symbol is transposed with
them.

## Pizzicato and arco

Select a note, chord or rest of **violin, viola, cello or double bass**. On the
ribbon, in **Articulaciones › Técnica de cuerda**, or in the **Expresión** menu, choose
**pizz.** to play by plucking the string and **arco** to return to the usual sound. The
marking appears in italics above the staff and holds until the instrument's next mark,
also when playing from a selection. A mark on a rest prepares the next entry.

Pressing the chosen note's active technique again removes the mark; the previous one
applies again.
Changes are saved, can be undone and are kept when copying passages, even if the
passage starts after the original mark. Passages with bowing techniques are pasted
into compatible instruments. On a tied note, the attack's timbre holds until the chain
ends; the change affects the next attack. You can assign shortcuts in Preferences.
Example:
[`examples/cuerdas-pizzicato.partitura`](examples/cuerdas-pizzicato.partitura).

## String harmonics

Select a note or a passage and use **Expresión › Armónico natural** or **Armónico
artificial**, also available on the ribbon (**Articulaciones › Técnica de cuerda**).
Press the same option again to remove it; you can also select the circle or the
diamond and delete only that mark. Undo history, the clipboard, autosave and PDF export
keep them.

- **Natural:** write the pitch that should sound. Bravura's circle is added.
- **Artificial:** write a single stopped note; the program adds the diamond a perfect
  fourth above and plays only the sound two octaves higher. It accepts stopped notes
  from C0 to B6 to keep the sound within the editor's range.

They are marks on each event, compatible with arco, pizzicato, articulations, ties and
slurs, two voices and divisi. On a passage they apply to all the selected notes in a
single operation; rests do not take harmonics. Playback respects the sounding pitch
and uses Levare's own synthesized timbre for harmonics: sustained and airy with the
bow, with a short attack and a decaying resonance in pizzicato. It also works when
auditioning notes and when resuming from a selection, with both GeneralUser and
VSCO CE. It is an approximation by synthesis; these are not recordings of harmonics
from an orchestral library.
Example: `examples/armonicos-cuerdas.partitura`.

To compare, at the same pitch and dynamic, normal arco, the bowed harmonic, normal
pizzicato and the pizzicato harmonic:

```sh
cargo run --offline --locked -p score-platform --example harmonics_compare
```

It writes `output/audio/comparacion-armonicos.wav` with the app's own engine.

## Scores for several instruments

Press **+** in the parts panel header (or `I`) to open the dialog. Tick several
instruments and adjust their counts with **− / +**; **Añadir** adds the set in a single
operation you can undo. **Cancelar** or **Esc** closes without changing the score. You
can also use the **Instrumentos** menu. The catalog has **54 profiles** of woodwinds,
brass, percussion, keyboards, voices and strings; a score holds up to **48 parts**,
with repeated instruments.

Select a part in the left panel's list or click its staff to write in it.
**⌥↑ / ⌥↓** moves to the neighboring staff (and, in a chord, first goes through its
notes). Each part keeps its notes; bars are added to every instrument and coinciding
beats stay aligned. The **trash** button of the chosen part deletes that instrument
with its staves. **Deshacer** brings back its notes, dynamics, ties and slurs. The
**Instrumentos** menu can delete the active part. The score keeps at least one
instrument. If you delete the active part, the neighboring instrument is selected; when
you delete another one, your selection is kept.

When a score is opened or instruments are added, the score and the parts list are
sorted by family: woodwinds, brass, percussion, keyboards, plucked strings, voices and
strings. Within each family the orchestral catalog order is kept
(flute–oboe–clarinet–bassoon, horn–trumpet–trombone, violin–viola–cello–double bass).
Repeated instruments keep their relative order and their notes; the IDs do not change.

Each system shows **family brackets** and barlines joined within each group, with
spacing between families. The piano keeps its **two-hand brace**. The thin initial
barline connects the whole system. The order is saved in the native file and older
scores need no conversion.

Each part uses its timbre and its treble, C, bass or percussion clef. New transposing
parts are written in **written pitch** and sound at the corresponding real pitch. The
overall key is chosen in sounding pitch: in a work in C major, the B♭ clarinet shows D
major and its written C natural sounds B♭. Key changes during the piece, auditions,
copying between parts and the exports of score and parts to PDF and MusicXML adapt as
well.

There are clarinets in C, B♭ and A, E♭ clarinet, bass clarinet, cor anglais, horn in
F, trumpets in C, B♭ and D, cornet and saxophones; the octave transpositions of
piccolo, contrabassoon, double bass, guitar, celesta, xylophone and glockenspiel are
included. Harp, piano, organ, celesta, marimba and vibraphone use two staves. Scores of
12 parts or more are exported to PDF on A3; individual parts stay on A4. The voices are
**Voz** (solo), **Soprano**, **Contralto**, **Tenor** and **Bajo**; the tenor is written
in treble clef an octave above how it sounds. No sound pack includes voices, so they
sound with the choir of the built-in General MIDI bank. Snare drum, bass drum, cymbals,
triangle, tambourine and tam-tam have a percussion clef without key signature and one
stroke per voice. Their sounds are approximated with the built-in General MIDI bank;
profiles with the same program share a timbre.

Each staff takes two voices; single-staff instruments can use divisi. Older documents
keep their original pitches and sound, including the double bass previously written in
sounding pitch. You can open
[`examples/orquesta-completa.partitura`](examples/orquesta-completa.partitura) to check
the line-up and each instrument's key signature changes.

[`examples/menuetto-y-trio-orquesta.partitura`](examples/menuetto-y-trio-orquesta.partitura)
arranges the string Menuetto for **32 parts and 84 bars**: piccolo, woodwinds in pairs,
cor anglais, bass clarinet, contrabassoon, four horns in F, two trumpets in B♭, three
trombones, tuba, timpani, bass drum, cymbals, triangle, harp and the five string
sections. It keeps divisi, harmonics, pizzicato, hairpins and changes of tempo, time
signature and key. The double bass moves to octave notation without changing its
sound. Bars **17 and 65** let you compare the tutti; bar **33** starts the Trio and
bar **49** changes to 6/8. In concert D major, the B♭ clarinets and trumpets show E
major and the horns in F show A major. It can be regenerated with:

```bash
cargo run -p score-io --example menuetto_orquesta -- examples/menuetto-y-trio-cuerdas.partitura examples/menuetto-y-trio-orquesta.partitura
```

New files use schema 29 and store parts, staves, chords, accidentals, key and
expression. Schemas 1–28 open; schemas 1–5 use C major to keep their original sound:
the existing piano notes stay in the right hand and an empty left hand is added. You
can open [`examples/trio.partitura`](examples/trio.partitura) to hear an ensemble of
piano, flute and cello.

## Two voices and divisi

Choose **Voz 1** or **Voz 2** in **Escribir › Voz** or with **⌥1 / ⌥2**. Each voice has
its own notes, rests and durations within the same bar; it does not use up the other's
rhythmic space. Choosing an empty voice turns on writing from the start of the bar. If
it already contains music, its first note is selected: use the arrows to go through it
and **N** to continue writing after the selection. Use rests to delay an entry.

When both voices are present, voice 1 has stems up and voice 2 stems down. Beams,
triplets, ties, slurs and articulations belong to their voice. Selecting, copying,
pasting and deleting a passage act on the active voice; clicking a note makes its
voice active. In a unison with coinciding heads, the active voice is chosen. Each piano
hand also takes its two voices.

For an instrumental section, **div.** opens two staves from the selected note or rest:
voice 1 in **Divisi I** and voice 2 in **Divisi II**, keeping the clef. **unis.** marks
the return to playing together at the chosen position. Notes keep their voice,
identity, rhythm and connections; changing the layout does not change playback. The
buttons are in **Dinámicas › Divisi** (**Dividir** and **Unir**) and the Expresión
menu; they also work on an empty bar.

To divide only a range, select its notes with **Shift + click** or by dragging and
press **div.** or **Dividir**. The span starts at the first note and ends when the last
one finishes; there the previous layout is restored. You can open and close several
spans within the same bar. Without a selection, the input position is used. To make
reading easier, the lower staff is shown whole in every bar that contains divisi, with
its clef and key signature. The «div.» and «unis.» marks keep their exact beats within
the bar; outside the affected bars a single staff returns. Sustained notes keep their
value and their line, with no artificial ties between staves. The document and playback
do not change. Splits that would need values smaller than the available ones are
refused without changing the music.
Pressing an active marking again removes that boundary; Undo brings back the whole
change. Select the lines with **Divisi I / II**, **⌥↑ / ⌥↓**, a click or **⌥1 / ⌥2**.

Dynamics, hairpins and techniques are shared between the section's two voices; rhythm
and articulations stay independent. Saving, recovery and PDF keep the passages. Older
files with full divisi open as before; when their first passage is edited, the lines
are merged internally without losing music. If they contain more than two voices, empty
the extra ones first. The piano keeps its two hands and does not use this conversion.

Examples: [`divisi-dentro-del-compas.partitura`](examples/divisi-dentro-del-compas.partitura),
[`divisi-por-pasaje.partitura`](examples/divisi-por-pasaje.partitura),
[`voces-y-divisi.partitura`](examples/voces-y-divisi.partitura) and
[`divisi-dos-pentagramas.partitura`](examples/divisi-dos-pentagramas.partitura).

## Two-hand piano

Each piano is a single instrument with a **right hand in treble clef** and a **left
hand in bass clef**, joined by a brace and barlines. Both hands are written and edited
independently, with their beats aligned.

Choose **Mano derecha** or **Mano izquierda** under the piano in the parts list. You can
also click the corresponding staff or use **⌥↑ / ⌥↓**. The initial octave is 4 for the
right hand and 3 for the left; **⌘↑ / ⌘↓** moves the written note an octave and input
continues from there. Bars, saving and Undo include both hands. Playback mixes them with
the same piano; pausing and resuming keep the exact point of both.

Following tries to show the whole ensemble and, if it is too tall, the two hands of the
active piano. Overlapping unisons in both hands are held as a single key until they
end; cross-staff writing is not supported yet. You can open
[`examples/piano-dos-manos.partitura`](examples/piano-dos-manos.partitura).

## Chords in each hand

Write the first note with `A`–`G` and add the others with **⇧A–⇧G** or with the
intervals **⇧2–⇧9**. For example, `C`, `⇧3`, `⇧5` makes C–E–G. They all share the first
note's duration and take a single input beat. Up to **8 distinct staff positions per
chord** are allowed, in either hand.

You can also turn on **Acorde: sí** in the bar or with **⌥A**: plain letters add notes
to the selected element. Turn it off to continue with the next value. If no note is
selected, the mode enters the first one normally.

A click on any head selects the whole chord. The arrows transpose it; **⌥Backspace**
removes its highest note, keeping at least one. **Backspace** turns the whole chord
into a rest of the same duration. You can undo each change. Audition, playback, pause
and following include all its notes. Open
[`examples/piano-acordes.partitura`](examples/piano-acordes.partitura) to try it.

## Score views

The ribbon's **Vista** tab switches between **Página**, **Continua** and
**Pentagrama**. They are also in the **Vista** menu.

- **Página:** numbered pages with breaks between systems. Use the ‹ › arrows in the
  status bar to turn pages. Zoom and the window size keep the layout of the pages.
- **Continua:** lines that fit the available width, with vertical scrolling.
- **Pentagrama:** all bars in one horizontal strip, with the instruments aligned and
  sideways scrolling.

**⌘0** fits a whole page to the window. The **− / +** buttons and the zoom shortcuts
work in all three views. Changing view keeps the selection, the input point and
playback. If a full system needs more room because of its density or number of
instruments, the pages grow to keep it.

## Sharps, flats and naturals

In **Escribir › Alteración**, choose Armadura, **♯**, **♭** or **♮** before writing. That
option holds for the next notes and for those you add to the chord with ⇧+letter.
Armadura uses the chosen key; ♮ forces a natural note with an explicit natural sign.

To change an existing note, select it and press **Aplicar ♯**, **Aplicar ♭** or
**Aplicar ♮**. In a chord, first choose its head with a click or its name in the
**Selección** tab; the head in darker blue shows which one will change. **⌥↑ / ⌥↓**
step through its notes. As in Sibelius, **8 / 9 / 7** apply sharp, flat and natural,
and pressing the same one again removes the note's own accidental so that it follows
the key signature. With no selection, these keys change the input option. Altering an
existing note does not change the input. Each change can be undone and is heard with
audition on.

A note that follows the key signature after its own accidental gets a cautionary sign
when needed in that bar. Notes in other octaves do not inherit an individual
accidental. The notation state resets in every bar and is kept separate between hands.
The signs of chords are spread in columns. Transposing with the arrows keeps the kind
of the note's own accidental. There are no double accidentals or automatic enharmonic
changes yet.

You can open [`examples/piano-alteraciones.partitura`](examples/piano-alteraciones.partitura).

## Key signatures

Click **Tonalidad** in the strip above the score (or press `K`), choose **Mayores** or
**Menores** and select the key in the editor that opens in the right panel. They are
also in the **Tonalidad** menu. There are 30 options, from seven flats to seven sharps,
including the relative minors. The key signature appears after the clef in every
system, on every staff.

With **Escribir › Alteración › Armadura**, notes and chords use its sharps or flats in
every octave: in G major, writing F produces F♯ without repeating a sharp before each
note. **Aplicar ♮** (`7`) writes an F natural; **Seguir armadura** or pressing `7` again
turns it back into F♯. The Selección tab shows the written pitch and, if the part
transposes, the one that will sound («Suena …»). Audition, start from selection and
pause use the same key.

In **Aplicar desde**, choose **Inicio de la pieza** or the **selected bar**. The change
affects every staff from that bar to the next key change, including the written notes
that follow the key signature. It keeps their positions and the individual
accidentals. You can undo it or use **Quitar cambio de tonalidad** to bring back the
previous key signature. Minor keys use their usual signature; the accidentals of the
harmonic or melodic minor are written individually.

Try [`piano-sol-mayor.partitura`](examples/piano-sol-mayor.partitura) or
[`piano-mi-bemol-mayor.partitura`](examples/piano-mi-bemol-mayor.partitura).

## Time signature changes

Select a bar, even an empty one, and press **T** or the **Métrica** value in the strip
above the score. Choose **Inicio de la pieza** to change the initial time signature, or
**Compás N seleccionado** to add a change during the piece. You can choose a common
time signature or set the numerator and denominator and press **Aplicar**.

The new time signature applies to every staff until the next change. It keeps the
existing notes and bars; if a note does not fit in the new duration, the application
shows the reason and leaves the piece intact. **Quitar cambio de métrica** brings back
the previous time signature. Time signature and key changes can be undone and redone,
are saved in the file and are reflected in playback, the counter and the time ruler.

## Tempo marking

Click the **Tempo** value, **Texto › Tempo › Indicación** or press **⌥⌘T**
(`Ctrl+Mayús+T` on Windows and Linux). Choose **Inicio de la pieza** or the bar where
the change should start. Type a marking such as «Andante cantabile», enter **20 to 400
beats per minute** and choose the beat value, dotted if needed. **Aplicar tempo** or
**Enter** writes the marking above the first staff. You can also use the editor's usual
markings; their values are editable.

The tempo applies to every instrument until the next change. Click a written marking
to change it or use **Quitar cambio de tempo** to bring back the previous one. Changes
are saved and can be undone. The transport shows the tempo at its position; **− / +**
change that value and keep the position, stopping the sequence to rebuild it when you
play again.

## Listening while you write

`cargo run` includes the adapter for **VSCO CE**, an
[optional sound pack](docs/vsco.md): if its samples are installed, its original SFZ
recordings are chosen from the **Sonido** tab; if not, only GeneralUser appears. To
build without the adapter, use `--no-default-features --features runtime-shaders`.
GeneralUser remains the initial choice; changing pack does not change the score. The
banks implement a [common audio contract](docs/sound-banks.md). Every pack plays
through the sfizz engine that ships with Levare. An installed Levare looks for packs
only in its own folder (`soundpacks`, inside `Levare.app` on macOS, next to the
executable on Windows and Linux) and in the `soundpacks` folder of its data folder.

**Audición al escribir** is on at startup: each note or chord entered or transposed
sounds briefly with its instrument's sound, the dynamic in force and its articulation.
Changing a dynamic or articulation plays the selected note again. Staccato shortens the
audition and, if the bank has them, uses real short samples. Accents and marcato
strengthen the attack; tenuto keeps the full duration. Rests and refused edits do not
trigger a note. You can turn audition off from the sound bar.

Select a note or rest and press **Espacio** or **Reproducir** to start from that
element. With no selection, it starts from the beginning. The selection appears in
blue; the active notes and rests are colored orange, stems included, without boxes
that hide the staff. A playhead with a thin line moves continuously with the audio,
also between notes and during rests. The bottom ruler shows bars and beats: click to
place the playhead on the nearest beat and press **Espacio** or **Reproducir** to start
from there. During playback, the click jumps straight to the chosen point; while
paused, it stays paused at the new position. The note selection is kept when you use
the ruler. The main counter shows bar and beat and keeps the elapsed time below. The
view scrolls automatically when needed to keep the playhead visible, also when moving
to another line or in very wide bars.

During playback, **Espacio** or **Pausar** suspends the sound. **Espacio** or
**Continuar** resumes from the same point, even in the middle of a note or rest. Pause
keeps the state of every instrument and does not attack the note again. While paused
you can scroll freely; when you resume, the view follows the playback point again.
You can also place the playhead by **clicking directly on the score**: a note or rest
chooses its exact entry and the staff space chooses the nearest beat. It works stopped,
playing and paused; it only changes the transport's position. **Shift + click/drag**
keeps the playhead so you can extend the musical selection.

If you extend the selection with Shift while paused, Continuar keeps the paused point;
to start from the new selection, press **Detener** and then **Reproducir**.

**Esc** and **Detener** cancel playback and pause, keeping the selection. With playback
already stopped, Esc clears the selection so you can play from the beginning again.
Editing or undoing stops the previous sequence and, where appropriate, the new note
sounds. Rests and the remaining duration of the inner and final bars shown by the ruler
are respected.

### Practice and review

- **Bucle** (`⌥P`, or the transport's loop button): repeats what is selected. With a
  passage selected it repeats from its first to its last note; with a single note, its
  bar; with no selection, the whole score. The jump back to the start is exact, and the
  notes that were already sounding at that point sound again. If you turn it on while
  playing outside the passage, playback jumps to its start. To change the passage, turn
  the loop off, select another one and turn it on again.
- **Media velocidad** (`⇧Espacio`, or **½×**): plays at 50 % of the written tempo,
  without changing the score's markings. When stopped, it starts playing; when playing,
  it changes speed at the same point. It also works with the loop.
- **Repetir la última reproducción** (`⌥Espacio`; `Alt+Espacio` on Windows,
  `Mayús+Alt+Espacio` on Linux): starts again from where the previous playback started.
- **Línea de reproducción al inicio** (`⌘↖` or `⌘[`; `Ctrl+Inicio` on Windows and
  Linux): takes the playhead to bar 1, playing or stopped.
- **Compás siguiente** (`⌘→`) and **Inicio del compás** (`⌘←`): select the first note of
  the next bar, or of the current bar; if you are already there, of the previous one.
- **Ir al compás…** (`⌥⌘G`, `Ctrl+G` on Windows and Linux, or a click on the
  **COMPÁS · TIEMPO** counter): type the number and press Enter. If it is playing,
  playback jumps there too.

The initial tempo is **120 quarters per minute**. Its markings and changes are part of
the `.levare` file. The volume is adjusted without stopping playback and remains a
session setting.

Sound uses the system's default output. If it is not available, editing keeps working
and **Reintentar audio** appears to reconnect it. To check the device briefly without
opening the interface:

```sh
cargo run -p score-platform --example audio_smoke
```

## Files, autosave and recovery

`.levare` files are ZIP archives: an uncompressed `mimetype` entry
(`application/vnd.levare.score`) and `score.json`, compressed, holding the versioned
JSON schema. An 84-bar orchestral score takes under 20 KB. `.partitura` files from
earlier versions are that same JSON uncompressed; they still open, and **Guardar**
offers a new `.levare` beside the original, named after it, without changing the
original. Levare versions from before `.levare` cannot open these files. Files are
recognized by their contents, not their extension. If you delete the extension in the
save dialog, Levare adds `.levare`, and asks before replacing a file that already has
that name. Files are written by atomic replacement, keeping the file's permissions and
any symbolic link; opening, creating or closing asks to discard changes if there are
any.

If a score cannot be opened or saved, a window names the file and explains why in
Spanish (it does not exist, there is no permission, it is read-only, it is damaged or
not a score, it is too large, the disk is full…); a score saved by a newer Levare says
so and asks you to update. The technical cause goes to the log.

Changes are copied automatically to recovery after 750 ms without edits (or after 3
seconds if you keep writing). This separate copy does not change your original file.
After a forced quit, the next start recovers the last completed copy, keeps the
original path and shows it with unsaved changes. Saving or explicitly discarding
clears the recovery. If the recovery cannot be read or restored, Levare moves it aside
in the data folder as «Recuperación sin abrir …», says where it is and can show it, so
the next autosave does not overwrite it. Only one session can use the same recovery
folder.

Quitting from outside the window (the Dock, logging out) cannot ask about unsaved
changes, so Levare writes the autosave one last time and waits for it. If that fails,
or there is no autosave (another Levare window holds it), the unsaved score is saved as
a dated «Copia de emergencia ….levare» in the data folder, never over another file,
and added to «Abrir reciente». Restarting into an update also waits for the autosave
and does not restart if it fails.

## Levare's data folder

Levare keeps its settings and working files in a `Levare` folder (or the older
`Partitura` one, if it already exists) inside the system's local data folder:
`~/Library/Application Support` on macOS, `%LOCALAPPDATA%` on Windows and
`~/.local/share` (or `$XDG_DATA_HOME`) on Linux. `LEVARE_DATA_DIR`, set to an absolute
path, points it elsewhere. Among other things, it holds:

- `atajos.json` (your shortcuts), `recientes.json` (recent files), `paneles.json`
  (panel layout), `sonidos.json` (sound priority) and `actualizaciones.json` (update
  settings);
- `recovery.partitura`, the autosave, and any emergency copies or set-aside
  recoveries;
- `soundpacks/` (installed sound packs) and `actualizaciones/` (downloaded updates,
  emptied when Levare opens);
- `registro.log`, a log with timestamped lines (it moves to `registro.1.log` past
  1 MiB), and `fallo-<time>.txt` crash reports with the version, system, message and
  a backtrace; the newest ten are kept.

On the next start after a crash, Levare says «Levare se cerró inesperadamente» once
and offers **Mostrar el informe**, which shows the report in the file manager so you
can send it with a description of what you were doing. If the window cannot open (no
usable GPU, a broken driver), the reason goes to the log and a report, and Windows also
shows a message.

## Correcting notes and bars

Click a note to leave input and edit it: **A–G** replaces the chosen pitch; the values
in **Escribir › Duración** change its duration, including any dot or triplet it already
had. In chords the rest of the notes are kept. If the new duration does not fit, the
operation is refused entirely. **N** turns new note input back on.

**Backspace/Delete** turns the selected notes into rests of the same duration; the
following notes keep their beat. **Edición › Eliminar notas y su tiempo** and
**Cortar** remove the elements and move the following ones earlier within the bar.
Everything supports undo and redo.

In **Partitura › Compases**, **Antes** (⇧⌘B) adds an empty bar before the current one
and **Eliminar** (⌘Backspace) deletes the bars the selection spans, or the current one
if there is no passage. They affect every instrument and hand; they move the markings
with the music and keep the effective time signature, key and tempo of the first bar
kept. At least one bar always remains.

## Repeats, endings and jumps

In **Partitura › Repeticiones** write the start and end of a repeat, first and second
endings, the double barline and the final barline. **Da capo y segno** has Segno, Coda
and Fine, and the **D.C. / D.S.** dropdown gathers To Coda and the jumps; the
**Repeticiones** menu also gathers the six D.C./D.S. variants and the options to remove
the form.

Select a passage to place the opening marks on its first bar and the closing ones on
the last; an ending spans the whole range. Without a passage, the cursor's bar is used.
The selection is kept when adding several marks and each change can be undone.
Pressing an active barline or mark again removes it. Endings are removed from the
menu. The last bar has an automatic final barline. The marks follow their bars when
music is inserted or deleted and are kept in the native file, PDF and MusicXML.

**Reproducción › Práctica › Repeticiones** is on when the session starts. The cursor
goes back to the bar that is sounding and the clock keeps counting the elapsed time.
After a D.C./D.S. the repeats are skipped and the last ending of each group is played.
Changing this option stops the transport; turning it off lets you hear the music in
written order.

## Single-note and chord tremolos

Select a note, chord or passage without rests and choose **Articulaciones › Trémolo**:
one, two or three strokes. The mark crosses the stem; on whole notes it goes above the
head. Another variant replaces the previous one and pressing the active one again
removes it. You can also select the mark and delete it while keeping the note. Each
change to the passage is undone in one step.

Playback spreads eighth, sixteenth or thirty-second attacks within the written
duration, also with ties, fermatas and repeats. On strings, the three-stroke tremolo
uses the sound pack's tremolo recording when it has one
([details](docs/sfz.md#trémolo)). Only the first attack keeps the accent. The marks are
kept when copying and pasting and in the native, PDF and MusicXML formats.

## Rehearsal marks

**Texto › Marca de ensayo › Añadir** or **⌘R** adds or removes a mark at the start of
the selected bar, or of the passage's first bar. The **Letras**, **Números** and
**Nº de compás** controls (**Texto › Estilo de ensayo**) choose the style for the whole
work: A, B… Z, AA, BB…; 1, 2, 3…; or the bar's own number. Labels are renumbered when
marks are added or removed and when bars are inserted or deleted. Changing style keeps
the position of the barlines.

The marks appear in a box above the first staff and go along with extracted parts.
They are saved with the work and kept in PDF and MusicXML. When importing MusicXML, a
custom text such as «Trío» is kept without stopping the other marks from being numbered
automatically. Adding or removing a mark and changing style support undo and keep the
selected passage.

## Exporting the full score and parts

**Exportar…** in the title bar or **Archivo › Exportar partitura y partes…** opens a
picker with the **full score** and each **instrumental part**. You can tick them
separately, select all or clear all the ticks. Choose **PDF**, **MusicXML** or both; the
dialog shows how many files will be produced.

**Elegir destino…** lets you name and place a new folder. Each document and format
produces a separate file: `00 - Partitura general.pdf`, `01 - Violín I.pdf`,
`02 - Violín II.pdf`, and so on. You can also export a single part. If the folder
already exists you are asked for another name, without mixing or replacing its
contents. Cancelling the destination keeps the selection.

Parts keep all their voices and staves (both piano hands and divisi passages
included), as well as notes, expressions and global changes of time signature, key and
tempo. They keep the section names «Violín I» and «Violín II». Their layout is
recalculated as standalone scores. Pitches are in concert pitch, as in the editor;
transposed parts are not generated yet. Exporting does not change the work or mark its
changes as saved.

```sh
cargo run -p score-export --example export -- examples/menuetto-y-trio-cuerdas.partitura output/menuetto-partes
```

The parent folder must exist and `menuetto-partes` must be new. This example produces
the full score and every part in both formats.

## Exporting PDF

Select **PDF** in the export dialog to save every page on A4. The PDF uses Bravura's
vector outlines, includes the markings and arpeggios across the hands, and lays out the
pages independently of the screen's view and zoom. It includes no selections or
interface controls. Individual parts carry the instrument in the header, without
repeating it on every system. They use a more compact staff and up to six bars per
system; they keep the divisi and piano hand labels. Vertical space is calculated from
the real ink and the markings move closer to the music below them. The full score uses
the same compact spacing in the app (Página, Continua and Pentagrama) and in PDF
export.

To export an example without opening the interface:

```sh
cargo run -p score-pdf --example export -- examples/piano-arpegios.partitura exports/piano.pdf
```

The destination folder must exist.

## Exporting MusicXML

**Archivo › Exportar MusicXML…** opens the same picker with MusicXML ticked. Each
selected document is saved as uncompressed **.musicxml** (MusicXML 4.0) to exchange the
work with other editors. It includes instruments, clefs, notes, chords, rests, two
voices, dots and triplets; changes of time signature, key and tempo; dynamics,
articulations, ties, slurs, hairpins, fermatas, techniques, harmonics and arpeggios,
including those spanning both hands; grace notes, 8va and 8vb lines, pedal, trills and
glissandi, expression and technique text, lyrics (`lyric`, with their verses, hyphens
and extenders) and chord symbols (`harmony`).

Divisi keeps the voices in a single instrument, reserves a second staff and hides it in
the bars outside the passage. Changes within the bar keep their position; if they cut a
sustained note, its fragments are joined with ties. The exact layout depends on the
receiving editor. Pitches are exported in each instrument's written pitch, with its key
signature and transposition. Artificial harmonics include the stopped note and the
diamond touch note; their sound, like the rest of playback, depends on the receiving
program. Levare's own synthesized timbre is not stored in the file.

Export writes a copy and does not replace the native document or change its saved
state. Compressed **.mxl** export is not available yet.

```sh
cargo run -p score-io --example export_musicxml -- examples/menuetto-y-trio-cuerdas.partitura output/menuetto.musicxml
```

The destination folder must exist.

## Opening MusicXML

**Archivo › Abrir…** (⌘O) accepts native **.levare** and **.partitura** documents,
MusicXML **.musicxml** and **.xml**, and compressed **.mxl** files. They are recognized
by their contents, and both partwise and timewise structures are accepted.

Importing creates an editable copy with no native path and marks it as unsaved.
**Guardar** asks for a **.levare** file; the original MusicXML is not changed. The
imported copy also takes part in autosave and recovery.

Notes, chords of uniform duration, rests, up to two voices per staff and two staves per
instrument are kept; time signature, key signature, tempo, dynamics, basic
articulations, complete ties and slurs, hairpins, fermatas, arco/pizzicato, harmonics
and arpeggios. Transposing parts keep their transposition interval and are rebuilt in
written pitch. Pickups are represented with a shortened first bar. Divisi can be rebuilt
from the div./unis. markings or the visibility of the second staff. The page layout is
recalculated with Levare's engraving.

If something is left out or adapted, a warning appears on opening: for example, lyrics,
free text, ornaments, chord symbols, instruments without an equivalent and ties or
slurs that affect only some notes of a chord. Cases that cannot represent the music
correctly are refused with an explanation, including more than two voices per staff,
rhythms beyond the supported resolution, microtones, different simultaneous time
signatures and touch harmonics with no deducible sounding pitch. Natural harmonics
without a pitch type are read at the written pitch with a warning. The artificial
harmonics accepted are at the fourth.

```sh
cargo run -p score-io --example import_musicxml -- obra.musicxml copia-editable.levare
```

## Updates

A release build of Levare looks for a newer version on GitHub ten seconds after it
opens and every six hours, downloads it into the data folder and checks its signature.
The first time it looks, the status bar says so and names the command that turns it
off, «Activar o desactivar la búsqueda automática de actualizaciones», in the
**Levare** menu on macOS and the **Archivo** menu on Windows and Linux; the notice
appears once, also after updating from an older version. **Acerca de Levare…** says
whether the checks are on. **Buscar actualizaciones…** checks right away.

When the update is ready, the status bar shows «Reiniciar para actualizar a X.Y.Z».
**Reiniciar para actualizar** swaps the installation and restarts; unsaved work comes
back through the autosave and a saved score opens again. It does not restart while the
agent is working, something is being saved or exported, or sounds are being installed;
the status bar says why. Development builds do not update.
[docs/releasing.md](docs/releasing.md#the-in-app-updater) describes the whole process.

## Verifying

```sh
cargo fmt --all --check
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
```

CI runs them on macOS, Windows and Linux, and opens the app on Windows and Linux.
Without a Windows or Linux machine, `cargo check` and `cargo clippy` can still target
them from macOS with `--target x86_64-pc-windows-msvc` or
`--target x86_64-unknown-linux-gnu`.

To work only on the core, without graphics or audio dependencies:

```sh
cargo test -p score-core -p score-edit -p score-layout -p score-io -p score-document -p score-keymap
```

`docs/atajos.md` is generated from `crates/keymap`; after changing commands or
shortcuts, regenerate it with `LEVARE_ACTUALIZAR_ATAJOS=1 cargo test -p score-keymap`.

The architecture, decisions and next milestones are in
[docs/desarrollo.md](docs/desarrollo.md). The reference inventory continues in
[docs/herramientas.md](docs/herramientas.md).

## Releasing

Versions come from the commit messages (Conventional Commits, see `AGENTS.md`) through
release-plz, and the Release workflow builds and publishes all three platforms from the
same commit: the signed universal macOS app as a `.dmg` and a `.zip`, the Windows
`.zip`, the Linux `.tar.gz`, and the signed `latest.json` the in-app updater reads.
Process and secrets: [`docs/releasing.md`](docs/releasing.md).

## Current scope

Two voices per staff and divisi on one or two staves, chords of up to eight notes with
sharps, flats and naturals, six durations (from the whole note to the thirty-second)
with dots and triplets, and treble, C and bass clefs. The engraving includes beaming,
ledger lines, articulations, dynamics, ties and slurs, hairpins, grace notes, lines,
text, lyrics and chord symbols, with a per-bar cache.
Playback mixes the parts on a common clock, with start from selection, pause and
resume, loop and half speed. There is no external MIDI yet.

Levare is released under the [MIT license](LICENSE). It includes
[Bravura 1.392](https://github.com/steinbergmedia/bravura/tree/bravura-1.392),
copyright Steinberg Media Technologies GmbH, under the
[SIL Open Font License](assets/fonts/OFL.txt). It also includes
[GeneralUser GS 2.0.3](assets/sounds/README.md) by S. Christian Collins, with its
[original license](assets/sounds/LICENSE.txt). Every download carries a `licenses`
folder with these, the Liberation and Cantarell fonts, sfizz and every Rust library
built into Levare; **Acerca de Levare… › Ver licencias** opens it.
