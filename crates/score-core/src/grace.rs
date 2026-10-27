//! Grace notes: up to four small notes written before a note, taking no time
//! of the bar. An acciaccatura is crushed just before the beat; an
//! appoggiatura leans on it and takes half of the note it decorates.
use super::*;

pub const MAX_GRACE_NOTES: usize = 4;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum GraceKind {
    Acciaccatura,
    Appoggiatura,
}

impl GraceKind {
    pub const ALL: [Self; 2] = [Self::Acciaccatura, Self::Appoggiatura];
    pub const fn label(self) -> &'static str {
        match self {
            Self::Acciaccatura => "Acciaccatura",
            Self::Appoggiatura => "Appoggiatura",
        }
    }
}

/// The grace notes of one event, in the order they sound.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Grace {
    pub kind: GraceKind,
    notes: [Option<Pitch>; MAX_GRACE_NOTES],
}

impl Grace {
    pub fn new(kind: GraceKind, first: Pitch) -> Self {
        let mut notes = [None; MAX_GRACE_NOTES];
        notes[0] = Some(first);
        Self { kind, notes }
    }

    /// A group from one to four notes; more are not written.
    pub fn from_notes(kind: GraceKind, notes: &[Pitch]) -> Option<Self> {
        let (first, rest) = notes.split_first()?;
        let mut grace = Self::new(kind, *first);
        for pitch in rest {
            if !grace.push(*pitch) {
                return None;
            }
        }
        Some(grace)
    }

    pub fn notes(&self) -> impl Iterator<Item = Pitch> + '_ {
        self.notes.iter().map_while(|note| *note)
    }

    pub fn len(&self) -> usize {
        self.notes().count()
    }

    pub fn is_empty(&self) -> bool {
        self.notes[0].is_none()
    }

    pub fn note(&self, index: usize) -> Option<Pitch> {
        self.notes.get(index).copied().flatten()
    }

    /// Notes in order with no gaps; a group is never empty.
    fn is_valid(&self) -> bool {
        !self.is_empty()
            && self
                .notes
                .windows(2)
                .all(|pair| pair[0].is_some() || pair[1].is_none())
    }

    fn push(&mut self, pitch: Pitch) -> bool {
        match self.notes.iter_mut().find(|note| note.is_none()) {
            Some(slot) => {
                *slot = Some(pitch);
                true
            }
            None => false,
        }
    }

    /// Without the note at `index`; `None` once the group is empty.
    fn without(mut self, index: usize) -> Option<Self> {
        if index >= self.len() {
            return Some(self);
        }
        self.notes[index..].rotate_left(1);
        self.notes[MAX_GRACE_NOTES - 1] = None;
        (!self.is_empty()).then_some(self)
    }
}

impl Score {
    fn grace_slot(&mut self, id: ElementId) -> Result<&mut Notation, ScoreError> {
        let event = self
            .event(id)
            .ok_or(ScoreError("Selecciona una nota o acorde"))?;
        if event.pitches.is_empty() {
            return Err(ScoreError("Un silencio no lleva notas de adorno"));
        }
        Ok(&mut Arc::make_mut(
            &mut self.part_mut(id.part)?.staff_mut(id.staff)?.measures[id.measure].events,
        )[id.event]
            .notation)
    }

    /// Write `kind` before the note, starting from its top pitch, or change
    /// the kind of the group already there; `None` removes the group.
    pub fn set_grace(&mut self, id: ElementId, kind: Option<GraceKind>) -> Result<(), ScoreError> {
        let top = self.event(id).and_then(|event| event.pitches.iter().last());
        let notation = self.grace_slot(id)?;
        notation.grace = match (kind, notation.grace) {
            (None, _) => None,
            (Some(kind), Some(grace)) => Some(Grace { kind, ..grace }),
            (Some(kind), None) => Some(Grace::new(kind, top.expect("pitched event"))),
        };
        Ok(())
    }

    /// Add one more grace note, repeating the last one, or start an
    /// acciaccatura. Returns the index of the new note.
    pub fn add_grace_note(&mut self, id: ElementId) -> Result<usize, ScoreError> {
        let top = self.event(id).and_then(|event| event.pitches.iter().last());
        let notation = self.grace_slot(id)?;
        let Some(mut grace) = notation.grace else {
            notation.grace = Some(Grace::new(
                GraceKind::Acciaccatura,
                top.expect("pitched event"),
            ));
            return Ok(0);
        };
        let last = grace.note(grace.len() - 1).expect("nonempty group");
        if !grace.push(last) {
            return Err(ScoreError(
                "Una nota lleva como mucho cuatro notas de adorno",
            ));
        }
        // Several grace notes are written as a group, not crushed one by one.
        grace.kind = GraceKind::Appoggiatura;
        notation.grace = Some(grace);
        Ok(grace.len() - 1)
    }

    /// Move one grace note by diatonic `steps`, as notes move.
    pub fn transpose_grace_note(
        &mut self,
        id: ElementId,
        index: usize,
        steps: i16,
    ) -> Result<(), ScoreError> {
        let notation = self.grace_slot(id)?;
        let mut grace = notation
            .grace
            .ok_or(ScoreError("La nota no tiene notas de adorno"))?;
        let pitch = grace
            .note(index)
            .ok_or(ScoreError("Esa nota de adorno ya no existe"))?;
        let moved = pitch
            .shift(steps)
            .ok_or(ScoreError("La nota de adorno saldría del registro"))?;
        grace.notes[index] = Some(moved);
        notation.grace = Some(grace);
        Ok(())
    }

    /// Remove one grace note; the last one takes the group with it.
    pub fn remove_grace_note(&mut self, id: ElementId, index: usize) -> Result<(), ScoreError> {
        let notation = self.grace_slot(id)?;
        let grace = notation
            .grace
            .ok_or(ScoreError("La nota no tiene notas de adorno"))?;
        notation.grace = grace.without(index);
        Ok(())
    }

    /// The grace notes of `id` as they sound, with the key and the part's
    /// transposition applied. Unpitched percussion plays none.
    pub fn sounding_graces(&self, id: ElementId) -> Result<Vec<Pitch>, ScoreError> {
        let event = self.event(id).ok_or(ScoreError("La nota no existe"))?;
        let part = self
            .part(id.part)
            .ok_or(ScoreError("El instrumento no existe"))?;
        let Some(grace) = event.notation.grace else {
            return Ok(Vec::new());
        };
        if part.instrument().percussion_key().is_some() {
            return Ok(Vec::new());
        }
        grace
            .notes()
            .map(|pitch| {
                self.sounding_at(id, Pitches::single(Some(pitch)))?
                    .first()
                    .ok_or(ScoreError("La nota de adorno no tiene altura"))
            })
            .collect()
    }

    pub(crate) fn validate_grace(event: &Event) -> Result<(), ScoreError> {
        match event.notation.grace {
            Some(_) if event.pitches.is_empty() => {
                Err(ScoreError("Notas de adorno en un silencio"))
            }
            Some(grace) if !grace.is_valid() => Err(ScoreError("Grupo de notas de adorno vacío")),
            _ => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn note(step: Step) -> Event {
        Event {
            pitches: Pitches::single(Pitch::new(step, 5)),
            duration: Duration::Quarter,
            notation: Notation::default(),
        }
    }

    #[test]
    fn graces_start_from_the_top_note_grow_to_four_and_shrink_back() {
        let mut score = Score::default();
        let id = score.insert(0, 0, note(Step::D)).unwrap();
        score.set_grace(id, Some(GraceKind::Acciaccatura)).unwrap();
        let grace = score.event(id).unwrap().notation.grace.unwrap();
        assert_eq!(grace.kind, GraceKind::Acciaccatura);
        assert_eq!(
            grace.notes().collect::<Vec<_>>(),
            [Pitch::new(Step::D, 5).unwrap()]
        );

        score.transpose_grace_note(id, 0, 1).unwrap();
        for index in 1..MAX_GRACE_NOTES {
            assert_eq!(score.add_grace_note(id).unwrap(), index);
        }
        assert!(score.add_grace_note(id).is_err());
        let grace = score.event(id).unwrap().notation.grace.unwrap();
        assert_eq!(grace.len(), 4);
        assert_eq!(grace.kind, GraceKind::Appoggiatura);
        assert!(
            grace
                .notes()
                .all(|pitch| pitch == Pitch::new(Step::E, 5).unwrap())
        );
        score.validate().unwrap();

        score.transpose_grace_note(id, 3, -2).unwrap();
        score.remove_grace_note(id, 0).unwrap();
        let grace = score.event(id).unwrap().notation.grace.unwrap();
        assert_eq!(grace.len(), 3);
        assert_eq!(grace.note(2), Pitch::new(Step::C, 5));
        for _ in 0..3 {
            score.remove_grace_note(id, 0).unwrap();
        }
        assert_eq!(score.event(id).unwrap().notation.grace, None);
        score.validate().unwrap();
    }

    #[test]
    fn transposing_a_note_moves_its_grace_notes_with_it() {
        let mut score = Score::default();
        let id = score.insert(0, 0, note(Step::D)).unwrap();
        score.add_grace_note(id).unwrap();
        score.transpose_grace_note(id, 0, -1).unwrap();
        score.transpose(id, 2).unwrap();
        let grace = score.event(id).unwrap().notation.grace.unwrap();
        assert_eq!(grace.note(0), Pitch::new(Step::E, 5));
        assert!(score.transpose(id, 60).is_err());
    }

    #[test]
    fn rests_take_no_graces_and_a_kind_change_keeps_the_notes() {
        let mut score = Score::default();
        let rest = score
            .insert(
                0,
                0,
                Event {
                    pitches: Pitches::default(),
                    ..note(Step::C)
                },
            )
            .unwrap();
        assert!(
            score
                .set_grace(rest, Some(GraceKind::Acciaccatura))
                .is_err()
        );
        let id = score.insert(0, 1, note(Step::G)).unwrap();
        score.add_grace_note(id).unwrap();
        score.add_grace_note(id).unwrap();
        score.set_grace(id, Some(GraceKind::Acciaccatura)).unwrap();
        let grace = score.event(id).unwrap().notation.grace.unwrap();
        assert_eq!((grace.kind, grace.len()), (GraceKind::Acciaccatura, 2));
        score.set_grace(id, None).unwrap();
        assert_eq!(score.event(id).unwrap().notation.grace, None);
    }
}
