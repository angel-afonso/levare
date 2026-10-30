# Instrument ranges

The editor marks in red the pitches outside the reference range of each instrument.
The limits are inclusive and are compared in **sounding pitch**, with C4 as middle C.
The written key signature, the explicit accidental, the transposition stored in the
part and harmonics all take part in the comparison. This also makes it possible to
open old or imported documents with a different transposition.

These are warnings about the usual professional range, not absolute physical limits
or difficulty levels. They do not prevent editing, saving, playing or exporting. They
are not deduced from the sound bank and no colors are stored in the document. The red
ink is applied only in the editor; selection and playback keep the warning. Marks,
rests and staff lines keep their color.

## References and conventions

Orchestral ranges are based on the **Range** sections of
[Vienna Academy](https://www.vsl.co.at/academy), especially
[strings](https://www.vsl.co.at/academy/strings/violin),
[clarinets](https://www.vsl.co.at/academy/woodwinds/clarinet),
[flute](https://www.vsl.co.at/academy/woodwinds/concert-flute),
[horn](https://www.vsl.co.at/academy/brass/horn-f) and
[trombone](https://www.vsl.co.at/academy/brass/tenor-trombone).
When the catalog does not distinguish models, the editor adopts a fixed reference:

- Flute with B foot: B3–D7; standard piccolo: D5–C8.
- Five-string orchestral double bass: B0–G4; bass clarinet with low C:
  B♭1–B5; baritone saxophone with low A: C2–A4.
- Saxophones with written high F♯, without altissimo register.
- Tenor trombone: E2–F5, not including pedal tones; bass trombone: B♭0–F5.
- Generic tuba: E1–F4. Timpani: usual set: C2–A3; the distribution among drums
  and the time needed to tune them are not evaluated.
- Four-valve [B♭ piccolo trumpet](https://www.vsl.co.at/academy/brass/piccolo-trumpet):
  D4–G6 in sounding pitch (E3–A5 written).
- 19-fret classical guitar: E2–B5; 88-key piano: A0–C8;
  pedal harp: C1–G7.
- Five-octave marimba: C2–C7; vibraphone: F3–F6; celesta of four
  sounding octaves: C4–C8. Organ: manuals C2–C7 without register transpositions.

The full table sits next to the model's catalog, in
`crates/score-core/src/instrument_range.rs`. Changing clef does not change the
range. Each written notehead of a chord is evaluated separately, also
in the second voice and in both staves of a part.

## Harmonics

Notes with a natural harmonic are written at their sounding pitch. Their lower
limit is the second partial of the lowest string. Artificial harmonics at the fourth
sound two octaves above the stopped note; the contact diamond does not count
as another note. The maximums are D8 for violin, E7 for viola, A7 for cello
and G6 for double bass, according to the Vienna Academy string references.

The warning checks the sounding range, not the existence of each natural partial,
the fingering of a chord or the difficulty of a technique.

## Custom ranges and review

**Revisión › Rangos › Editar rango…** (Review › Ranges › Edit range…) lets you
replace the usual reference of the active part with your own limits in sounding
pitch. Scientific names (`C4`, `F#3`, `B♭5`) or Spanish names (`Do4`, `Fa♯3`,
`Si♭5`) are accepted, from C0 to G9. Both ends are inclusive; the lower limit cannot
exceed the upper one. Custom limits apply to the sound of all events, including
harmonics. **Restablecer rango habitual** (reset to the usual range) removes the
exception and restores the usual limits and the specific treatment of harmonics
described above.

The exception is saved in the part, is kept when extracting it and supports history.
Native format 26 keeps read compatibility with earlier files, which keep the usual
values. MusicXML and PDF do not include these review limits. They do not change the
notes, the sound or the exported engraving.

The **Notas en rojo** (Red notes) button changes only the visibility of the warning
during the session. **Comprobar** (Check) still finds out-of-range notes when the
color is hidden. The warnings list lets you navigate to each note of the chord, filter
by part and also review problems with repeats and jumps.
