//! Transactional editing and snapshot history. No dependency on the UI.
use score_core::{
    Accidental, Duration, ElementId, Event, Instrument, PartId, Pitch, Score, ScoreError, StaffId,
};

pub mod clipboard;
mod selection;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Cursor {
    pub staff: StaffId,
    pub part: PartId,
    pub measure: usize,
    pub index: usize,
}

#[derive(Clone, Copy, Debug)]
pub enum Command {
    Insert(Event),
    SetArticulation(Option<score_core::Articulation>),
    SetDynamic(Option<score_core::Dynamic>),
    SetDotted(bool),
    SetTriplet(bool),
    SetTechnique(Option<score_core::PlayingTechnique>),
    AddConnection(score_core::Connection),
    RemoveConnections(score_core::ConnectionKind),
    RemoveConnection(score_core::Connection),
    Delete,
    Transpose(i16),
    AddPitch(Pitch),
    RemoveHighestPitch,
    SetAccidental(Option<Accidental>),
    AppendMeasure,
    AddPart(Instrument),
    RemovePart,
    SetKeySignature(score_core::KeySignature),
}

#[derive(Clone)]
struct Snapshot {
    score: Score,
    cursor: Cursor,
    selection: Option<ElementId>,
    range_anchor: Option<ElementId>,
    tone: usize,
}

pub struct Editor {
    state: Snapshot,
    undo: Vec<Snapshot>,
    redo: Vec<Snapshot>,
    pub duration: Duration,
    pub octave: u8,
}

impl Default for Editor {
    fn default() -> Self {
        Self::new(Score::default()).expect("valid default score")
    }
}

impl Editor {
    pub fn new(mut score: Score) -> Result<Self, ScoreError> {
        score.validate()?;
        score.arrange_parts();
        let first_part = score.parts()[0].id();
        Ok(Self {
            state: Snapshot {
                score,
                cursor: Cursor {
                    staff: score_core::StaffId::Upper,
                    part: first_part,
                    ..Default::default()
                },
                selection: None,
                range_anchor: None,
                tone: 0,
            },
            undo: Vec::new(),
            redo: Vec::new(),
            duration: Duration::Quarter,
            octave: 4,
        })
    }
    pub fn score(&self) -> &Score {
        &self.state.score
    }
    pub fn cursor(&self) -> Cursor {
        self.state.cursor
    }
    pub fn selection(&self) -> Option<ElementId> {
        self.state.selection
    }
    pub fn selected_pitch(&self) -> Option<Pitch> {
        self.selection()
            .and_then(|id| self.score().event(id))?
            .pitches
            .iter()
            .nth(self.state.tone)
    }
    pub fn select_tone(&mut self, index: usize) {
        if self
            .selection()
            .and_then(|id| self.score().event(id))
            .is_some_and(|event| index < event.pitches.len())
        {
            self.state.tone = index;
            self.octave = self.selected_pitch().unwrap().octave();
        }
    }
    pub fn select_pitch(&mut self, id: ElementId, pitch: Option<Pitch>) {
        self.select(id);
        if self.selection() == Some(id) {
            self.state.tone = self
                .score()
                .event(id)
                .and_then(|event| event.pitches.iter().position(|p| Some(p) == pitch))
                .unwrap_or(0);
            if let Some(pitch) = self.selected_pitch() {
                self.octave = pitch.octave();
            }
        }
    }
    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    pub fn select(&mut self, id: ElementId) {
        if let Some(index) = self
            .score()
            .part(id.part)
            .and_then(|part| part.staff(id.staff))
            .and_then(|staff| staff.measures().get(id.measure))
            .and_then(|measure| measure.index_of(id.event))
        {
            let switched = self.cursor().part != id.part || self.cursor().staff != id.staff;
            self.state.selection = Some(id);
            self.state.range_anchor = None;
            self.state.tone = 0;
            self.state.cursor = Cursor {
                staff: id.staff,
                part: id.part,
                measure: id.measure,
                index: index + 1,
            };
            if let Some(pitch) = self
                .score()
                .event(id)
                .and_then(|event| event.pitches.first())
            {
                self.octave = pitch.octave();
            } else if switched {
                self.octave = self
                    .score()
                    .part(id.part)
                    .expect("selected part")
                    .entry_octave(id.staff);
            }
        }
    }

    pub fn set_cursor(&mut self, cursor: Cursor) {
        if self
            .score()
            .part(cursor.part)
            .and_then(|part| part.staff(cursor.staff))
            .and_then(|staff| staff.measures().get(cursor.measure))
            .is_some_and(|measure| cursor.index <= measure.len())
        {
            if self.cursor().part != cursor.part || self.cursor().staff != cursor.staff {
                self.octave = self
                    .score()
                    .part(cursor.part)
                    .expect("cursor part")
                    .entry_octave(cursor.staff);
            }
            self.state.cursor = cursor;
            self.state.selection = None;
            self.state.range_anchor = None;
        }
    }

    /// Move through events and empty measures, keeping the insertion point visible.
    pub fn navigate(&mut self, forward: bool) {
        let cursor = self.cursor();
        let measure = &self
            .score()
            .part(cursor.part)
            .expect("cursor part")
            .staff(cursor.staff)
            .expect("cursor staff")
            .measures()[cursor.measure];
        let index = if forward {
            Some(cursor.index)
        } else if self.selection().is_some() {
            cursor.index.checked_sub(2)
        } else {
            cursor.index.checked_sub(1)
        };
        if let Some(id) = index.and_then(|index| measure.id_at(index)) {
            self.select(ElementId {
                staff: cursor.staff,
                part: cursor.part,
                measure: cursor.measure,
                event: id,
            });
        } else if forward && cursor.measure + 1 < self.score().measures().len() {
            self.set_cursor(Cursor {
                staff: cursor.staff,
                part: cursor.part,
                measure: cursor.measure + 1,
                index: 0,
            });
        } else if !forward && cursor.index > 0 {
            self.set_cursor(Cursor {
                staff: cursor.staff,
                part: cursor.part,
                measure: cursor.measure,
                index: 0,
            });
        } else if !forward && cursor.measure > 0 {
            let previous = cursor.measure - 1;
            let len = self
                .score()
                .part(cursor.part)
                .expect("cursor part")
                .staff(cursor.staff)
                .expect("cursor staff")
                .measures()[previous]
                .len();
            self.set_cursor(Cursor {
                staff: cursor.staff,
                part: cursor.part,
                measure: previous,
                index: len,
            });
        } else if forward {
            self.set_cursor(Cursor {
                staff: cursor.staff,
                part: cursor.part,
                measure: cursor.measure,
                index: measure.len(),
            });
        }
    }

    pub fn execute(&mut self, command: Command) -> Result<bool, ScoreError> {
        let mut next = self.state.clone();
        match command {
            Command::SetArticulation(value) => {
                let id = next
                    .selection
                    .ok_or(ScoreError("Selecciona una nota o acorde"))?;
                if next.score.event(id).unwrap().notation.articulation == value {
                    return Ok(false);
                }
                next.score.set_articulation(id, value)?;
            }
            Command::SetTechnique(value) => {
                let id = next
                    .selection
                    .ok_or(ScoreError("Selecciona una nota o silencio"))?;
                if next.score.event(id).unwrap().notation.technique == value {
                    return Ok(false);
                }
                next.score.set_technique(id, value)?;
            }
            Command::SetDotted(value) | Command::SetTriplet(value) => {
                let ids = self.selected_ids();
                if ids.is_empty() {
                    return Err(ScoreError("Selecciona notas, acordes o silencios"));
                }
                let mut changed = false;
                for id in ids {
                    let old = next.score.event(id).unwrap().duration;
                    let duration = if matches!(command, Command::SetDotted(_)) {
                        old.with_dotted(value)
                    } else {
                        old.with_triplet(value)
                    };
                    if duration != old {
                        next.score.set_duration(id, duration)?;
                        changed = true;
                    }
                }
                if !changed {
                    return Ok(false);
                }
            }
            Command::SetDynamic(value) => {
                let id = next
                    .selection
                    .ok_or(ScoreError("Selecciona una nota o silencio"))?;
                if next.score.event(id).unwrap().notation.dynamic == value {
                    return Ok(false);
                }
                next.score.set_dynamic(id, value)?;
            }
            Command::AddConnection(connection) => {
                if next.score.connections().contains(&connection) {
                    return Ok(false);
                }
                next.score.add_connection(connection)?;
            }
            Command::RemoveConnection(connection) => {
                if !next.score.connections().contains(&connection) {
                    return Ok(false);
                }
                next.score.remove_connection(connection);
            }
            Command::RemoveConnections(kind) => {
                let id = next
                    .selection
                    .ok_or(ScoreError("Selecciona un extremo de la ligadura"))?;
                let before = next.score.connections().len();
                next.score.remove_connections(id, kind);
                if before == next.score.connections().len() {
                    return Ok(false);
                }
            }
            Command::Insert(event) => {
                let measure = &next
                    .score
                    .part(next.cursor.part)
                    .expect("cursor part")
                    .staff(next.cursor.staff)
                    .expect("cursor staff")
                    .measures()[next.cursor.measure];
                if next.cursor.index == measure.len()
                    && measure.used_ticks() == next.score.time().capacity()
                {
                    next.cursor.measure += 1;
                    next.cursor.index = 0;
                    if next.cursor.measure == next.score.measures().len() {
                        next.score.append_measure()?;
                    }
                }
                let id = next.score.insert_on(
                    next.cursor.part,
                    next.cursor.staff,
                    next.cursor.measure,
                    next.cursor.index,
                    event,
                )?;
                next.cursor.index += 1;
                next.selection = Some(id);
                next.tone = 0;
            }
            Command::Delete => {
                let ids = self.selected_ids();
                let Some(first) = ids.first().copied() else {
                    return Ok(false);
                };
                let index = next
                    .score
                    .part(first.part)
                    .unwrap()
                    .staff(first.staff)
                    .unwrap()
                    .measures()[first.measure]
                    .index_of(first.event)
                    .unwrap();
                for id in ids.iter().rev() {
                    next.score.remove(*id)?;
                }
                next.cursor = Cursor {
                    part: first.part,
                    staff: first.staff,
                    measure: first.measure,
                    index,
                };
                next.selection = None;
            }
            Command::Transpose(steps) => {
                if steps == 0 {
                    return Ok(false);
                }
                let Some(id) = next.selection else {
                    return Ok(false);
                };
                next.score.transpose(id, steps)?;
            }
            Command::AddPitch(pitch) => {
                let id = next
                    .selection
                    .ok_or(ScoreError("Selecciona una nota para formar un acorde"))?;
                next.score.add_pitch(id, pitch)?;
                next.tone = next
                    .score
                    .event(id)
                    .unwrap()
                    .pitches
                    .iter()
                    .position(|p| p == pitch)
                    .unwrap();
            }
            Command::SetAccidental(accidental) => {
                let id = next.selection.ok_or(ScoreError("Selecciona una nota"))?;
                let pitch = next
                    .score
                    .event(id)
                    .and_then(|event| event.pitches.iter().nth(next.tone))
                    .ok_or(ScoreError("Un silencio no admite alteraciones"))?;
                if pitch.accidental() == accidental {
                    return Ok(false);
                }
                next.score.set_accidental(id, next.tone, accidental)?;
            }
            Command::RemoveHighestPitch => {
                let id = next.selection.ok_or(ScoreError("Selecciona un acorde"))?;
                next.score.remove_highest_pitch(id)?;
                next.tone = next
                    .tone
                    .min(next.score.event(id).unwrap().pitches.len() - 1);
            }
            Command::SetKeySignature(key) => {
                if next.score.key_signature() == key {
                    return Ok(false);
                }
                next.score.set_key_signature(key);
            }
            Command::AppendMeasure => {
                next.score.append_measure()?;
                next.cursor = Cursor {
                    staff: next.cursor.staff,
                    part: next.cursor.part,
                    measure: next.score.measures().len() - 1,
                    index: 0,
                };
                next.selection = None;
            }
            Command::AddPart(instrument) => {
                let part = next.score.add_part(instrument)?;
                next.score.arrange_parts();
                next.cursor = Cursor {
                    staff: score_core::StaffId::Upper,
                    part,
                    measure: 0,
                    index: 0,
                };
                next.selection = None;
            }
            Command::RemovePart => {
                let index = next
                    .score
                    .parts()
                    .iter()
                    .position(|part| part.id() == next.cursor.part)
                    .expect("cursor part");
                next.score.remove_part(next.cursor.part)?;
                let neighbor = next.score.parts()[index.min(next.score.parts().len() - 1)].id();
                next.cursor = Cursor {
                    staff: score_core::StaffId::Upper,
                    part: neighbor,
                    measure: next.cursor.measure,
                    index: 0,
                };
                next.selection = None;
            }
        }
        if !matches!(command, Command::SetDotted(_) | Command::SetTriplet(_)) {
            next.range_anchor = None;
        }
        self.commit(next);
        if let Command::AddPitch(pitch) = command {
            self.octave = pitch.octave();
        }
        if matches!(command, Command::AddPart(_) | Command::RemovePart) {
            self.octave = self
                .score()
                .part(self.cursor().part)
                .expect("cursor part")
                .entry_octave(self.cursor().staff);
        }
        Ok(true)
    }

    fn commit(&mut self, next: Snapshot) {
        self.undo.push(std::mem::replace(&mut self.state, next));
        self.redo.clear();
        if let Some(pitch) = self.selected_pitch() {
            self.octave = pitch.octave();
        }
    }

    pub fn select_staff(&mut self, staff: StaffId) {
        if staff == self.cursor().staff {
            return;
        }
        let part = self.cursor().part;
        if self
            .score()
            .part(part)
            .and_then(|part| part.staff(staff))
            .is_some()
        {
            self.set_cursor(Cursor {
                staff,
                part,
                measure: self.cursor().measure,
                index: 0,
            });
            self.octave = self.score().part(part).expect("part").entry_octave(staff);
        }
    }

    pub fn select_part(&mut self, part: PartId) {
        if part == self.cursor().part {
            return;
        }
        if let Some(instrument) = self.score().part(part).map(|part| part.instrument()) {
            self.set_cursor(Cursor {
                staff: score_core::StaffId::Upper,
                part,
                measure: self.cursor().measure,
                index: 0,
            });
            self.octave = instrument.entry_octave();
        }
    }

    pub fn undo(&mut self) -> bool {
        let Some(previous) = self.undo.pop() else {
            return false;
        };
        self.redo.push(std::mem::replace(&mut self.state, previous));
        self.sync_octave();
        true
    }
    pub fn redo(&mut self) -> bool {
        let Some(next) = self.redo.pop() else {
            return false;
        };
        self.undo.push(std::mem::replace(&mut self.state, next));
        self.sync_octave();
        true
    }

    fn sync_octave(&mut self) {
        if let Some(pitch) = self.selected_pitch() {
            self.octave = pitch.octave();
        } else {
            self.octave = self
                .score()
                .part(self.cursor().part)
                .expect("cursor part")
                .entry_octave(self.cursor().staff);
        }
    }

    pub fn note(&self, step: score_core::Step) -> Event {
        Event {
            notation: Default::default(),
            pitches: score_core::Pitches::single(Pitch::new(step, self.octave)),
            duration: self.duration,
        }
    }
    pub fn rest(&self) -> Event {
        Event {
            notation: Default::default(),
            pitches: score_core::Pitches::single(None),
            duration: self.duration,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use score_core::Step;

    #[test]
    fn changing_key_reinterprets_implicit_notes_without_moving_or_rewriting_them_and_undoes() {
        use score_core::{KeyMode, KeySignature};
        let mut editor = Editor::default();
        editor
            .execute(Command::Insert(editor.note(Step::F)))
            .unwrap();
        let id = editor.selection().unwrap();
        editor
            .execute(Command::AddPitch(Pitch::new(Step::C, 5).unwrap()))
            .unwrap();
        let before = editor.score().event(id).copied();
        let cursor = editor.cursor();
        let selected = editor.selected_pitch();
        let key = KeySignature::new(2, KeyMode::Major).unwrap();
        assert!(editor.execute(Command::SetKeySignature(key)).unwrap());
        assert_eq!(editor.score().event(id).copied(), before);
        assert_eq!(editor.cursor(), cursor);
        assert_eq!(editor.selected_pitch(), selected);
        assert_eq!(
            editor
                .selected_pitch()
                .unwrap()
                .in_key(editor.score().key_signature())
                .to_string(),
            "C♯5"
        );
        assert!(!editor.execute(Command::SetKeySignature(key)).unwrap());
        editor
            .execute(Command::SetAccidental(Some(Accidental::Natural)))
            .unwrap();
        assert_eq!(
            editor.selected_pitch().unwrap().in_key(key).to_string(),
            "C♮5"
        );
        editor.undo();
        editor.undo();
        assert_eq!(editor.score().key_signature(), KeySignature::default());
        assert_eq!(editor.score().event(id).copied(), before);
        assert_eq!(editor.selected_pitch(), selected);
        editor.redo();
        assert_eq!(editor.score().key_signature(), key);
    }

    #[test]
    fn altering_one_chord_tone_keeps_its_neighbors_and_undo_restores_the_target() {
        let mut editor = Editor::default();
        editor
            .execute(Command::Insert(editor.note(Step::C)))
            .unwrap();
        for step in [Step::E, Step::G] {
            editor
                .execute(Command::AddPitch(Pitch::new(step, 4).unwrap()))
                .unwrap();
        }
        let right = editor.selection().unwrap();
        editor.select_tone(1);
        let cursor = editor.cursor();
        let before = editor.score().clone();
        assert_eq!(editor.selected_pitch().unwrap().step, Step::E);
        editor
            .execute(Command::SetAccidental(Some(Accidental::Flat)))
            .unwrap();
        assert_eq!(editor.cursor(), cursor);
        assert_eq!(editor.selected_pitch().unwrap().to_string(), "E♭4");
        let pitches = editor.score().event(right).unwrap().pitches;
        assert_eq!(pitches.first(), Pitch::new(Step::C, 4));
        assert_eq!(pitches.last(), Pitch::new(Step::G, 4));
        editor.undo();
        assert_eq!(editor.score(), &before);
        assert_eq!(editor.selected_pitch().unwrap().step, Step::E);
        editor.redo();
        editor.execute(Command::Transpose(7)).unwrap();
        assert_eq!(editor.selected_pitch().unwrap().to_string(), "E♭5");
        editor.select_staff(StaffId::Lower);
        editor
            .execute(Command::Insert(editor.note(Step::C)))
            .unwrap();
        editor
            .execute(Command::SetAccidental(Some(Accidental::Sharp)))
            .unwrap();
        assert_eq!(editor.selected_pitch().unwrap().to_string(), "C♯3");
        assert_eq!(
            editor
                .score()
                .event(right)
                .unwrap()
                .pitches
                .iter()
                .nth(1)
                .unwrap()
                .to_string(),
            "E♭5"
        );
        editor.execute(Command::Insert(editor.rest())).unwrap();
        let before = editor.score().clone();
        assert!(
            editor
                .execute(Command::SetAccidental(Some(Accidental::Natural)))
                .is_err()
        );
        assert_eq!(editor.score(), &before);
    }

    #[test]
    fn adding_chord_tones_keeps_cursor_duration_and_other_hand_then_undoes() {
        let mut editor = Editor::default();
        editor
            .execute(Command::Insert(editor.note(Step::C)))
            .unwrap();
        let right = editor.selection().unwrap();
        let cursor = editor.cursor();
        editor.duration = Duration::Whole; // Adding a tone preserves the selected quarter.
        for step in [Step::E, Step::G] {
            editor
                .execute(Command::AddPitch(Pitch::new(step, 4).unwrap()))
                .unwrap();
        }
        assert_eq!(editor.cursor(), cursor);
        assert_eq!(editor.selection(), Some(right));
        assert_eq!(
            editor.score().event(right).unwrap().duration,
            Duration::Quarter
        );
        assert_eq!(editor.score().measures()[0].len(), 1);
        let before = editor.score().clone();
        assert!(
            editor
                .execute(Command::AddPitch(Pitch::new(Step::G, 4).unwrap()))
                .is_err()
        );
        assert_eq!(editor.score(), &before);
        editor.select_staff(StaffId::Lower);
        editor
            .execute(Command::Insert(editor.note(Step::C)))
            .unwrap();
        let left = editor.selection().unwrap();
        editor
            .execute(Command::AddPitch(Pitch::new(Step::G, 3).unwrap()))
            .unwrap();
        assert_eq!(editor.score().event(left).unwrap().pitches.len(), 2);
        assert_eq!(editor.score().event(right).unwrap().pitches.len(), 3);
        editor.execute(Command::RemoveHighestPitch).unwrap();
        assert_eq!(editor.score().event(left).unwrap().pitches.len(), 1);
        editor.undo();
        assert_eq!(editor.score().event(left).unwrap().pitches.len(), 2);
        editor.undo();
        assert_eq!(editor.score().event(left).unwrap().pitches.len(), 1);
        editor.redo();
        assert_eq!(editor.score().event(left).unwrap().pitches.len(), 2);
        editor.execute(Command::Delete).unwrap();
        assert!(editor.score().event(left).is_none());
        assert!(editor.score().event(right).is_some());
        editor.score().validate().unwrap();
    }

    #[test]
    fn missing_selection_or_rest_cannot_silently_create_a_chord() {
        let mut editor = Editor::default();
        let command = Command::AddPitch(Pitch::new(Step::C, 4).unwrap());
        assert!(editor.execute(command).is_err());
        assert!(!editor.can_undo());
        editor.execute(Command::Insert(editor.rest())).unwrap();
        let before = editor.score().clone();
        assert!(editor.execute(command).is_err());
        assert_eq!(editor.score(), &before);
    }

    #[test]
    fn writing_navigating_and_undoing_the_left_hand_leave_the_right_hand_intact() {
        let mut editor = Editor::default();
        let piano = editor.cursor().part;
        editor
            .execute(Command::Insert(editor.note(Step::E)))
            .unwrap();
        let right = editor.selection().unwrap();
        let right_pitch = editor.score().event(right).unwrap().pitches.first();
        editor.select_staff(StaffId::Lower);
        assert_eq!(editor.octave, 3);
        for _ in 0..17 {
            editor
                .execute(Command::Insert(editor.note(Step::C)))
                .unwrap();
        }
        let left = editor.selection().unwrap();
        assert_eq!(left.staff, StaffId::Lower);
        assert_eq!(left.measure, 4);
        assert_eq!(
            editor.score().event(right).unwrap().pitches.first(),
            right_pitch
        );
        editor.navigate(false);
        editor.navigate(true);
        assert_eq!(editor.cursor().staff, StaffId::Lower);
        editor.select(left);
        editor.execute(Command::Transpose(-1)).unwrap();
        editor.execute(Command::Delete).unwrap();
        assert!(editor.score().event(left).is_none());
        editor.undo();
        editor.undo();
        assert_eq!(editor.selection(), Some(left));
        assert_eq!(editor.cursor().staff, StaffId::Lower);
        assert_eq!(editor.octave, 3);
        assert_eq!(
            editor.score().event(right).unwrap().pitches.first(),
            right_pitch
        );
        assert!(
            editor
                .score()
                .part(piano)
                .unwrap()
                .staves()
                .iter()
                .all(|staff| staff.measures().len() == 5)
        );
        editor.execute(Command::AddPart(Instrument::Flute)).unwrap();
        editor.select_part(piano);
        editor.execute(Command::RemovePart).unwrap();
        editor.undo();
        assert!(editor.score().event(left).is_some());
        assert!(editor.score().event(right).is_some());
        editor.score().validate().unwrap();
    }

    #[test]
    fn choosing_an_empty_left_hand_sets_its_entry_octave() {
        let mut editor = Editor::default();
        editor.set_cursor(Cursor {
            part: editor.cursor().part,
            staff: StaffId::Lower,
            measure: 2,
            index: 0,
        });
        assert_eq!(editor.octave, 3);
        editor.execute(Command::Insert(editor.rest())).unwrap();
        let rest = editor.selection().unwrap();
        editor.select_staff(StaffId::Upper);
        assert_eq!(editor.octave, 4);
        editor.select(rest);
        assert_eq!(editor.cursor().staff, StaffId::Lower);
        assert_eq!(editor.octave, 3);
    }

    #[test]
    fn instrument_edits_and_bar_growth_undo_as_complete_transactions() {
        let mut editor = Editor::default();
        editor
            .execute(Command::Insert(editor.note(Step::E)))
            .unwrap();
        let piano_note = editor.selection().unwrap();
        editor.execute(Command::AddPart(Instrument::Cello)).unwrap();
        let cello = editor.cursor().part;
        assert_eq!(editor.octave, 3);
        for _ in 0..17 {
            editor
                .execute(Command::Insert(editor.note(Step::C)))
                .unwrap();
        }
        let selected = editor.selection().unwrap();
        assert_eq!(selected.part, cello);
        assert_eq!(editor.score().parts()[0].measures()[0].len(), 1);
        assert!(
            editor
                .score()
                .parts()
                .iter()
                .all(|part| part.measures().len() == 5)
        );
        editor.execute(Command::RemovePart).unwrap();
        assert!(editor.score().part(cello).is_none());
        assert!(editor.undo());
        assert_eq!(editor.selection(), Some(selected));
        assert!(editor.score().event(piano_note).is_some());
        assert!(editor.redo());
        assert_eq!(editor.cursor().part, piano_note.part);
        let before = editor.score().clone();
        assert!(editor.execute(Command::RemovePart).is_err());
        assert_eq!(editor.score(), &before);
        editor.score().validate().unwrap();
    }

    #[test]
    fn selection_navigation_and_deletion_stay_on_the_chosen_part() {
        let mut editor = Editor::default();
        let piano = editor.cursor().part;
        editor
            .execute(Command::Insert(editor.note(Step::C)))
            .unwrap();
        let piano_note = editor.selection().unwrap();
        editor.execute(Command::AddPart(Instrument::Flute)).unwrap();
        editor
            .execute(Command::Insert(editor.note(Step::D)))
            .unwrap();
        let flute_note = editor.selection().unwrap();
        editor.navigate(false);
        editor.navigate(true);
        assert_eq!(editor.selection(), Some(flute_note));
        editor.execute(Command::Delete).unwrap();
        assert!(editor.score().event(flute_note).is_none());
        assert!(editor.score().event(piano_note).is_some());
        editor.select_part(piano);
        assert_eq!(editor.cursor().part, piano);
        editor.select(piano_note);
        editor.execute(Command::Transpose(1)).unwrap();
        assert_eq!(
            editor
                .score()
                .event(piano_note)
                .unwrap()
                .pitches
                .first()
                .unwrap()
                .step,
            Step::D
        );
    }

    #[test]
    fn full_measure_advances_and_history_restores_cursor() {
        let mut editor = Editor::default();
        for _ in 0..5 {
            editor
                .execute(Command::Insert(editor.note(Step::C)))
                .unwrap();
        }
        let selected = editor.selection();
        assert_eq!(
            editor.cursor(),
            Cursor {
                staff: score_core::StaffId::Upper,
                part: PartId(0),
                measure: 1,
                index: 1
            }
        );
        assert!(editor.undo());
        assert_eq!(
            editor.cursor(),
            Cursor {
                staff: score_core::StaffId::Upper,
                part: PartId(0),
                measure: 0,
                index: 4
            }
        );
        assert!(editor.redo());
        assert_eq!(editor.selection(), selected);
        editor.score().validate().unwrap();
    }

    #[test]
    fn failed_edit_preserves_history_and_branch_clears_redo() {
        let mut editor = Editor::default();
        let original = editor.score().clone();
        editor
            .execute(Command::Insert(editor.note(Step::D)))
            .unwrap();
        let before = editor.score().clone();
        let cursor = editor.cursor();
        assert!(
            editor
                .execute(Command::Insert(Event {
                    notation: Default::default(),
                    pitches: score_core::Pitches::single(None),
                    duration: Duration::Whole
                }))
                .is_err()
        );
        assert_eq!(editor.score(), &before);
        assert_eq!(editor.cursor(), cursor);
        editor.undo();
        assert_eq!(editor.score(), &original);
        editor.execute(Command::Insert(editor.rest())).unwrap();
        assert!(!editor.can_redo());
    }

    #[test]
    fn long_edit_undo_sequence_preserves_invariants() {
        let mut editor = Editor::default();
        let original = editor.score().clone();
        for index in 0..300 {
            let command = match index % 7 {
                0 => Command::AppendMeasure,
                1 => Command::Transpose(1),
                2 => Command::Delete,
                _ => Command::Insert(editor.note(Step::E)),
            };
            let _ = editor.execute(command);
            editor.score().validate().unwrap();
        }
        while editor.undo() {
            editor.score().validate().unwrap();
        }
        assert_eq!(editor.score(), &original);
        while editor.redo() {
            editor.score().validate().unwrap();
        }
    }

    #[test]
    fn undo_transposition_restores_input_octave() {
        let mut editor = Editor::default();
        editor
            .execute(Command::Insert(editor.note(Step::C)))
            .unwrap();
        editor.execute(Command::Transpose(-1)).unwrap();
        assert_eq!(editor.octave, 3);
        editor.undo();
        assert_eq!(editor.note(Step::D).pitches.first(), Pitch::new(Step::D, 4));
        editor.redo();
        assert_eq!(editor.note(Step::D).pitches.first(), Pitch::new(Step::D, 3));
    }
}

#[cfg(test)]
mod expression_tests;

#[cfg(test)]
mod arrangement_tests;

#[cfg(test)]
mod clipboard_tests;
