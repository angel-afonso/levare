//! Stored events are written pitches; the conductor key is concert pitch.
use super::*;

/// Written-to-sounding interval. Missing legacy metadata means concert notation.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Transposition {
    pub diatonic: i8,
    pub chromatic: i8,
}
impl Transposition {
    pub const fn new(diatonic: i8, chromatic: i8) -> Self {
        Self {
            diatonic,
            chromatic,
        }
    }
    pub fn is_concert(self) -> bool {
        self == Self::default()
    }
    pub fn validate(self) -> Result<(), ScoreError> {
        if !(-28..=28).contains(&self.diatonic) || !(-48..=48).contains(&self.chromatic) {
            return Err(ScoreError("Transposición fuera de rango"));
        }
        Ok(())
    }
    pub fn written_key(self, concert: KeySignature) -> KeySignature {
        let mut fifths = i16::from(concert.fifths())
            - (7 * i16::from(self.chromatic) - 12 * i16::from(self.diatonic));
        while fifths > 7 {
            fifths -= 12;
        }
        while fifths < -7 {
            fifths += 12;
        }
        KeySignature::new(fifths as i8, concert.mode()).unwrap()
    }
    pub fn inverse(self) -> Self {
        Self::new(
            self.diatonic.saturating_neg(),
            self.chromatic.saturating_neg(),
        )
    }
    pub fn pitches(
        self,
        pitches: Pitches,
        source_key: KeySignature,
    ) -> Result<Pitches, ScoreError> {
        self.validate()?;
        if self.is_concert() {
            return Ok(pitches.in_key(source_key));
        }
        let mut out = Vec::new();
        for pitch in pitches.in_key(source_key).iter() {
            let shifted = pitch.shift(i16::from(self.diatonic)).ok_or(ScoreError(
                "La transposición deja la nota fuera del registro admitido",
            ))?;
            let natural = |p: Pitch| {
                i16::from(p.octave()) * 12 + [0, 2, 4, 5, 7, 9, 11][p.step.index() as usize]
            };
            let target = natural(pitch) + i16::from(pitch.alteration()) + i16::from(self.chromatic);
            let alter = target - natural(shifted);
            let value = if (-1..=1).contains(&alter) {
                shifted.with_accidental(Some(match alter {
                    -1 => Accidental::Flat,
                    1 => Accidental::Sharp,
                    _ => Accidental::Natural,
                }))
            } else {
                // Enharmonic spelling when an extreme key would require double signs.
                (0..63)
                    .filter_map(|d| {
                        Pitch::new(
                            [
                                Step::C,
                                Step::D,
                                Step::E,
                                Step::F,
                                Step::G,
                                Step::A,
                                Step::B,
                            ][d % 7],
                            (d / 7) as u8,
                        )
                    })
                    .find_map(|p| {
                        let a = target - natural(p);
                        (-1..=1).contains(&a).then(|| {
                            p.with_accidental(Some(match a {
                                -1 => Accidental::Flat,
                                1 => Accidental::Sharp,
                                _ => Accidental::Natural,
                            }))
                        })
                    })
                    .ok_or(ScoreError("Nota transpuesta fuera de registro"))?
            };
            out.push(value);
        }
        Pitches::new(&out)
    }
}
impl Part {
    pub fn transposition(&self) -> Transposition {
        self.transposition
    }
}
impl Score {
    pub(super) fn validate_event_pitches(
        &self,
        part: PartId,
        measure: usize,
        event: Event,
    ) -> Result<(), ScoreError> {
        event.pitches.validate()?;
        if event.pitches.is_empty() && event.notation.tremolo.is_some() {
            return Err(ScoreError("Un silencio no admite trémolo"));
        }
        let source = self
            .part(part)
            .ok_or(ScoreError("El instrumento no existe"))?;
        event.validate_harmonic(source.instrument())?;
        if source.instrument().percussion_key().is_some() {
            if event.pitches.len() > 1 {
                return Err(ScoreError(
                    "La percusión sin altura admite un golpe por voz",
                ));
            }
        } else {
            source.transposition().pitches(
                event.sounding_pitches(self.written_key_at(part, measure)),
                KeySignature::default(),
            )?;
        }
        Ok(())
    }
    /// Set the written-to-sounding interval. Existing written notes stay in place.
    pub fn set_part_transposition(
        &mut self,
        part: PartId,
        interval: Transposition,
    ) -> Result<(), ScoreError> {
        interval.validate()?;
        let mut next = self.clone();
        next.parts
            .iter_mut()
            .find(|p| p.id == part)
            .ok_or(ScoreError("El instrumento no existe"))?
            .transposition = interval;
        next.validate()?;
        *self = next;
        Ok(())
    }
    pub fn written_key_at(&self, part: PartId, measure: usize) -> KeySignature {
        let Some(part) = self.part(part) else {
            return self.key_signature_at(measure);
        };
        if part.instrument().percussion_key().is_some() {
            KeySignature::default()
        } else {
            part.transposition()
                .written_key(self.key_signature_at(measure))
        }
    }
    pub fn sounding_pitches(&self, id: ElementId) -> Result<Pitches, ScoreError> {
        let event = self.event(id).ok_or(ScoreError("La nota no existe"))?;
        let part = self
            .part(id.part)
            .ok_or(ScoreError("El instrumento no existe"))?;
        if let Some(key) = part.instrument().percussion_key() {
            if event.pitches.is_empty() {
                return Ok(Pitches::default());
            }
            let (step, alteration) = [
                (Step::C, 0),
                (Step::C, 1),
                (Step::D, 0),
                (Step::E, -1),
                (Step::E, 0),
                (Step::F, 0),
                (Step::F, 1),
                (Step::G, 0),
                (Step::A, -1),
                (Step::A, 0),
                (Step::B, -1),
                (Step::B, 0),
            ][usize::from(key % 12)];
            return Ok(Pitches::single(Pitch::new(step, key / 12 - 1).map(|p| {
                p.with_accidental(Some(match alteration {
                    -1 => Accidental::Flat,
                    1 => Accidental::Sharp,
                    _ => Accidental::Natural,
                }))
            })));
        }
        let sounding = part.transposition.pitches(
            event.sounding_pitches(self.written_key_at(id.part, id.measure)),
            KeySignature::default(),
        )?;
        self.with_ottava(id, sounding)
    }

    fn with_ottava(&self, id: ElementId, pitches: Pitches) -> Result<Pitches, ScoreError> {
        match self.ottava_steps(id) {
            0 => Ok(pitches),
            _ if pitches.is_empty() => Ok(pitches),
            steps => pitches.transposed(steps),
        }
    }

    /// `pitches` written at `id`'s place, as they sound: key, the part's
    /// transposition and any octave line applied.
    pub(crate) fn sounding_at(
        &self,
        id: ElementId,
        pitches: Pitches,
    ) -> Result<Pitches, ScoreError> {
        let part = self
            .part(id.part)
            .ok_or(ScoreError("El instrumento no existe"))?;
        let key = self.written_key_at(id.part, id.measure);
        let sounding = part
            .transposition
            .pitches(pitches.in_key(key), KeySignature::default())?;
        self.with_ottava(id, sounding)
    }

    /// Diatonic steps an 8va (+7) or 8vb (−7) line moves the note at `id`.
    pub fn ottava_steps(&self, id: ElementId) -> i16 {
        let Some(tick) = self.event_tick(id) else {
            return 0;
        };
        self.connections()
            .iter()
            .find(|c| {
                c.kind.is_ottava()
                    && c.start.part == id.part
                    && c.start.staff == id.staff
                    && self.event_tick(c.start).is_some_and(|start| start <= tick)
                    && self.event_tick(c.end).is_some_and(|end| tick <= end)
            })
            .map_or(0, |c| {
                if c.kind == ConnectionKind::OttavaAlta {
                    7
                } else {
                    -7
                }
            })
    }

    /// The note above for a trill on the single note at `id`, as it sounds.
    pub fn trill_upper(&self, id: ElementId) -> Option<Pitch> {
        let event = self.event(id)?;
        let [pitch] = event.pitches.iter().collect::<Vec<_>>()[..] else {
            return None;
        };
        let upper = pitch.with_accidental(None).shift(1)?;
        self.sounding_at(id, Pitches::single(Some(upper)))
            .ok()?
            .first()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn midi(p: Pitch) -> i16 {
        (i16::from(p.octave()) + 1) * 12
            + [0, 2, 4, 5, 7, 9, 11][p.step.index() as usize]
            + i16::from(p.alteration())
    }
    #[test]
    fn written_keys_and_sounding_notes_follow_each_orchestral_profile() {
        for (instrument, semitones, fifths) in [
            (Instrument::ClarinetBb, -2, 2),
            (Instrument::ClarinetA, -3, -3),
            (Instrument::ClarinetEb, 3, 3),
            (Instrument::HornF, -7, 1),
            (Instrument::EnglishHorn, -7, 1),
            (Instrument::AltoSax, -9, 3),
            (Instrument::TenorSax, -14, 2),
            (Instrument::BaritoneSax, -21, 3),
            (Instrument::Piccolo, 12, 0),
            (Instrument::Glockenspiel, 24, 0),
            (Instrument::DoubleBass, -12, 0),
            (Instrument::Clarinet, 0, 0),
        ] {
            let mut score = Score::new("Orquesta".into(), &[instrument], 2).unwrap();
            let part = score.parts()[0].id();
            assert_eq!(
                score.written_key_at(part, 0).fifths(),
                fifths,
                "{}",
                instrument.name()
            );
            let event = Event {
                pitches: Pitches::single(
                    Pitch::new(Step::C, 4).map(|p| p.with_accidental(Some(Accidental::Natural))),
                ),
                duration: Duration::Quarter,
                notation: Notation::default(),
            };
            let id = score.insert_on(part, StaffId::Upper, 0, 0, event).unwrap();
            assert_eq!(
                midi(score.sounding_pitches(id).unwrap().first().unwrap()),
                60 + semitones,
                "{}",
                instrument.name()
            );
            score
                .set_key_signature_at(1, KeySignature::new(-2, KeyMode::Minor).unwrap())
                .unwrap();
            assert_eq!(score.written_key_at(part, 1).mode(), KeyMode::Minor);
            assert_eq!(score.written_key_at(part, 1).fifths(), fifths - 2);
            score.validate().unwrap();
        }
    }
    #[test]
    fn catalog_has_valid_profiles_and_appropriate_staff_structure() {
        assert!(Instrument::ALL.len() >= 45);
        for instrument in Instrument::ALL {
            let score = Score::new("Catálogo".into(), &[instrument], 1).unwrap();
            let part = &score.parts()[0];
            part.transposition().validate().unwrap();
            assert_eq!(
                part.staves().len(),
                if instrument.is_grand_staff() { 2 } else { 1 }
            );
            if instrument.is_grand_staff() {
                assert_eq!(part.clef(StaffId::Lower), Clef::Bass);
            }
            if instrument.percussion_key().is_some() {
                assert_eq!(part.clef(StaffId::Upper), Clef::Percussion);
            }
            assert!(instrument.program() < 128);
        }
    }
    #[test]
    fn extreme_keys_keep_pitch_and_octave_and_bad_registers_fail_atomically() {
        for instrument in Instrument::ALL {
            let t = instrument.standard_transposition();
            for fifths in -7..=7 {
                let key = KeySignature::new(fifths, KeyMode::Major).unwrap();
                for step in [
                    Step::C,
                    Step::D,
                    Step::E,
                    Step::F,
                    Step::G,
                    Step::A,
                    Step::B,
                ] {
                    let pitches = Pitches::single(Pitch::new(step, 4));
                    let sounding = t.pitches(pitches, key).unwrap();
                    let back = t
                        .inverse()
                        .pitches(sounding, KeySignature::default())
                        .unwrap();
                    assert_eq!(
                        midi(pitches.in_key(key).first().unwrap()),
                        midi(back.first().unwrap())
                    );
                }
            }
        }
        let mut score = Score::new("Extremos".into(), &[Instrument::Glockenspiel], 1).unwrap();
        let before = score.clone();
        assert!(
            score
                .insert_on(
                    score.parts()[0].id(),
                    StaffId::Upper,
                    0,
                    0,
                    Event {
                        pitches: Pitches::single(Pitch::new(Step::C, 8)),
                        duration: Duration::Quarter,
                        notation: Notation::default()
                    }
                )
                .is_err()
        );
        assert_eq!(score, before);
    }
    #[test]
    fn direct_pitch_and_harmonic_edits_reject_invalid_sounds_without_mutating() {
        let mut score = Score::new("Límites".into(), &[Instrument::Glockenspiel], 1).unwrap();
        let part = score.parts()[0].id();
        let id = score
            .insert_on(
                part,
                StaffId::Upper,
                0,
                0,
                Event {
                    pitches: Pitches::single(Pitch::new(Step::C, 6)),
                    duration: Duration::Quarter,
                    notation: Notation::default(),
                },
            )
            .unwrap();
        let before = score.clone();
        assert!(score.transpose(id, 7).is_err());
        assert_eq!(score, before);
        let mut drums = Score::new("Percusión".into(), &[Instrument::SnareDrum], 1).unwrap();
        let id = drums
            .insert_on(
                drums.parts()[0].id(),
                StaffId::Upper,
                0,
                0,
                Event {
                    pitches: Pitches::single(Pitch::new(Step::B, 4)),
                    duration: Duration::Quarter,
                    notation: Notation::default(),
                },
            )
            .unwrap();
        let before = drums.clone();
        assert!(
            drums
                .add_pitch(id, Pitch::new(Step::C, 5).unwrap())
                .is_err()
        );
        assert_eq!(drums, before);
        let mut bass = Score::new("Armónico".into(), &[Instrument::DoubleBass], 1).unwrap();
        let id = bass
            .insert_on(
                bass.parts()[0].id(),
                StaffId::Upper,
                0,
                0,
                Event {
                    pitches: Pitches::single(Pitch::new(Step::C, 0)),
                    duration: Duration::Quarter,
                    notation: Notation {
                        harmonic: Some(Harmonic::Artificial),
                        ..Default::default()
                    },
                },
            )
            .unwrap();
        let before = bass.clone();
        assert!(bass.set_harmonic(id, Some(Harmonic::Natural)).is_err());
        assert_eq!(bass, before);
    }
}
