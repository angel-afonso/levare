//! A shared piano roll joins stable simultaneous events in the two hands.
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PianoArpeggio {
    pub upper: ElementId,
    pub lower: ElementId,
    pub kind: Arpeggio,
}
impl PianoArpeggio {
    pub fn contains(self, id: ElementId) -> bool {
        self.upper == id || self.lower == id
    }
}
/// The effective mark, whether local or shared by two staves.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ArpeggioMark {
    pub anchor: ElementId,
    pub other_hand: Option<ElementId>,
    pub kind: Arpeggio,
}
impl Score {
    pub fn piano_arpeggios(&self) -> &Vector<PianoArpeggio> {
        &self.piano_arpeggios
    }
    pub fn arpeggio_at(&self, id: ElementId) -> Option<ArpeggioMark> {
        if let Some(mark) = self.piano_arpeggios.iter().find(|mark| mark.contains(id)) {
            return Some(ArpeggioMark {
                anchor: mark.upper,
                other_hand: Some(mark.lower),
                kind: mark.kind,
            });
        }
        Some(ArpeggioMark {
            anchor: id,
            other_hand: None,
            kind: self.event(id)?.notation.arpeggio?,
        })
    }
    /// The other hand must attack at this onset, rather than merely sustain across it.
    pub fn piano_partner(&self, id: ElementId) -> Option<ElementId> {
        let part = self.part(id.part)?;
        if !part.instrument().is_grand_staff() || self.event(id)?.pitches.is_empty() {
            return None;
        }
        let other = if id.staff == StaffId::Upper {
            StaffId::Lower
        } else {
            StaffId::Upper
        };
        let onset = self.event_tick(id)?;
        let voice = self.voice_of(id)?;
        let bar = part.staff(other)?.measures().get(id.measure)?;
        for (_, tick, event, value) in bar.timed_iter().filter(|(v, _, _, _)| *v == voice) {
            if self.measure_start_tick(id.measure) + u64::from(tick) == onset
                && !value.pitches.is_empty()
            {
                return Some(ElementId {
                    staff: other,
                    event,
                    ..id
                });
            }
        }
        None
    }
    pub fn set_arpeggio_both_hands(
        &mut self,
        id: ElementId,
        enabled: bool,
    ) -> Result<(), ScoreError> {
        if !enabled {
            if let Some(index) = self
                .piano_arpeggios
                .iter()
                .position(|mark| mark.contains(id))
            {
                let kind = self.piano_arpeggios.remove(index).kind;
                if self.event(id).is_some_and(|event| event.pitches.len() >= 2) {
                    self.set_arpeggio(id, Some(kind))?;
                }
            }
            return Ok(());
        }
        if self.piano_arpeggios.iter().any(|mark| mark.contains(id)) {
            return Ok(());
        }
        let partner = self.piano_partner(id).ok_or(ScoreError(
            "Ambos pentagramas deben tener notas que empiecen en el mismo momento",
        ))?;
        if self.piano_arpeggios.len() >= 4096 {
            return Err(ScoreError("Máximo de 4096 arpegios entre manos"));
        }
        let kind = self
            .arpeggio_at(id)
            .or_else(|| self.arpeggio_at(partner))
            .map_or(Arpeggio::Normal, |mark| mark.kind);
        let (upper, lower) = if id.staff == StaffId::Upper {
            (id, partner)
        } else {
            (partner, id)
        };
        self.set_arpeggio(upper, None)?;
        self.set_arpeggio(lower, None)?;
        self.piano_arpeggios
            .push_back(PianoArpeggio { upper, lower, kind });
        Ok(())
    }
    fn valid_piano_arpeggio(&self, mark: PianoArpeggio) -> bool {
        mark.upper.staff == StaffId::Upper
            && mark.lower.staff == StaffId::Lower
            && mark.upper.part == mark.lower.part
            && mark.upper.measure == mark.lower.measure
            && self.piano_partner(mark.upper) == Some(mark.lower)
            && [mark.upper, mark.lower].into_iter().all(|id| {
                self.event(id)
                    .is_some_and(|event| event.notation.arpeggio.is_none())
            })
    }
    pub(super) fn prune_piano_arpeggios(&mut self) {
        self.piano_arpeggios = self
            .piano_arpeggios
            .iter()
            .copied()
            .filter(|mark| self.valid_piano_arpeggio(*mark))
            .collect();
    }
    pub(super) fn validate_piano_arpeggios(&self) -> Result<(), ScoreError> {
        let mut seen = std::collections::HashSet::new();
        if self.piano_arpeggios.len() > 4096
            || self.piano_arpeggios.iter().any(|mark| {
                !self.valid_piano_arpeggio(*mark)
                    || !seen.insert(mark.upper)
                    || !seen.insert(mark.lower)
            })
        {
            return Err(ScoreError("Arpegio entre manos inválido o duplicado"));
        }
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
    fn either_hand_can_join_change_or_remove_one_shared_mark_with_single_notes() {
        let mut score = Score::default();
        let piano = score.parts()[0].id();
        let upper = score.insert(0, 0, note(Step::C, 4)).unwrap();
        let lower = score
            .insert_on(piano, StaffId::Lower, 0, 0, note(Step::C, 3))
            .unwrap();
        score.set_arpeggio_both_hands(lower, true).unwrap();
        assert_eq!(score.arpeggio_at(upper), score.arpeggio_at(lower));
        assert_eq!(score.arpeggio_at(lower).unwrap().anchor, upper);
        score.set_arpeggio(lower, Some(Arpeggio::Down)).unwrap();
        assert_eq!(score.arpeggio_at(upper).unwrap().kind, Arpeggio::Down);
        score
            .add_pitch(lower, Pitch::new(Step::G, 3).unwrap())
            .unwrap();
        score.remove_highest_pitch(lower).unwrap();
        assert_eq!(score.piano_arpeggios().len(), 1);
        score.validate().unwrap();
        score.remove_mark(MarkRef::Arpeggio(lower)).unwrap();
        assert!(score.piano_arpeggios().is_empty());
        assert_eq!(score.event(upper).unwrap().pitches.len(), 1);
        assert_eq!(score.event(lower).unwrap().pitches.len(), 1);
    }
    #[test]
    fn shared_marks_require_matching_piano_onsets_and_prune_when_a_member_moves_or_disappears() {
        let mut score = Score::default();
        let piano = score.parts()[0].id();
        let upper = score.insert(0, 0, note(Step::C, 4)).unwrap();
        assert!(score.set_arpeggio_both_hands(upper, true).is_err());
        let lower = score
            .insert_on(piano, StaffId::Lower, 0, 0, note(Step::C, 3))
            .unwrap();
        score.set_arpeggio_both_hands(upper, true).unwrap();
        let snapshot = score.clone();
        score
            .insert_on(piano, StaffId::Lower, 0, 0, note(Step::D, 3))
            .unwrap();
        assert!(score.piano_arpeggios().is_empty());
        let mut score = snapshot.clone();
        score.remove(lower).unwrap();
        assert!(score.piano_arpeggios().is_empty());
        let mut score = snapshot;
        score
            .add_pitch(upper, Pitch::new(Step::G, 4).unwrap())
            .unwrap();
        score.set_arpeggio_both_hands(upper, false).unwrap();
        assert_eq!(
            score.event(upper).unwrap().notation.arpeggio,
            Some(Arpeggio::Normal)
        );
        assert!(score.arpeggio_at(lower).is_none());
        score.validate().unwrap();
        let flute = score.add_part(Instrument::Flute).unwrap();
        let f = score.insert_in(flute, 0, 0, note(Step::C, 4)).unwrap();
        assert!(score.set_arpeggio_both_hands(f, true).is_err());
    }
}
