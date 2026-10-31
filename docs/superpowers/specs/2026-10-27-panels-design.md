# Panels that make sense

Redesign of Levare's side panels so that they are genuinely practical. The user approved the
navigable mockup <https://claude.ai/artifact/QH3h8MK2WJ4ac5urJxoPwc> (private; its source is next
to this document: `2026-10-27-panels-mockup.html`). This document fixes what it decides.

## Problems today

1. The same thing in three or four places: dot, triplet, accidental (twice inside Properties and
   in a different order than in the ribbon), voice, div./unis., rest, chord, octave, articulations,
   dynamics, hairpins and slurs are in the ribbon and in Properties; bar, beat, tempo, meter, key
   signature and voice are in the top strip, the transport, the status bar and Properties;
   «Añadir instrumento» four times.
2. The left panel does two jobs: instrument list and host of the editors for clef, key signature,
   meter, tempo, text, lyrics, chord symbols and score information, which push the list down.
   Below it, a fixed text «Orden automático por familias» and a «Vista» block that repeats the
   ribbon.
3. Properties does not change with the selection, is full of help paragraphs, has two «Pendiente»
   sliders that do nothing, and read-only pitch and duration.
4. With 32 parts, the list and the Mixer (a block with two sliders per part) are scrolling screens.
5. Nothing is remembered: open panels and the chosen tab are lost on close.
6. The inspector tabs do not fit on one line.

## Decisions

### Left: «¿Qué hay en la partitura?» (What is in the score?)

- Header: «Partes» (Parts) with the number of parts and a single «+» button (Add instruments, key
  I). Below, a two-option selector «Lista | Mezcla» (List | Mix) and a «Buscar parte» (Search
  part) field that filters by name (ignoring case and accents).
- Parts grouped by family; each family is a collapsible header with its number of parts. A search
  shows the families expanded.
- **Lista:** a 28px row per part: name on the left; on the right, in `MUTED`, its clef or clefs
  and the transposition if it has one («Si♭ · Sol», «Fa · 8vb»). Clicking the row selects it as
  today. The chosen row takes the ribbon's selected look. Only the chosen row shows, if the part
  has two staves, its chips (M. derecha / M. izquierda or Divisi I / II) and the delete button.
- **Mezcla:** each row adds volume and stereo position (the sliders that are today in the Mixer)
  and, for strings, woodwinds and brass, «Sección | Solista» (Section | Soloist). The volume and
  position settings remain per session; Solista or Sección is saved and undone as today.
- Footer: a discreet line with the chosen part and how to move («⌥↑ ⌥↓»), or in Mezcla the master
  volume.
- Out of the left panel: all the editors, the text «Orden automático por familias» and the
  «Vista»/«Páginas» block. Page navigation stays available where it already is in the ribbon and
  the menus; if today it lives only here, it moves to the bottom status bar.

### Right: «¿Qué tengo seleccionado y qué le cambio?» (What do I have selected and what do I change?)

Three tabs on one line, as a three-option selector: **Selección · Sonido · Revisión**
(Selection · Sound · Review).

**Selección** changes with the selection:

- **Nothing:** «Partitura». Editable title, subtitle and composer (the score information editor).
  Rows that open their editor: Tonalidad, Métrica, Tempo inicial. Data: Compases, Partes,
  Duración.
- **Note or chord:** the part name; «Compás N, tiempo T · voz V»; the pitch, large (written), and
  the duration; «Suena …» if the part transposes; the range warning if there is one; **Marcas en
  esta nota**: one row per mark (articulation, dynamic, technique, ties, slurs and hairpins that
  start or end on it, grace notes, texts) with its glyph, its name and a ✕ button that removes it
  (undoable); **Texto en esta nota**: Letra ⌘L, Cifrado ⌘K, Comentario.
- **Rest:** like the note, without pitch or «Suena», with its marks.
- **Passage:** «Pasaje» with the parts and bars it spans; Notas, Partes, Marcas; **Hacer con el
  pasaje**: commands that already exist (up/down a step, up/down an octave, Copiar, Convertir en
  silencios, Comprobar selección); **Marcas en el pasaje** with ✕, each with its bar.
- **Open editor** (clef, key signature, meter, tempo, text, lyrics, chord symbols, information):
  the editor takes over the tab, with Aplicar and Cancelar (Esc). Opening it from the ribbon, the
  menu or the top strip shows the right panel and the Selección tab.
- No help paragraphs (their text goes to the tooltip) and no controls that are already in the
  ribbon, nor the «Pendiente» sliders.

**Sonido:** output (master volume, reverb and its settings), sound of the chosen part (bank,
sound, natural/literal performance) and Más sonidos (More sounds), as today. The per-part setting
for volume and position lives in Mezcla, on the left.

**Revisión:** what the Revisión tab has today (check, warnings, ranges, comments), with its help
paragraphs moved to the tooltip.

### Top: «¿Dónde estoy?» (Where am I?)

The strip above the score is the only place for position and context: Compás, Tiempo | Tempo,
Métrica, Tonalidad (editable: they open their editor in the right panel) | Parte · Voz | summary
of the selection. Tonalidad stops opening the left panel.

### Bottom

The status bar keeps the status message, page navigation in Page mode, and the save and autosave
state. It stops repeating voice, key signature, meter, instruments and view mode.

### Memory

Saved in the data folder, in `paneles.json`: which panels are open, the right tab, Lista or
Mezcla, and the collapsed families. If the file is missing or damaged, the defaults are used (left
and right open, Selección, Lista, nothing collapsed).

## Out of scope

- New features (transposing by a free interval, mute/solo per part, resizable panels).
- The ribbon, the title bar and the transport except as stated above.
