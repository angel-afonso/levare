//! Single-note tremolo, written as strokes through a stem.
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Tremolo {
    One,
    Two,
    Three,
}
impl Tremolo {
    pub const ALL: [Self; 3] = [Self::One, Self::Two, Self::Three];
    pub const fn strokes(self) -> u8 {
        match self {
            Self::One => 1,
            Self::Two => 2,
            Self::Three => 3,
        }
    }
    pub const fn from_strokes(strokes: u8) -> Option<Self> {
        match strokes {
            1 => Some(Self::One),
            2 => Some(Self::Two),
            3 => Some(Self::Three),
            _ => None,
        }
    }
    pub const fn label(self) -> &'static str {
        match self {
            Self::One => "Trémolo de una barra",
            Self::Two => "Trémolo de dos barras",
            Self::Three => "Trémolo de tres barras",
        }
    }
    pub const fn glyph(self) -> &'static str {
        match self {
            Self::One => "\u{e220}",
            Self::Two => "\u{e221}",
            Self::Three => "\u{e222}",
        }
    }
    pub const fn repetition_ticks(self) -> u16 {
        match self {
            Self::One => 24,
            Self::Two => 12,
            Self::Three => 6,
        }
    }
}
impl Score {
    pub fn set_tremolo(&mut self, id: ElementId, value: Option<Tremolo>) -> Result<(), ScoreError> {
        let event = self
            .event(id)
            .ok_or(ScoreError("Selecciona una nota o acorde"))?;
        if event.pitches.is_empty() {
            return Err(ScoreError("Un silencio no admite trémolo"));
        }
        Arc::make_mut(
            &mut self.part_mut(id.part)?.staff_mut(id.staff)?.measures[id.measure].events,
        )[id.event]
            .notation
            .tremolo = value;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn tremolo_replaces_removes_and_erases_without_leaving_invalid_rests() {
        let mut score = Score::default();
        let id = score
            .insert(
                0,
                0,
                Event {
                    pitches: Pitches::single(Pitch::new(Step::C, 4)),
                    duration: Duration::Quarter,
                    notation: Default::default(),
                },
            )
            .unwrap();
        for mark in Tremolo::ALL {
            assert_eq!(Tremolo::from_strokes(mark.strokes()), Some(mark));
            score
                .set_point_mark(id, MarkFamily::Tremolo, Some(MarkKind::Tremolo(mark)))
                .unwrap();
            assert_eq!(
                score.mark_kind(MarkRef::Tremolo(id)),
                Some(MarkKind::Tremolo(mark))
            );
            assert_eq!(MarkKind::Tremolo(mark).placement(), Placement::Stem);
            score.validate().unwrap();
        }
        score.remove_mark(MarkRef::Tremolo(id)).unwrap();
        assert!(score.event(id).unwrap().notation.tremolo.is_none());
        score.set_tremolo(id, Some(Tremolo::Two)).unwrap();
        assert!(score.set_pitches(id, Pitches::default()).is_err());
        score.erase(id).unwrap();
        assert!(score.event(id).unwrap().notation.tremolo.is_none());
        assert!(score.set_tremolo(id, Some(Tremolo::One)).is_err());
        score.validate().unwrap();
        assert_eq!(Tremolo::from_strokes(0), None);
        assert_eq!(Tremolo::from_strokes(4), None);
    }
}
