//! Transposing written music by an interval, as Sibelius' Transpose dialog
//! does: the letters move by the interval's number and the accidentals by
//! its quality, so a major third above F♯ is A♯, never B♭.
use super::*;

/// Letters and semitones to move by; negative values go down.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Interval {
    pub steps: i8,
    pub semitones: i8,
}

/// The intervals Levare offers, up to an octave, with their names.
pub const INTERVALS: [(&str, Interval); 14] = [
    ("Unísono aumentado", Interval::new(0, 1)),
    ("Segunda menor", Interval::new(1, 1)),
    ("Segunda mayor", Interval::new(1, 2)),
    ("Segunda aumentada", Interval::new(1, 3)),
    ("Tercera menor", Interval::new(2, 3)),
    ("Tercera mayor", Interval::new(2, 4)),
    ("Cuarta justa", Interval::new(3, 5)),
    ("Cuarta aumentada", Interval::new(3, 6)),
    ("Quinta disminuida", Interval::new(4, 6)),
    ("Quinta justa", Interval::new(4, 7)),
    ("Sexta menor", Interval::new(5, 8)),
    ("Sexta mayor", Interval::new(5, 9)),
    ("Séptima menor", Interval::new(6, 10)),
    ("Séptima mayor", Interval::new(6, 11)),
];

impl Interval {
    pub const OCTAVE: Self = Self::new(7, 12);
    pub const fn new(steps: i8, semitones: i8) -> Self {
        Self { steps, semitones }
    }
    pub const fn down(self) -> Self {
        Self::new(-self.steps, -self.semitones)
    }
    /// The same interval with `octaves` more octaves.
    pub const fn plus_octaves(self, octaves: i8) -> Self {
        Self::new(self.steps + 7 * octaves, self.semitones + 12 * octaves)
    }
}

/// Semitones of a letter above C.
fn natural(step: Step) -> i16 {
    [0, 2, 4, 5, 7, 9, 11][step.index() as usize]
}

impl Pitch {
    /// The pitch `interval` away, read in `key` and written with the sign
    /// its letter needs (up to a double sharp or flat); `None` when it
    /// would leave the range or need more than a double sign.
    pub fn transposed(self, interval: Interval, key: KeySignature) -> Option<Self> {
        let pitch = self.in_key(key);
        let moved = pitch.shift(i16::from(interval.steps))?;
        let semitones = |p: Pitch| i16::from(p.octave()) * 12 + natural(p.step);
        let alteration =
            semitones(pitch) + i16::from(pitch.alteration()) + i16::from(interval.semitones)
                - semitones(moved);
        let sign = Accidental::from_semitones(i8::try_from(alteration).ok()?)?;
        // What the key already says needs no sign of its own.
        let implied = key.accidental(moved.step).unwrap_or(Accidental::Natural);
        Some(moved.with_accidental((sign != implied).then_some(sign)))
    }
}

impl Pitch {
    /// The same sound with another letter, as Sibelius' Return and
    /// MuseScore's J give it: the spelling with the fewest signs, a sharp
    /// turning to a flat and a flat to a sharp. `None` when there is none.
    pub fn respelled(self, key: KeySignature) -> Option<Self> {
        let pitch = self.in_key(key);
        let semitones = |p: Pitch| i16::from(p.octave()) * 12 + natural(p.step);
        let sound = semitones(pitch) + i16::from(pitch.alteration());
        let up = pitch.alteration() >= 0;
        [-2i16, -1, 1, 2]
            .into_iter()
            .filter_map(|steps| {
                let letter = pitch.shift(steps)?;
                let sign =
                    Accidental::from_semitones(i8::try_from(sound - semitones(letter)).ok()?)?;
                Some((steps, letter, sign))
            })
            .min_by_key(|(steps, _, sign)| (sign.semitones().abs(), (*steps > 0) != up))
            .map(|(_, letter, sign)| {
                let implied = key.accidental(letter.step).unwrap_or(Accidental::Natural);
                letter.with_accidental((sign != implied).then_some(sign))
            })
    }
}

impl ChordRoot {
    /// Moved by `interval`, spelled with at most one sharp or flat.
    fn transposed(self, interval: Interval) -> Self {
        let target = i16::from(self.semitone()) + i16::from(interval.semitones);
        const STEPS: [Step; 7] = [
            Step::C,
            Step::D,
            Step::E,
            Step::F,
            Step::G,
            Step::A,
            Step::B,
        ];
        let letter = |offset: i16| {
            STEPS[(self.step.index() + i16::from(interval.steps) + offset).rem_euclid(7) as usize]
        };
        // The interval's own letter, or its enharmonic neighbour when a
        // chord symbol would need a double sign.
        [0, 1, -1]
            .into_iter()
            .map(letter)
            .find_map(|step| {
                let alteration = (target - natural(step) + 6).rem_euclid(12) - 6;
                (-1..=1).contains(&alteration).then_some(Self {
                    step,
                    alteration: alteration as i8,
                })
            })
            .unwrap_or(self)
    }
}

impl ChordSymbol {
    pub(crate) fn transposed(self, interval: Interval) -> Self {
        Self {
            root: self.root.transposed(interval),
            bass: self.bass.map(|bass| bass.transposed(interval)),
            ..self
        }
    }
}

impl Score {
    /// Respell the notes of `ids`, or only the chord tone `tone` of a single
    /// note, all or nothing.
    pub fn respell(&mut self, ids: &[ElementId], tone: Option<usize>) -> Result<(), ScoreError> {
        // Tied notes keep the same name, or the tie would no longer hold.
        let mut chain: Vec<ElementId> = ids.to_vec();
        let mut index = 0;
        while index < chain.len() {
            let id = chain[index];
            for c in self
                .connections
                .iter()
                .filter(|c| c.kind == ConnectionKind::Tie)
            {
                for (from, to) in [(c.start, c.end), (c.end, c.start)] {
                    if from == id && !chain.contains(&to) {
                        chain.push(to);
                    }
                }
            }
            index += 1;
        }
        let mut next = self.clone();
        // Respell every connection's notes first, then put the ties back.
        let ties: Vec<_> = self
            .connections
            .iter()
            .filter(|c| c.kind == ConnectionKind::Tie && chain.contains(&c.start))
            .copied()
            .collect();
        for &id in &chain {
            let event = *next.event(id).ok_or(ScoreError("La nota no existe"))?;
            if event.pitches.is_empty() {
                continue;
            }
            let key = next.written_key_at(id.part, id.measure);
            let pitches: Vec<_> = event
                .pitches
                .iter()
                .enumerate()
                .map(|(index, pitch)| {
                    if tone.is_some_and(|tone| tone != index) {
                        return Ok(pitch);
                    }
                    pitch
                        .respelled(key)
                        .ok_or(ScoreError("Esta nota no tiene otro nombre"))
                })
                .collect::<Result<_, _>>()?;
            next.set_pitches(id, Pitches::new(&pitches)?)?;
        }
        for tie in ties {
            if !next.connections.contains(&tie) {
                next.add_connection(tie)?;
            }
        }
        *self = next;
        Ok(())
    }
    /// Move the notes, grace notes and chord symbols of `ids` by `interval`,
    /// all or nothing. Rests are left as they are.
    pub fn transpose_by(
        &mut self,
        ids: &[ElementId],
        interval: Interval,
    ) -> Result<(), ScoreError> {
        let out_of_range = || ScoreError("La transposición deja alguna nota fuera del registro");
        let mut next = self.clone();
        for &id in ids {
            let event = *next.event(id).ok_or(ScoreError("La nota no existe"))?;
            let key = next.written_key_at(id.part, id.measure);
            if !event.pitches.is_empty() {
                let pitches: Vec<_> = event
                    .pitches
                    .iter()
                    .map(|pitch| pitch.transposed(interval, key).ok_or_else(out_of_range))
                    .collect::<Result<_, _>>()?;
                next.set_pitches(id, Pitches::new(&pitches)?)?;
            }
            if let Some(grace) = event.notation.grace {
                let moved = grace
                    .map(|pitch| pitch.transposed(interval, key))
                    .ok_or_else(out_of_range)?;
                next.grace_slot(id)?.grace = Some(moved);
            }
            if let Some(chord) = event.notation.chord {
                next.set_chord(id, Some(chord.transposed(interval)))?;
            }
        }
        *self = next;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pitch(step: Step, octave: u8, accidental: Option<Accidental>) -> Pitch {
        Pitch::new(step, octave)
            .unwrap()
            .with_accidental(accidental)
    }

    #[test]
    fn letters_move_by_the_number_and_signs_by_the_quality() {
        let c = KeySignature::default();
        let named = |name: &str| INTERVALS.iter().find(|(n, _)| *n == name).unwrap().1;
        let sharp = Some(Accidental::Sharp);
        // A major third above F♯ is A♯, never B♭.
        assert_eq!(
            pitch(Step::F, 4, sharp).transposed(named("Tercera mayor"), c),
            Some(pitch(Step::A, 4, sharp))
        );
        // Above A♯ the same third needs a double sharp.
        assert_eq!(
            pitch(Step::A, 4, sharp).transposed(named("Tercera mayor"), c),
            Some(pitch(Step::C, 5, Some(Accidental::DoubleSharp)))
        );
        // Down a minor second from C is B, and an octave keeps the letter.
        assert_eq!(
            pitch(Step::C, 4, None).transposed(named("Segunda menor").down(), c),
            Some(pitch(Step::B, 3, None))
        );
        assert_eq!(
            pitch(Step::E, 4, Some(Accidental::Flat)).transposed(Interval::OCTAVE, c),
            Some(pitch(Step::E, 5, Some(Accidental::Flat)))
        );
        // In D major, E up a whole tone is F♯: the key already says it.
        let d = KeySignature::new(2, KeyMode::Major).unwrap();
        assert_eq!(
            pitch(Step::E, 4, None).transposed(named("Segunda mayor"), d),
            Some(pitch(Step::F, 4, None))
        );
        // C♯ in D major follows the key; a fifth up, G♯ needs its sharp.
        assert_eq!(
            pitch(Step::C, 4, None).transposed(named("Quinta justa"), d),
            Some(pitch(Step::G, 4, sharp))
        );
        // Beyond a double sign or the range there is no answer.
        assert_eq!(
            pitch(Step::C, 4, Some(Accidental::DoubleSharp))
                .transposed(named("Unísono aumentado"), c),
            None
        );
        assert_eq!(
            pitch(Step::B, 8, None).transposed(Interval::OCTAVE, c),
            None
        );
    }

    #[test]
    fn respelling_swaps_sharps_and_flats_with_the_fewest_signs() {
        let c = KeySignature::default();
        let (sharp, flat) = (Some(Accidental::Sharp), Some(Accidental::Flat));
        let respell = |p: Pitch| p.respelled(c).unwrap();
        assert_eq!(respell(pitch(Step::C, 4, sharp)), pitch(Step::D, 4, flat));
        assert_eq!(respell(pitch(Step::D, 4, flat)), pitch(Step::C, 4, sharp));
        // A natural note takes the single sign, and gives it back.
        assert_eq!(respell(pitch(Step::E, 4, None)), pitch(Step::F, 4, flat));
        assert_eq!(respell(pitch(Step::F, 4, flat)), pitch(Step::E, 4, None));
        assert_eq!(
            respell(pitch(Step::C, 4, Some(Accidental::DoubleSharp))),
            pitch(Step::D, 4, None)
        );
        // B♯ crosses into the next octave as C.
        assert_eq!(respell(pitch(Step::B, 3, sharp)), pitch(Step::C, 4, None));
        // In E major D♯ follows the key: as E♭ it needs its flat.
        let e = KeySignature::new(4, KeyMode::Major).unwrap();
        assert_eq!(
            pitch(Step::D, 4, None).respelled(e),
            Some(pitch(Step::E, 4, flat))
        );
    }

    #[test]
    fn respelling_a_tied_note_renames_the_whole_tie() {
        let mut score = Score::default();
        let c_sharp = Event {
            pitches: Pitches::single(
                Pitch::new(Step::C, 4).map(|p| p.with_accidental(Some(Accidental::Sharp))),
            ),
            duration: Duration::Half,
            notation: Notation::default(),
        };
        let a = score.insert(0, 0, c_sharp).unwrap();
        let b = score.insert(0, 1, c_sharp).unwrap();
        let tie = Connection {
            start: a,
            end: b,
            kind: ConnectionKind::Tie,
        };
        score.add_connection(tie).unwrap();
        score.respell(&[b], None).unwrap();
        for id in [a, b] {
            assert_eq!(
                score.event(id).unwrap().pitches.first(),
                Some(pitch(Step::D, 4, Some(Accidental::Flat)))
            );
        }
        assert!(score.connections().contains(&tie));
    }

    #[test]
    fn a_passage_moves_with_its_grace_notes_and_chord_symbols_or_not_at_all() {
        let mut score = Score::default();
        let note = |step| Event {
            pitches: Pitches::single(Pitch::new(step, 4)),
            duration: Duration::Quarter,
            notation: Notation::default(),
        };
        let a = score.insert(0, 0, note(Step::C)).unwrap();
        let rest = score
            .insert(
                0,
                1,
                Event {
                    pitches: Pitches::default(),
                    ..note(Step::C)
                },
            )
            .unwrap();
        let b = score.insert(0, 2, note(Step::G)).unwrap();
        score
            .set_chord(a, Some(ChordSymbol::parse("Cmaj7/E").unwrap()))
            .unwrap();
        // A grace note on G, which moves with it.
        score.add_grace_note(b).unwrap();
        let third = Interval::new(2, 4);
        score.transpose_by(&[a, rest, b], third).unwrap();
        assert_eq!(
            score.event(a).unwrap().pitches.first().unwrap().step,
            Step::E
        );
        assert_eq!(
            score.event(b).unwrap().pitches.first(),
            Some(pitch(Step::B, 4, None))
        );
        let grace = score.event(b).unwrap().notation.grace.unwrap();
        assert_eq!(grace.note(0), Some(pitch(Step::B, 4, None)));
        assert_eq!(
            score.event(a).unwrap().notation.chord.unwrap().label(),
            "Emaj7/G♯"
        );
        // One note out of range leaves the whole passage where it was.
        let high = score.insert(1, 0, note(Step::C)).unwrap();
        let before = score.clone();
        assert!(
            score
                .transpose_by(&[a, high], Interval::OCTAVE.plus_octaves(4))
                .is_err()
        );
        assert_eq!(score, before);
    }
}
