//! Transactional editing and snapshot history. No dependency on the UI.
use score_core::{
    Accidental, Duration, ElementId, Event, Instrument, PartId, Pitch, Score, ScoreError, StaffId,
};

pub mod clipboard;
#[cfg(test)]
mod comment_tests;
#[cfg(test)]
mod instrument_tests;
#[cfg(test)]
mod move_tests;
#[cfg(test)]
mod range_tests;
mod selection;
#[cfg(test)]
mod write_tests;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Cursor {
    pub staff: StaffId,
    pub voice: score_core::Voice,
    pub part: PartId,
    pub measure: usize,
    pub index: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FormEdit {
    ToggleRepeatStart,
    ToggleRepeatEnd,
    Barline(score_core::Barline),
    ToggleSegno,
    ToggleCoda,
    ToggleToCoda,
    ToggleFine,
    Jump(Option<score_core::Jump>),
    Clear,
}

#[derive(Clone, Debug)]
pub enum Command {
    SaveComment {
        id: Option<score_core::CommentId>,
        text: String,
        anchors: Vec<ElementId>,
    },
    SetCommentResolved(score_core::CommentId, bool),
    RemoveComment(score_core::CommentId),
    EditForm(FormEdit),
    SetEnding(Vec<u8>),
    ToggleRehearsalMark,
    ToggleLayoutBreak(score_core::LayoutBreak),
    SetRehearsalStyle(score_core::RehearsalStyle),
    RemoveEndings,
    Insert(Event),
    /// Sibelius-style entry: overwrite from the insertion point.
    Write(Event),
    /// Overwrite at a tick of the cursor's part, staff and voice.
    WriteAt {
        measure: usize,
        tick: u16,
        event: Event,
    },
    SetPartDivisi(bool),
    SetPassageDivisi(bool),
    SetArticulation(Option<score_core::Articulation>),
    SetArpeggioBothHands(bool),
    SetDynamic(Option<score_core::Dynamic>),
    SetDotted(bool),
    SetTriplet(bool),
    SetTechnique(Option<score_core::PlayingTechnique>),
    AddConnection(score_core::Connection),
    RemoveConnections(score_core::ConnectionKind),
    RemoveConnection(score_core::Connection),
    /// One step: swap a span mark for another, such as crescendo for diminuendo.
    /// The original stays if the replacement is not valid.
    ReplaceConnection(score_core::Connection, score_core::Connection),
    /// Delete one mark selected in the score, keeping its note.
    RemoveMark(score_core::MarkRef),
    /// Move one written mark in tenths of a staff space; zero restores engraving.
    SetMarkOffset(score_core::MarkRef, score_core::Offset),
    /// Take a mark to another note, or a line to other ends, on the same staff,
    /// with its manual offset there.
    MoveMark(score_core::MarkRef, score_core::MarkRef, score_core::Offset),
    SetBarMarkOffset(score_core::BarMarkRef, score_core::Offset),
    ResetBarMarkOffset(score_core::BarMarkRef),
    /// Restore one mark, or all marks on selected notes and spans starting there.
    ResetMarkOffsets(Option<score_core::MarkRef>),
    /// The ribbon's point marks: write `kind` on the selection, replacing its
    /// family's mark, or remove it when it is already there.
    ToggleMark(score_core::MarkKind),
    Delete,
    /// Clear pitches while keeping all event onsets unchanged.
    Erase,
    ReplacePitch(Pitch),
    MoveSelectedEvent {
        measure: usize,
        tick: u16,
    },
    SetDuration(score_core::NoteValue),
    InsertMeasure,
    DeleteMeasures,
    Transpose(i16),
    AddPitch(Pitch),
    RemoveHighestPitch,
    SetAccidental(Option<Accidental>),
    AppendMeasure,
    AddPart(Instrument),
    /// Add an ensemble atomically, including repeated instruments.
    AddParts(Vec<Instrument>),
    RemovePart,
    RemovePartById(PartId),
    SetKeySignature(score_core::KeySignature),
    SetKeySignatureAt(usize, score_core::KeySignature),
    SetTimeSignatureAt(usize, score_core::TimeSignature),
    SetClefAt(PartId, StaffId, usize, score_core::Clef),
    RemoveClefChange(PartId, StaffId, usize),
    RemoveKeyChange(usize),
    RemoveTimeChange(usize),
    SetTempoAt(usize, score_core::TempoMark),
    /// Title and the credits printed around it.
    SetHeader(String, score_core::Credits),
    RemoveTempoChange(usize),
    SetInstrumentRange(PartId, Option<score_core::InstrumentRange>),
    /// Solo or section recordings for one part; `None` lets the score decide.
    SetPlayers(PartId, Option<score_core::Players>),
}

#[derive(Clone)]
struct Snapshot {
    score: Score,
    cursor: Cursor,
    selection: Option<ElementId>,
    range_anchor: Option<ElementId>,
    divisi_anchor: Option<score_core::DivisiPoint>,
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
                divisi_anchor: None,
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
        self.state.divisi_anchor = None;
        if let Some(index) = self
            .score()
            .part(id.part)
            .and_then(|part| part.staff(id.staff))
            .and_then(|staff| staff.measures().get(id.measure))
            .and_then(|measure| {
                measure.voice_index_of(self.score().voice_of(id).unwrap(), id.event)
            })
        {
            let switched = self.cursor().part != id.part || self.cursor().staff != id.staff;
            self.state.selection = Some(id);
            self.state.range_anchor = None;
            self.state.tone = 0;
            self.state.cursor = Cursor {
                voice: self.score().voice_of(id).unwrap(),
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
                    .entry_octave_at(id.staff, id.measure);
            }
        }
    }

    pub fn select_voice(&mut self, voice: score_core::Voice) {
        if self.cursor().voice == voice {
            return;
        }
        self.set_cursor(Cursor {
            voice,
            index: 0,
            ..self.cursor()
        });
    }

    pub fn set_cursor(&mut self, cursor: Cursor) {
        if self
            .score()
            .part(cursor.part)
            .and_then(|part| part.staff(cursor.staff))
            .and_then(|staff| staff.measures().get(cursor.measure))
            .is_some_and(|measure| cursor.index <= measure.voice_len(cursor.voice))
        {
            if self.cursor().part != cursor.part || self.cursor().staff != cursor.staff {
                self.octave = self
                    .score()
                    .part(cursor.part)
                    .expect("cursor part")
                    .entry_octave_at(cursor.staff, cursor.measure);
            }
            self.state.divisi_anchor = None;
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
        if let Some(id) = index.and_then(|index| measure.voice_id_at(cursor.voice, index)) {
            self.select(ElementId {
                staff: cursor.staff,
                part: cursor.part,
                measure: cursor.measure,
                event: id,
            });
        } else if forward && cursor.measure + 1 < self.score().measures().len() {
            self.set_cursor(Cursor {
                voice: cursor.voice,
                staff: cursor.staff,
                part: cursor.part,
                measure: cursor.measure + 1,
                index: 0,
            });
        } else if !forward && cursor.index > 0 {
            self.set_cursor(Cursor {
                voice: cursor.voice,
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
                .voice_len(cursor.voice);
            self.set_cursor(Cursor {
                voice: cursor.voice,
                staff: cursor.staff,
                part: cursor.part,
                measure: previous,
                index: len,
            });
        } else if forward {
            self.set_cursor(Cursor {
                voice: cursor.voice,
                staff: cursor.staff,
                part: cursor.part,
                measure: cursor.measure,
                index: measure.voice_len(cursor.voice),
            });
        }
    }

    pub fn execute(&mut self, command: Command) -> Result<bool, ScoreError> {
        let mut next = self.state.clone();
        match command {
            Command::SaveComment {
                id,
                ref text,
                ref anchors,
            } => {
                if !next.score.save_comment(id, text, anchors)? {
                    return Ok(false);
                }
            }
            Command::SetCommentResolved(id, resolved) => {
                if !next.score.set_comment_resolved(id, resolved)? {
                    return Ok(false);
                }
            }
            Command::RemoveComment(id) => {
                next.score.remove_comment(id)?;
            }
            Command::SetInstrumentRange(part, range) => {
                if !next.score.set_instrument_range(part, range)? {
                    return Ok(false);
                }
            }
            Command::SetPlayers(part, players) => {
                if !next.score.set_players(part, players)? {
                    return Ok(false);
                }
            }
            Command::SetPassageDivisi(divided) => {
                next.set_passage_divisi(divided, false)?;
            }
            Command::ToggleMark(score_core::MarkKind::Division(mark))
                if !next
                    .score
                    .part(next.cursor.part)
                    .unwrap()
                    .instrument()
                    .is_grand_staff() =>
            {
                next.set_passage_divisi(mark == score_core::Division::Divisi, true)?;
            }
            Command::SetPartDivisi(divided) => {
                let part = next.cursor.part;
                if next.score.part(part).unwrap().is_divisi() == divided {
                    return Ok(false);
                }
                let remap = next.score.set_part_divisi(part, divided)?;
                next.selection = next
                    .selection
                    .map(|id| remap.get(&id).copied().unwrap_or(id));
                next.range_anchor = next
                    .range_anchor
                    .map(|id| remap.get(&id).copied().unwrap_or(id));
                if divided && next.cursor.voice == score_core::Voice::Two {
                    next.cursor.staff = StaffId::Lower;
                    next.cursor.voice = score_core::Voice::One;
                } else if !divided && next.cursor.staff == StaffId::Lower {
                    next.cursor.staff = StaffId::Upper;
                    next.cursor.voice = score_core::Voice::Two;
                }
                if let Some(id) = next.selection {
                    next.cursor.staff = id.staff;
                    next.cursor.voice = next.score.voice_of(id).unwrap();
                    next.cursor.measure = id.measure;
                    next.cursor.index = next
                        .score
                        .part(part)
                        .unwrap()
                        .staff(id.staff)
                        .unwrap()
                        .measures()[id.measure]
                        .voice_index_of(next.cursor.voice, id.event)
                        .unwrap()
                        + 1;
                } else {
                    next.cursor.index = next.cursor.index.min(
                        next.score
                            .part(part)
                            .unwrap()
                            .staff(next.cursor.staff)
                            .unwrap()
                            .measures()[next.cursor.measure]
                            .voice_len(next.cursor.voice),
                    );
                }
            }
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
                let edits: Vec<_> = ids
                    .into_iter()
                    .filter_map(|id| {
                        let old = next.score.event(id).unwrap().duration;
                        let new = if matches!(command, Command::SetDotted(_)) {
                            old.with_dotted(value)
                        } else {
                            old.with_triplet(value)
                        };
                        (new != old).then_some((id, new))
                    })
                    .collect();
                if edits.is_empty() {
                    return Ok(false);
                }
                next.score.set_durations(&edits)?;
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
            Command::ReplaceConnection(old, new) => {
                if old == new {
                    return Ok(false);
                }
                if !next.score.connections().contains(&old) {
                    return Err(ScoreError("La marca ya no está en la partitura"));
                }
                let offset = next.score.mark_offset(score_core::MarkRef::Connection(old));
                next.score.remove_connection(old);
                next.score.add_connection(new)?;
                // A stretched or reversed mark stays where the user nudged it.
                if offset != score_core::Offset::ZERO {
                    next.score
                        .set_mark_offset(score_core::MarkRef::Connection(new), offset)?;
                }
            }
            Command::RemoveMark(mark) => next.score.remove_mark(mark)?,
            Command::SetMarkOffset(mark, offset) => {
                if !next.score.set_mark_offset(mark, offset)? {
                    return Ok(false);
                }
            }
            Command::MoveMark(from, to, offset) => {
                if !next.score.move_mark(from, to, offset)? {
                    return Ok(false);
                }
            }
            Command::SetBarMarkOffset(mark, offset) => {
                if !next.score.set_bar_mark_offset(mark, offset)? {
                    return Ok(false);
                }
            }
            Command::ResetBarMarkOffset(mark) => {
                if !next
                    .score
                    .set_bar_mark_offset(mark, score_core::Offset::ZERO)?
                {
                    return Ok(false);
                }
            }
            Command::ResetMarkOffsets(Some(mark)) => {
                if !next.score.set_mark_offset(mark, score_core::Offset::ZERO)? {
                    return Ok(false);
                }
            }
            Command::ResetMarkOffsets(None) => {
                let ids: std::collections::HashSet<_> = self.selected_ids().into_iter().collect();
                let mut changed = false;
                for &id in &ids {
                    for family in [
                        score_core::MarkFamily::Dynamic,
                        score_core::MarkFamily::Articulation,
                        score_core::MarkFamily::Fermata,
                        score_core::MarkFamily::Technique,
                        score_core::MarkFamily::Harmonic,
                    ] {
                        if let Some(mark) = score_core::MarkRef::point(family, id)
                            && next.score.has_mark(mark)
                        {
                            changed |=
                                next.score.set_mark_offset(mark, score_core::Offset::ZERO)?;
                        }
                    }
                }
                let connections: Vec<_> = next
                    .score
                    .connection_offsets()
                    .iter()
                    .filter(|entry| ids.contains(&entry.connection.start))
                    .map(|entry| entry.connection)
                    .collect();
                for connection in connections {
                    changed |= next.score.set_mark_offset(
                        score_core::MarkRef::Connection(connection),
                        score_core::Offset::ZERO,
                    )?;
                }
                let bars: std::collections::HashSet<_> = ids.iter().map(|id| id.measure).collect();
                let bar_marks: Vec<_> = next
                    .score
                    .bar_offsets()
                    .iter()
                    .filter(|(bar, _)| bars.contains(bar))
                    .flat_map(|(&measure, offsets)| {
                        score_core::BarMark::ALL
                            .into_iter()
                            .filter_map(move |mark| {
                                offsets
                                    .get(mark)
                                    .map(|_| score_core::BarMarkRef { measure, mark })
                            })
                    })
                    .collect();
                for mark in bar_marks {
                    changed |= next
                        .score
                        .set_bar_mark_offset(mark, score_core::Offset::ZERO)?;
                }
                if !changed {
                    return Ok(false);
                }
            }
            Command::SetArpeggioBothHands(enabled) => {
                let id = next
                    .selection
                    .ok_or(ScoreError("Selecciona una nota o acorde del piano"))?;
                next.score.set_arpeggio_both_hands(id, enabled)?;
            }
            Command::ToggleMark(score_core::MarkKind::Harmonic(mark)) => {
                let ids = self.selected_ids();
                if ids.is_empty() {
                    return Err(ScoreError("Selecciona notas para aplicar el armónico"));
                }
                let clear = ids
                    .iter()
                    .all(|id| next.score.event(*id).unwrap().notation.harmonic == Some(mark));
                let edits: Vec<_> = ids
                    .into_iter()
                    .map(|id| (id, (!clear).then_some(mark)))
                    .collect();
                next.score.set_harmonics(&edits)?;
            }
            Command::ToggleMark(score_core::MarkKind::Tremolo(mark)) => {
                let ids = self.selected_ids();
                if ids.is_empty() {
                    return Err(ScoreError("Selecciona notas para aplicar el trémolo"));
                }
                let clear = ids
                    .iter()
                    .all(|id| next.score.event(*id).unwrap().notation.tremolo == Some(mark));
                for id in ids {
                    next.score.set_tremolo(id, (!clear).then_some(mark))?;
                }
            }
            Command::ToggleMark(kind) => {
                let id = next
                    .selection
                    .ok_or(ScoreError("Selecciona una nota o silencio"))?;
                let family = kind.family();
                let current = next.score.point_mark(id, family);
                next.score
                    .set_point_mark(id, family, (current != Some(kind)).then_some(kind))?;
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
                if next.cursor.index == measure.voice_len(next.cursor.voice)
                    && measure.voice_ticks(next.cursor.voice)
                        == next.score.time_at(next.cursor.measure).capacity()
                {
                    next.cursor.measure += 1;
                    next.cursor.index = 0;
                    if next.cursor.measure == next.score.measures().len() {
                        next.score.append_measure()?;
                    }
                }
                let id = next.score.insert_in_voice(
                    next.cursor.part,
                    next.cursor.staff,
                    next.cursor.measure,
                    next.cursor.voice,
                    next.cursor.index,
                    event,
                )?;
                next.cursor.index += 1;
                next.selection = Some(id);
                next.tone = 0;
            }
            Command::Write(event) => {
                let c = next.cursor;
                let bar = &next
                    .score
                    .part(c.part)
                    .unwrap()
                    .staff(c.staff)
                    .unwrap()
                    .measures()[c.measure];
                let mut tick: u16 = bar
                    .voice_iter(c.voice)
                    .take(c.index)
                    .map(|(_, e)| e.duration.ticks())
                    .sum();
                let mut measure = c.measure;
                if c.index == bar.voice_len(c.voice)
                    && tick == next.score.time_at(measure).capacity()
                {
                    measure += 1;
                    tick = 0;
                    if measure == next.score.measures().len() {
                        next.score.append_measure()?;
                    }
                }
                let written = next
                    .score
                    .overwrite(c.part, c.staff, measure, c.voice, tick, event)?;
                let last = *written.last().unwrap();
                let index = next
                    .score
                    .part(last.part)
                    .unwrap()
                    .staff(last.staff)
                    .unwrap()
                    .measures()[last.measure]
                    .voice_index_of(c.voice, last.event)
                    .unwrap();
                next.selection = Some(last);
                next.cursor = Cursor {
                    measure: last.measure,
                    index: index + 1,
                    ..c
                };
                next.tone = 0;
            }
            Command::WriteAt {
                measure,
                tick,
                event,
            } => {
                let c = next.cursor;
                let written = next
                    .score
                    .overwrite(c.part, c.staff, measure, c.voice, tick, event)?;
                let last = *written.last().unwrap();
                let index = next
                    .score
                    .part(last.part)
                    .unwrap()
                    .staff(last.staff)
                    .unwrap()
                    .measures()[last.measure]
                    .voice_index_of(c.voice, last.event)
                    .unwrap();
                next.selection = Some(last);
                next.cursor = Cursor {
                    measure: last.measure,
                    index: index + 1,
                    ..c
                };
                next.tone = 0;
            }
            Command::Erase => {
                let ids = self.selected_ids();
                if ids.is_empty()
                    || ids
                        .iter()
                        .all(|id| next.score.event(*id).unwrap().pitches.is_empty())
                {
                    return Ok(false);
                }
                for id in ids {
                    next.score.erase(id)?;
                }
            }
            Command::MoveSelectedEvent { measure, tick } => {
                let id = next
                    .selection
                    .ok_or(ScoreError("Selecciona una nota o acorde"))?;
                if measure == id.measure
                    && next.score.event_tick(id)
                        == Some(next.score.measure_start_tick(measure) + u64::from(tick))
                {
                    return Ok(false);
                }
                let moved = next.score.move_event(id, measure, tick)?;
                let voice = next.score.voice_of(moved).unwrap();
                let index = next
                    .score
                    .part(moved.part)
                    .unwrap()
                    .staff(moved.staff)
                    .unwrap()
                    .measures()[measure]
                    .voice_index_of(voice, moved.event)
                    .unwrap();
                next.selection = Some(moved);
                next.cursor = Cursor {
                    part: moved.part,
                    staff: moved.staff,
                    voice,
                    measure,
                    index,
                };
            }
            Command::ReplacePitch(pitch) => {
                let id = next
                    .selection
                    .ok_or(ScoreError("Selecciona una nota o silencio"))?;
                if self.selected_pitch() == Some(pitch) {
                    return Ok(false);
                }
                next.score.replace_pitch(id, next.tone, pitch)?;
                next.tone = next
                    .score
                    .event(id)
                    .unwrap()
                    .pitches
                    .iter()
                    .position(|p| p == pitch)
                    .unwrap();
            }
            Command::SetDuration(value) => {
                let ids = self.selected_ids();
                if ids.is_empty() {
                    return Err(ScoreError("Selecciona notas, acordes o silencios"));
                }
                let edits: Vec<_> = ids
                    .into_iter()
                    .filter_map(|id| {
                        let old = next.score.event(id).unwrap().duration;
                        let new = old.with_value(value);
                        (old != new).then_some((id, new))
                    })
                    .collect();
                if edits.is_empty() {
                    return Ok(false);
                }
                next.score.set_durations(&edits)?;
            }

            Command::InsertMeasure => {
                let bar = next.cursor.measure;
                next.score.insert_measure(bar)?;
                next.cursor = Cursor {
                    index: 0,
                    ..next.cursor
                };
                next.selection = None;
            }
            Command::ToggleRehearsalMark => {
                let bar = self
                    .selected_ids()
                    .iter()
                    .map(|id| id.measure)
                    .min()
                    .unwrap_or(next.cursor.measure);
                if !next.score.toggle_rehearsal_mark(bar)? {
                    return Ok(false);
                }
            }
            Command::ToggleLayoutBreak(kind) => {
                let bar = self
                    .selected_ids()
                    .iter()
                    .map(|id| id.measure)
                    .max()
                    .unwrap_or(next.cursor.measure);
                if !next.score.toggle_layout_break(bar, kind)? {
                    return Ok(false);
                }
            }
            Command::SetRehearsalStyle(style) => {
                if !next.score.set_rehearsal_style(style) {
                    return Ok(false);
                }
            }
            Command::EditForm(_) | Command::SetEnding(_) | Command::RemoveEndings => {
                let ids = self.selected_ids();
                let start = ids
                    .iter()
                    .map(|id| id.measure)
                    .min()
                    .unwrap_or(next.cursor.measure);
                let end = ids.iter().map(|id| id.measure).max().unwrap_or(start);
                let changed = match command {
                    Command::SetEnding(ref numbers) => {
                        next.score.set_ending(score_core::Ending {
                            start,
                            end,
                            numbers: numbers.clone(),
                        })?
                    }
                    Command::RemoveEndings => next.score.remove_endings(start, end),
                    Command::EditForm(FormEdit::Clear) => {
                        let mut changed = next.score.remove_endings(start, end);
                        for bar in start..=end {
                            changed |= next.score.set_bar_form(bar, Default::default())?;
                        }
                        changed
                    }
                    Command::EditForm(edit) => {
                        let bar = if matches!(
                            edit,
                            FormEdit::ToggleRepeatStart
                                | FormEdit::ToggleSegno
                                | FormEdit::ToggleCoda
                        ) {
                            start
                        } else {
                            end
                        };
                        let mut form = next.score.bar_form(bar);
                        match edit {
                            FormEdit::ToggleRepeatStart => form.repeat_start = !form.repeat_start,
                            FormEdit::ToggleRepeatEnd => {
                                form.repeat_end = if form.repeat_end.is_some() {
                                    None
                                } else {
                                    Some(2)
                                }
                            }
                            FormEdit::Barline(style) => form.barline = style,
                            FormEdit::ToggleSegno => form.segno = !form.segno,
                            FormEdit::ToggleCoda => form.coda = !form.coda,
                            FormEdit::ToggleToCoda => form.to_coda = !form.to_coda,
                            FormEdit::ToggleFine => form.fine = !form.fine,
                            FormEdit::Jump(jump) => {
                                form.jump = if form.jump == jump { None } else { jump }
                            }
                            FormEdit::Clear => unreachable!(),
                        }
                        next.score.set_bar_form(bar, form)?
                    }
                    _ => unreachable!(),
                };
                if !changed {
                    return Ok(false);
                }
            }
            Command::DeleteMeasures => {
                let ids = self.selected_ids();
                let start = ids
                    .iter()
                    .map(|id| id.measure)
                    .min()
                    .unwrap_or(next.cursor.measure);
                let end = ids.iter().map(|id| id.measure).max().unwrap_or(start);
                next.score.remove_measures(start, end - start + 1)?;
                next.cursor = Cursor {
                    measure: start.min(next.score.measures().len() - 1),
                    index: 0,
                    ..next.cursor
                };
                next.selection = None;
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
                    .voice_index_of(next.cursor.voice, first.event)
                    .unwrap();
                for id in ids.iter().rev() {
                    next.score.remove(*id)?;
                }
                next.cursor = Cursor {
                    voice: next.cursor.voice,
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
            Command::SetKeySignatureAt(bar, key) => {
                if !next.score.set_key_signature_at(bar, key)? {
                    return Ok(false);
                }
            }
            Command::SetTimeSignatureAt(bar, time) => {
                if !next.score.set_time_signature_at(bar, time)? {
                    return Ok(false);
                }
            }
            Command::SetClefAt(part, staff, bar, clef) => {
                if !next.score.set_clef_at(part, staff, bar, clef)? {
                    return Ok(false);
                }
            }
            Command::RemoveClefChange(part, staff, bar) => {
                if !next.score.remove_clef_change(part, staff, bar)? {
                    return Ok(false);
                }
            }
            Command::RemoveKeyChange(bar) => {
                if !next.score.remove_key_change(bar) {
                    return Ok(false);
                }
            }
            Command::RemoveTimeChange(bar) => {
                if !next.score.remove_time_change(bar)? {
                    return Ok(false);
                }
            }
            Command::SetTempoAt(bar, ref mark) => {
                if !next.score.set_tempo_at(bar, mark.clone())? {
                    return Ok(false);
                }
            }
            Command::SetHeader(ref title, ref credits) => {
                if !next.score.set_header(title, credits)? {
                    return Ok(false);
                }
            }
            Command::RemoveTempoChange(bar) => {
                if !next.score.remove_tempo_change(bar) {
                    return Ok(false);
                }
            }
            Command::AppendMeasure => {
                next.score.append_measure()?;
                next.cursor = Cursor {
                    voice: next.cursor.voice,
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
                    voice: Default::default(),
                    staff: score_core::StaffId::Upper,
                    part,
                    measure: 0,
                    index: 0,
                };
                next.selection = None;
            }
            Command::AddParts(ref instruments) => {
                if instruments.is_empty() {
                    return Ok(false);
                }
                let mut first = None;
                for &instrument in instruments {
                    let id = next.score.add_part(instrument)?;
                    first.get_or_insert(id);
                }
                next.score.arrange_parts();
                next.cursor = Cursor {
                    part: first.expect("nonempty ensemble"),
                    ..Cursor::default()
                };
                next.selection = None;
            }
            Command::RemovePart | Command::RemovePartById(_) => {
                let target = match command {
                    Command::RemovePartById(id) => id,
                    _ => next.cursor.part,
                };
                let index = next
                    .score
                    .parts()
                    .iter()
                    .position(|part| part.id() == target)
                    .ok_or(ScoreError("El instrumento no existe"))?;
                next.score.remove_part(target)?;
                if next.cursor.part == target {
                    let neighbor = next.score.parts()[index.min(next.score.parts().len() - 1)].id();
                    next.cursor = Cursor {
                        voice: Default::default(),
                        staff: score_core::StaffId::Upper,
                        part: neighbor,
                        measure: next.cursor.measure,
                        index: 0,
                    };
                    next.selection = None;
                }
            }
        }
        let remove_other =
            matches!(command, Command::RemovePartById(id) if id != self.cursor().part);
        let preserve_range = matches!(
            command,
            Command::EditForm(_)
                | Command::SaveComment { .. }
                | Command::SetCommentResolved(..)
                | Command::RemoveComment(_)
                | Command::SetInstrumentRange(_, _)
                | Command::SetPlayers(_, _)
                | Command::ToggleRehearsalMark
                | Command::ToggleLayoutBreak(_)
                | Command::SetRehearsalStyle(_)
                | Command::SetEnding(_)
                | Command::RemoveEndings
                | Command::SetDotted(_)
                | Command::SetTriplet(_)
                | Command::ToggleMark(
                    score_core::MarkKind::Harmonic(_) | score_core::MarkKind::Tremolo(_)
                )
                | Command::SetDuration(_)
                | Command::SetMarkOffset(_, _)
                | Command::MoveMark(..)
                | Command::SetBarMarkOffset(_, _)
                | Command::ResetBarMarkOffset(_)
                | Command::ResetMarkOffsets(_)
                | Command::Erase
                | Command::SetPartDivisi(_)
                | Command::SetPassageDivisi(_)
                | Command::ToggleMark(score_core::MarkKind::Division(_))
        ) || remove_other;
        if !preserve_range {
            next.range_anchor = None;
        }
        if !matches!(
            command,
            Command::SetPassageDivisi(_)
                | Command::SaveComment { .. }
                | Command::SetCommentResolved(..)
                | Command::RemoveComment(_)
                | Command::SetInstrumentRange(_, _)
                | Command::SetPlayers(_, _)
                | Command::ToggleMark(score_core::MarkKind::Division(_))
        ) {
            next.divisi_anchor = None;
        }
        next.score.prune_bar_offsets();
        next.score.validate()?;
        let entry_octave = self.octave;
        self.commit(next);
        if let Command::AddPitch(pitch) = command {
            self.octave = pitch.octave();
        }
        if remove_other {
            self.octave = entry_octave;
        } else if matches!(
            command,
            Command::AddPart(_)
                | Command::AddParts(_)
                | Command::RemovePart
                | Command::RemovePartById(_)
        ) {
            self.sync_octave();
        }
        Ok(true)
    }

    /// Like `execute`, but one undo removes this edit and the previous one,
    /// as when a slur follows each note typed after it.
    pub fn execute_joined(&mut self, command: Command) -> Result<bool, ScoreError> {
        let previous = self.undo.len();
        let changed = self.execute(command)?;
        if changed && previous > 0 {
            self.undo.pop();
        }
        Ok(changed)
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
                voice: Default::default(),
                staff,
                part,
                measure: self.cursor().measure,
                index: 0,
            });
            self.octave = self
                .score()
                .part(part)
                .expect("part")
                .entry_octave_at(staff, self.cursor().measure);
        }
    }

    pub fn select_part(&mut self, part: PartId) {
        if part == self.cursor().part {
            return;
        }
        if self.score().part(part).is_some() {
            self.set_cursor(Cursor {
                voice: Default::default(),
                staff: score_core::StaffId::Upper,
                part,
                measure: self.cursor().measure,
                index: 0,
            });
            self.octave = self
                .score()
                .part(part)
                .unwrap()
                .entry_octave_at(StaffId::Upper, self.cursor().measure);
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
                .entry_octave_at(self.cursor().staff, self.cursor().measure);
        }
    }

    /// Letter entry lands on the octave nearest the selected pitch.
    pub fn note(&self, step: score_core::Step) -> Event {
        let pitch = self
            .selected_pitch()
            .and_then(|selected| Pitch::new(selected.step, self.octave))
            .map(|reference| nearest_pitch(step, reference))
            .or_else(|| Pitch::new(step, self.octave));
        Event {
            notation: Default::default(),
            pitches: score_core::Pitches::single(pitch),
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

fn nearest_pitch(step: score_core::Step, reference: Pitch) -> Pitch {
    let r = reference.diatonic();
    let below = r - (r - step.index()).rem_euclid(7);
    let above = below + 7;
    let candidates = if below == r || r - below < above - r {
        [below, above]
    } else {
        [above, below]
    };
    candidates
        .into_iter()
        .find(|d| (0..=62).contains(d))
        .and_then(|d| Pitch::new(step, (d / 7) as u8))
        .unwrap_or(reference)
}

#[cfg(test)]
mod breaks_tests;
#[cfg(test)]
mod mvp_tests;
#[cfg(test)]
mod voice_tests;

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
        assert!(editor.execute(command.clone()).is_err());
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
            voice: Default::default(),
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
                voice: Default::default(),
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
                voice: Default::default(),
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
        assert_eq!(editor.octave, 4);
        assert_eq!(editor.note(Step::D).pitches.first(), Pitch::new(Step::D, 4));
        editor.redo();
        assert_eq!(editor.octave, 3);
        assert_eq!(editor.note(Step::D).pitches.first(), Pitch::new(Step::D, 4));
    }
}

#[cfg(test)]
mod expression_tests;

#[cfg(test)]
mod arrangement_tests;

#[cfg(test)]
mod clipboard_tests;

#[cfg(test)]
mod signature_tests;

impl Editor {
    pub fn select_display_staff(&mut self, staff: StaffId) {
        let cursor = self.cursor();
        let tick = self.divisi_position().tick;
        self.select_display_staff_at(
            staff,
            score_core::DivisiPoint::new(cursor.measure, tick),
            cursor.voice,
        );
    }
    /// Move to a displayed staff in another part at the same exact rhythmic point.
    pub fn select_display_staff_in(&mut self, part: PartId, staff: StaffId) {
        let at = self.divisi_position();
        let origin_voice = self.cursor().voice;
        if part == self.cursor().part {
            self.select_display_staff(staff);
            return;
        }
        let Some(destination) = self.score().part(part) else {
            return;
        };
        let (source, forced) = destination.input_source(staff, at.measure);
        let voice = forced.unwrap_or_else(|| {
            if destination.staff(source).unwrap().measures()[at.measure].voice_len(origin_voice) > 0
            {
                origin_voice
            } else {
                score_core::Voice::One
            }
        });
        self.select_part(part);
        self.select_display_staff_at(staff, at, voice);
    }
    fn select_display_staff_at(
        &mut self,
        staff: StaffId,
        point: score_core::DivisiPoint,
        preferred_voice: score_core::Voice,
    ) {
        let cursor = self.cursor();
        let part = self.score().part(cursor.part).unwrap();
        let (source, voice) = part.input_source(staff, point.measure);
        let voice = voice.unwrap_or(preferred_voice);
        let bar = &part.staff(source).unwrap().measures()[point.measure];
        let wanted = point.tick;
        let target = bar
            .timed_iter()
            .filter(|(v, _, _, _)| *v == voice)
            .filter_map(|(_, start, event, value)| {
                let at = if start <= wanted && wanted < start + value.duration.ticks() {
                    wanted
                } else {
                    start
                };
                (part.displayed_staff(source, voice, point.measure, at) == staff)
                    .then_some((event, at))
            })
            .min_by_key(|(_, at)| at.abs_diff(wanted))
            .map(|(event, at)| {
                (
                    ElementId {
                        part: cursor.part,
                        staff: source,
                        measure: point.measure,
                        event,
                    },
                    at,
                )
            });
        if let Some((id, at)) = target {
            self.select(id);
            self.select_divisi_position(score_core::DivisiPoint::new(point.measure, at));
        } else {
            self.set_cursor(Cursor {
                staff: source,
                voice,
                measure: point.measure,
                index: bar.voice_len(voice),
                ..cursor
            });
        }
    }
    pub fn divisi_position(&self) -> score_core::DivisiPoint {
        self.state.divisi_position()
    }
    /// Retain an exact position inside a held note for editing division boundaries.
    pub fn select_divisi_position(&mut self, point: score_core::DivisiPoint) {
        if let Some(id) = self.selection() {
            let start = (self.score().event_tick(id).unwrap()
                - self.score().measure_start_tick(id.measure)) as u16;
            let end = start + self.score().event(id).unwrap().duration.ticks();
            if point.measure == id.measure && start <= point.tick && point.tick < end {
                self.state.divisi_anchor = Some(point);
            }
        }
    }
}
impl Snapshot {
    fn divisi_position(&self) -> score_core::DivisiPoint {
        if let Some(point) = self.divisi_anchor {
            return point;
        }
        let tick = self.selection.map_or_else(
            || {
                self.score
                    .part(self.cursor.part)
                    .unwrap()
                    .staff(self.cursor.staff)
                    .unwrap()
                    .measures()[self.cursor.measure]
                    .voice_iter(self.cursor.voice)
                    .take(self.cursor.index)
                    .map(|(_, event)| event.duration.ticks())
                    .sum()
            },
            |id| {
                (self.score.event_tick(id).unwrap() - self.score.measure_start_tick(id.measure))
                    as u16
            },
        );
        if tick == self.score.time_at(self.cursor.measure).capacity()
            && self.cursor.measure + 1 < self.score.measures().len()
        {
            score_core::DivisiPoint::new(self.cursor.measure + 1, 0)
        } else {
            score_core::DivisiPoint::new(self.cursor.measure, tick)
        }
    }
    fn set_passage_divisi(&mut self, divided: bool, toggle: bool) -> Result<(), ScoreError> {
        let part = self.cursor.part;
        let focus = self.divisi_position();
        let (start, end) = if let Some(anchor) = self.range_anchor {
            let position = |id: ElementId| {
                score_core::DivisiPoint::new(
                    id.measure,
                    (self.score.event_tick(id).unwrap() - self.score.measure_start_tick(id.measure))
                        as u16,
                )
            };
            let a = position(anchor);
            let b = position(self.selection.unwrap());
            let last = if a > b {
                anchor
            } else {
                self.selection.unwrap()
            };
            let end = score_core::DivisiPoint::new(
                last.measure,
                position(last).tick + self.score.event(last).unwrap().duration.ticks(),
            );
            (a.min(b), Some(end))
        } else {
            (focus, None)
        };
        let requested = if divided {
            score_core::Division::Divisi
        } else {
            score_core::Division::Unison
        };
        if toggle
            && end.is_none()
            && self
                .score
                .part(part)
                .unwrap()
                .divisi_change_at_tick(start.measure, start.tick)
                == Some(requested)
        {
            return self.score.remove_divisi_change_at(part, start);
        }
        let was_lower = self.cursor.staff == StaffId::Lower;
        let remap = self.score.set_divisi_span(part, start, end, divided)?;
        self.selection = self
            .selection
            .map(|id| remap.get(&id).copied().unwrap_or(id));
        self.range_anchor = self
            .range_anchor
            .map(|id| remap.get(&id).copied().unwrap_or(id));
        if was_lower {
            self.cursor.staff = StaffId::Upper;
            self.cursor.voice = score_core::Voice::Two;
        }
        if let Some(id) = self.selection.filter(|id| {
            id.measure == start.measure
                && self.score.event_tick(*id).unwrap() - self.score.measure_start_tick(id.measure)
                    == u64::from(start.tick)
        }) {
            self.score.set_division(id, Some(requested))?;
        }
        if let Some(id) = self.selection {
            self.cursor.staff = id.staff;
            self.cursor.voice = self.score.voice_of(id).unwrap();
            self.cursor.measure = id.measure;
            self.cursor.index = self
                .score
                .part(part)
                .unwrap()
                .staff(id.staff)
                .unwrap()
                .measures()[id.measure]
                .voice_index_of(self.cursor.voice, id.event)
                .unwrap()
                + 1;
        }
        Ok(())
    }
}

#[cfg(test)]
mod harmonic_tests;

#[cfg(test)]
mod form_tests;

#[cfg(test)]
mod tremolo_tests;

#[cfg(test)]
mod rehearsal_tests;

#[cfg(test)]
mod offset_tests;
