# Clear ribbon

Redesign of Levare's ribbon (toolbar) so that it is very easy to use and understand. The user
approved the navigable mockup: <https://claude.ai/artifact/Uu2pdnXpMEo2Mx2KBbYJyq> (private; its
source is in the plan). This document fixes what the mockup decides.

## Problems today

1. Five ways of drawing a button (sunken segments, boxes with a border when off, loose glyphs,
   large buttons with text, small rows) and each group on a grey card: everything has a box and
   it is hard to tell what is active, what is a choice and what is an action.
2. Musical symbols with no visible name or key.
3. Unbalanced tabs: Layout with a group of two buttons; Notation with ten groups that
   collapse; Home mixes clipboard, instruments and bars.

## Decisions

### A single control grammar

- At rest: the glyph or text on the panel, with no background or border.
- Hover: one step of tone (`HOVER`).
- Active or chosen (toggle or choice, the same): `SELECT` fill, `SELECT_BORDER` border,
  `ACCENT_TEXT` ink.
- Options that exclude each other (duration, accidental, voice, rehearsal style, view mode)
  share a `FIELD` track with 2px of padding.
- Action: no state; only hover and press.
- Groups have no card: they are separated by space (20px) and carry their name underneath, 11px,
  `MUTED`, left-aligned.
- Pending: keeps the dashed border (`PENDING_OUTLINE`).

### Visible name and key

Each control is a tile: glyph on top (28px tall), below it the name (10.5px, `SECONDARY`) and
below the name the key (10px, `MUTED`), e.g. «Negra» and «4». Name and key go on two lines so
that the tile is as wide as the wider of the two, not their sum: on one line, Escribir did not
fit at 1440px. Where the group title already gives the context, the name is short («Unión»,
«Arriba»). When width is short, the group switches to normal mode: glyph and key, no name. If it
is still short, it collapses into its dropdown button, as it does today. The tooltip shows the
name, the key and a sentence about what it does.

### Tabs by task

(Tab and group names are shown as they appear in the app, in Spanish, with an English gloss.)

| Tab | Groups |
|---|---|
| Escribir (Write) | Entrada (Input: Escribir N, octave ↑ ↓) · Duración (Duration) · Modificar (Modify: dot, triplet, rest) · Alteración (Accidental) · Ligadura (Tie and slur: tie, slur) · Voz (Voice) · Acorde (Chord: chord mode; intervals in a dropdown) |
| Articulaciones (Articulations) | Articulación (Articulation: staccato, tenuto, accent, marcato, fermata) · Nota de adorno (Grace note) · Arpegio (Arpeggio) · Trémolo (Tremolo) · Técnica de cuerda (String technique) |
| Dinámicas (Dynamics) | Dinámica (Dynamic: ppp…fff) · Regulador (Hairpin) · Líneas (Lines: 8va, 8vb, pedal, trill, glissando) · Divisi |
| Texto (Text) | Partitura (Score: title and credits) · Tempo · Sobre el pentagrama (Above the staff) · Letra y cifrado (Lyrics and chord symbols) · Marca de ensayo (Rehearsal mark) · Estilo de ensayo (Rehearsal style) |
| Partitura (Score) | Instrumentos (Instruments) · Compases (Bars) · Cambios (Changes: clef, key signature, time signature) · Repeticiones (Repeats) · Da capo y segno (D.C./D.S. in a dropdown) · Sistema y página (System and page) |
| Reproducción (Playback) | Reproducir (Play) · Práctica (Practice) · Sonido (Sound) |
| Revisión (Review) | Comprobar (Check) · Avisos (Warnings) · Registro del instrumento (Instrument range) · Comentarios (Comments) |
| Vista (View) | Modo (Mode) · Zoom · Cinta (Ribbon) |

The clipboard leaves the ribbon (Edit menu and keys). The panels leave View (they are in the
header). At 1440px wide no tab collapses.

### Tabs and status

- Text tabs with a 2px blue indicator under the chosen one; no rounded background.
- The status pill to the right of the tabs always says what is about to be written:
  «Escribiendo · Negra con puntillo · Voz 2», with a blue dot when input is active.

### Tab shortcuts

⌥⌘1–8 follow the new order. The published IDs do not change name: `TAB_NOTE_INPUT` opens
Escribir, `TAB_NOTATION` Articulaciones, `TAB_HOME` Partitura, and `TAB_TEXT`, `TAB_PLAYBACK`,
`TAB_REVIEW` and `TAB_VIEW` their own; `TAB_DYNAMICS` is added. `TAB_LAYOUT` stays registered,
opens Partitura and has no default key.

## Out of scope

- Changes to the menus, the properties panel or the bottom status bar.
- New features: only what exists is reorganized and redrawn.
