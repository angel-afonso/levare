# Ribbon laid out as Sibelius'

The user asked for a ribbon like those of Sibelius and MuseScore, plus the controls they
have that Levare lacks. This first phase only reorganizes the ribbon and adds the commands
Levare already has; new notation (double sharps and flats, more articulations and
ornaments, noteheads, beams, bar and page numbers, page setup, metronome) comes later, one
feature at a time. It replaces the tab table of
[2026-10-26-ribbon-design.md](2026-10-26-ribbon-design.md); its control grammar (tiles with
name and key, choice tracks, groups without cards) stays.

## What the references do

- **Sibelius** has eleven tabs, in the order of the work: Archivo, Inicio, Entrada de
  notas, Notaciones, Texto, Reproducción, Diseño, Apariencia, Revisión, Partes, Vista.
  Durations, accidentals and articulations are not on its ribbon but on the Keypad panel.
  Notaciones shows families of symbols (clefs, barlines, lines, noteheads) as galleries.
- **MuseScore 4** has no ribbon: a note input toolbar above the score (durations, dot,
  rest, tie, slur, tuplet, accidentals, four articulations, flip, voices 1–4) and the
  palettes for everything else.

## Decisions

- Eight tabs in Sibelius' order: **Inicio**, **Entrada de notas**, **Notaciones**,
  **Texto**, **Reproducción**, **Diseño**, **Revisión**, **Vista**. Apariencia and Partes
  return when Levare has their functions. `⌥⌘1–8` (`Ctrl+Mayús+1–8`) follow the order.
- Entrada de notas keeps the old Escribir tab, which is MuseScore's toolbar, instead of
  moving durations to a Sibelius keypad. Silencio returns beside the dot.
- Inicio, as in Sibelius: Portapapeles (Pegar, Copiar, Cortar, Repetir), Instrumentos,
  Divisi, Compases and Seleccionar (Todo, Ir al compás…).
- Notaciones: Cambios (clef, key and time signature), Barra, Articulación,
  Regulador, Líneas, Técnica de cuerda, Nota de adorno, Arpegio and Trémolo.
- Texto takes the dynamics, as Sibelius writes them as Expression text, and the signs and
  jumps in one **D.C. / D.S.** dropdown.
- Diseño: Papel (paper and orientation) and Pentagrama (staff size) dropdowns, Sistema
  y página, Números de compás (each system, every bar, every 5 or 10,
  none), Números de página (footer, outside corner, none), Cabeza (cross, diamond and
  slash noteheads), Barra de corchea (start, join, none) and Posición (Restablecer).
  Noteheads and beams change only the drawing, and Notaciones has no room left at
  1440 px.
- At 1440 px only dropdowns fold. To fit, the families Sibelius shows as galleries are
  dropdowns: Barra, Líneas, Nota de adorno, Arpegio, Trémolo, D.C. / D.S. and
  Estilo de ensayo. Entrada de notas drops the names of Modificar, Alteración and
  Ligadura at 1440; their symbols and keys stay.
- Voz holds the four voices, as Sibelius and MuseScore do. To make room, the chord mode
  joins the intervals in one **Acorde** dropdown; its split button lights up while the
  mode is on. «Título y créditos» is shortened to «Título…».
- Published command ids keep their names. `TAB_HOME` opens Inicio, `TAB_LAYOUT` Diseño;
  `TAB_DYNAMICS` stays registered without a default key and opens Texto.

- Barra is Sibelius' barline gallery: the repeat barlines, the endings and every barline
  style (double, final, dashed, dotted, tick, short and invisible) in one dropdown, which
  keeps Notaciones within 1440 px.

## Left out of this phase

- Pending commands (Símbolo…, Líneas…, Flexi-time, MIDI input): Entrada de notas has no
  room for controls that do nothing yet.
- Dividir and Reunir parte: Divisi stays their only home.
- Quitar instrumento: the ribbon has no remove buttons.
