//! Per-event string harmonics. The artificial touch head is derived notation,
//! never a second sounding note or an editable chord tone.
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Harmonic {
    /// Written at sounding pitch, with the SMuFL harmonic circle.
    Natural,
    /// A stopped note and a diamond a perfect fourth above; sounds two octaves up.
    Artificial,
}
impl Harmonic {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Natural => "Armónico natural",
            Self::Artificial => "Armónico artificial",
        }
    }
    pub const fn glyph(self) -> &'static str {
        match self {
            Self::Natural => "\u{e614}",
            Self::Artificial => "\u{e0dd}",
        }
    }
}
impl Event {
    pub fn validate_harmonic(self, instrument: Instrument) -> Result<(), ScoreError> {
        let Some(mark) = self.notation.harmonic else {
            return Ok(());
        };
        if instrument.family() != InstrumentFamily::Strings {
            return Err(ScoreError(
                "Los armónicos requieren violín, viola, violonchelo o contrabajo",
            ));
        }
        if self.pitches.is_empty() {
            return Err(ScoreError("Un silencio no admite armónicos"));
        }
        if mark == Harmonic::Artificial
            && (self.pitches.len() != 1 || self.pitches.first().unwrap().octave() > 6)
        {
            return Err(ScoreError(
                "El armónico artificial requiere una sola nota pisada entre C0 y B6",
            ));
        }
        Ok(())
    }
    /// Resolve the key before changing octaves. Natural harmonics are already
    /// written at sounding pitch; artificial ones sound only the fourth partial.
    pub fn sounding_pitches(&self, key: KeySignature) -> Pitches {
        let pitches = self.pitches.in_key(key);
        if self.notation.harmonic == Some(Harmonic::Artificial) {
            pitches
                .transposed(14)
                .expect("validated artificial harmonic")
        } else {
            pitches
        }
    }
    /// Diatonic location and absolute alteration of the generated touch head.
    /// Alterations can include double flats (e.g. a stopped F-flat).
    pub fn harmonic_touch(&self, key: KeySignature) -> Option<(Pitch, i8)> {
        if self.notation.harmonic != Some(Harmonic::Artificial) {
            return None;
        }
        let base = self.pitches.first()?.in_key(key);
        let touch = base.shift(3)?.with_accidental(None);
        let semitone =
            |p: Pitch| i16::from(p.octave()) * 12 + [0, 2, 4, 5, 7, 9, 11][p.step.index() as usize];
        let alteration = semitone(base) + i16::from(base.alteration()) + 5 - semitone(touch);
        Some((touch, alteration as i8))
    }
}
impl Score {
    pub fn set_harmonic(
        &mut self,
        id: ElementId,
        value: Option<Harmonic>,
    ) -> Result<(), ScoreError> {
        self.set_harmonics(&[(id, value)])
    }
    /// Change a passage atomically, then validate its ties against the final pitches.
    pub fn set_harmonics(
        &mut self,
        edits: &[(ElementId, Option<Harmonic>)],
    ) -> Result<(), ScoreError> {
        for &(id, value) in edits {
            let mut event = *self.event(id).ok_or(ScoreError("Selecciona una nota"))?;
            event.notation.harmonic = value;
            self.validate_event_pitches(id.part, id.measure, event)?;
        }
        for &(id, value) in edits {
            let notation = &mut Arc::make_mut(
                &mut self.part_mut(id.part)?.staff_mut(id.staff)?.measures[id.measure].events,
            )[id.event]
                .notation;
            notation.harmonic = value;
            if value.is_none() {
                notation.offsets.harmonic = None;
            }
        }
        self.prune_connections();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn note(step: Step, octave: u8) -> Event {
        Event {
            pitches: Pitches::single(Pitch::new(step, octave)),
            duration: Duration::Quarter,
            notation: Notation::default(),
        }
    }
    #[test]
    fn artificial_touch_and_sound_resolve_keys_and_enharmonic_spellings() {
        let mut event = note(Step::F, 4);
        event.notation.harmonic = Some(Harmonic::Artificial);
        let sharp_key = KeySignature::new(1, KeyMode::Major).unwrap();
        let (touch, sign) = event.harmonic_touch(sharp_key).unwrap();
        assert_eq!((touch.step, touch.octave(), sign), (Step::B, 4, 0));
        let sound = event.sounding_pitches(sharp_key).first().unwrap();
        assert_eq!(
            (sound.step, sound.octave(), sound.alteration()),
            (Step::F, 6, 1)
        );
        event.pitches = Pitches::single(Some(
            Pitch::new(Step::F, 4)
                .unwrap()
                .with_accidental(Some(Accidental::Flat)),
        ));
        assert_eq!(event.harmonic_touch(sharp_key).unwrap().1, -2);
        assert_eq!(
            event
                .sounding_pitches(sharp_key)
                .first()
                .unwrap()
                .alteration(),
            -1
        );
        event.notation.harmonic = Some(Harmonic::Natural);
        assert_eq!(
            event.sounding_pitches(sharp_key),
            event.pitches.in_key(sharp_key)
        );
    }
    #[test]
    fn harmonic_validation_and_edits_are_atomic_and_erase_clears_the_mark() {
        let mut score = Score::default();
        let piano = score.parts()[0].id();
        let strings = score.add_part(Instrument::Violin).unwrap();
        let a = score
            .insert_on(strings, StaffId::Upper, 0, 0, note(Step::G, 4))
            .unwrap();
        let rest = score
            .insert_on(
                strings,
                StaffId::Upper,
                0,
                1,
                Event {
                    pitches: Pitches::default(),
                    ..note(Step::G, 4)
                },
            )
            .unwrap();
        let p = score
            .insert_on(piano, StaffId::Upper, 0, 0, note(Step::G, 4))
            .unwrap();
        assert!(score.set_harmonic(rest, Some(Harmonic::Natural)).is_err());
        assert!(score.set_harmonic(p, Some(Harmonic::Natural)).is_err());
        score.set_harmonic(a, Some(Harmonic::Artificial)).unwrap();
        let before = score.clone();
        assert!(score.add_pitch(a, Pitch::new(Step::B, 4).unwrap()).is_err());
        assert!(score.transpose(a, 21).is_err());
        assert_eq!(score, before);
        score.transpose(a, 1).unwrap();
        assert_eq!(
            score
                .event(a)
                .unwrap()
                .harmonic_touch(score.key_signature())
                .unwrap()
                .0
                .step,
            Step::D
        );
        score.erase(a).unwrap();
        assert!(score.event(a).unwrap().notation.harmonic.is_none());
        score.validate().unwrap();
    }
    #[test]
    fn ties_require_matching_harmonic_kind_and_survive_matching_pitch_changes() {
        let mut score = Score::default();
        let part = score.add_part(Instrument::Cello).unwrap();
        let a = score
            .insert_on(part, StaffId::Upper, 0, 0, note(Step::C, 3))
            .unwrap();
        let b = score
            .insert_on(part, StaffId::Upper, 0, 1, note(Step::C, 3))
            .unwrap();
        score.set_harmonic(a, Some(Harmonic::Artificial)).unwrap();
        let tie = Connection {
            start: a,
            end: b,
            kind: ConnectionKind::Tie,
        };
        assert!(score.add_connection(tie).is_err());
        score.set_harmonic(b, Some(Harmonic::Artificial)).unwrap();
        score.add_connection(tie).unwrap();
        score.set_harmonic(a, Some(Harmonic::Natural)).unwrap();
        assert!(score.connections().is_empty());
        score.validate().unwrap();
    }
}
