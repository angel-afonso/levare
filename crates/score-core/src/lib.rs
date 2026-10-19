//! Musical data, independent of the UI, engraving and audio backends.
use imbl::Vector;
use serde::{Deserialize, Serialize};
use slotmap::{SlotMap, new_key_type};
use std::{fmt, sync::Arc};

mod dynamics;
pub use dynamics::{DynamicEnvelope, DynamicSegment};
mod expression;
pub use expression::{
    Articulation, Connection, ConnectionKind, Dynamic, Notation, PlayingTechnique,
};
mod marks;
pub use marks::MarkRef;
mod instrument;
pub use instrument::{Clef, Instrument, InstrumentFamily};
mod key_signature;
pub use key_signature::{KeyMode, KeySignature};
mod pitches;
pub use pitches::{MAX_CHORD_NOTES, Pitches};

// Channel 9 is percussion; channel 15 is reserved for entry audition.
pub const MAX_PARTS: usize = 14;
pub const MAX_STAVES: usize = MAX_PARTS * 2;
pub const MAX_SOUNDING_NOTES: usize = MAX_STAVES * MAX_CHORD_NOTES;

/// Identity within a part. Piano hands keep separate event stores and cursors.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum StaffId {
    #[default]
    Upper,
    Lower,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PartId(pub u32);

pub const MAX_MEASURES: usize = 4096;
/// Largest supported bar (16/1), filled with triplet sixteenths.
pub const MAX_EVENTS_PER_MEASURE: usize = (16 * TICKS_PER_WHOLE / Duration::MIN_TICKS) as usize;

new_key_type! { pub struct EventId; }

/// Keys are local to a measure; the pair stays stable while other events change.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ElementId {
    pub part: PartId,
    pub staff: StaffId,
    pub measure: usize,
    pub event: EventId,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Step {
    C,
    D,
    E,
    F,
    G,
    A,
    B,
}

impl Step {
    pub fn index(self) -> i16 {
        match self {
            Self::C => 0,
            Self::D => 1,
            Self::E => 2,
            Self::F => 3,
            Self::G => 4,
            Self::A => 5,
            Self::B => 6,
        }
    }

    pub fn from_letter(key: &str) -> Option<Self> {
        Some(match key {
            "c" => Self::C,
            "d" => Self::D,
            "e" => Self::E,
            "f" => Self::F,
            "g" => Self::G,
            "a" => Self::A,
            "b" => Self::B,
            _ => return None,
        })
    }
}

/// Explicit notation. An absent sign follows the score's key signature.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Accidental {
    Flat,
    Natural,
    Sharp,
}
impl Accidental {
    pub fn semitones(self) -> i8 {
        match self {
            Self::Flat => -1,
            Self::Natural => 0,
            Self::Sharp => 1,
        }
    }
    pub fn symbol(self) -> &'static str {
        match self {
            Self::Flat => "♭",
            Self::Natural => "♮",
            Self::Sharp => "♯",
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Flat => "Bemol",
            Self::Natural => "Becuadro",
            Self::Sharp => "Sostenido",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pitch {
    pub step: Step,
    octave: u8,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    accidental: Option<Accidental>,
}

impl Pitch {
    pub fn new(step: Step, octave: u8) -> Option<Self> {
        (octave <= 8).then_some(Self {
            step,
            octave,
            accidental: None,
        })
    }
    pub fn accidental(self) -> Option<Accidental> {
        self.accidental
    }
    pub fn with_accidental(self, accidental: Option<Accidental>) -> Self {
        Self { accidental, ..self }
    }
    /// Resolve the key on a copy, keeping explicit per-note overrides.
    pub fn in_key(self, key: KeySignature) -> Self {
        self.with_accidental(self.accidental.or_else(|| key.accidental(self.step)))
    }
    pub fn alteration(self) -> i8 {
        self.accidental.map_or(0, Accidental::semitones)
    }
    pub fn octave(self) -> u8 {
        self.octave
    }
    pub fn diatonic(self) -> i16 {
        i16::from(self.octave) * 7 + self.step.index()
    }
    pub fn shift(self, steps: i16) -> Option<Self> {
        let value = self.diatonic().checked_add(steps)?;
        if !(0..63).contains(&value) {
            return None;
        }
        let step = [
            Step::C,
            Step::D,
            Step::E,
            Step::F,
            Step::G,
            Step::A,
            Step::B,
        ][value as usize % 7];
        Self::new(step, (value / 7) as u8).map(|pitch| pitch.with_accidental(self.accidental))
    }
}

impl fmt::Display for Pitch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:?}{}{}",
            self.step,
            self.accidental.map_or("", Accidental::symbol),
            self.octave
        )
    }
}

mod duration;
pub use duration::{Duration, NoteValue, TICKS_PER_QUARTER, TICKS_PER_WHOLE};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TimeSignature {
    pub numerator: u8,
    pub denominator: u8,
}

impl Default for TimeSignature {
    fn default() -> Self {
        Self {
            numerator: 4,
            denominator: 4,
        }
    }
}
impl TimeSignature {
    pub fn capacity(self) -> u16 {
        u16::from(self.numerator) * TICKS_PER_WHOLE / u16::from(self.denominator.max(1))
    }
    pub fn is_valid(self) -> bool {
        (1..=16).contains(&self.numerator) && [1, 2, 4, 8, 16].contains(&self.denominator)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Event {
    #[serde(default)]
    pub notation: Notation,
    pub pitches: Pitches,
    pub duration: Duration,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Measure {
    events: Arc<SlotMap<EventId, Event>>,
    order: Vector<EventId>,
}

impl Default for Measure {
    fn default() -> Self {
        Self {
            events: Arc::new(SlotMap::with_key()),
            order: Vector::new(),
        }
    }
}

impl PartialEq for Measure {
    fn eq(&self, other: &Self) -> bool {
        self.iter().eq(other.iter())
    }
}
impl Eq for Measure {}

impl Measure {
    pub fn iter(&self) -> impl Iterator<Item = (EventId, &Event)> {
        self.order
            .iter()
            .filter_map(|id| self.events.get(*id).map(|event| (*id, event)))
    }
    pub fn len(&self) -> usize {
        self.order.len()
    }
    pub fn is_empty(&self) -> bool {
        self.order.is_empty()
    }
    pub fn used_ticks(&self) -> u16 {
        self.iter().map(|(_, event)| event.duration.ticks()).sum()
    }
    pub fn event(&self, id: EventId) -> Option<&Event> {
        self.events.get(id)
    }
    pub fn id_at(&self, index: usize) -> Option<EventId> {
        self.order.get(index).copied()
    }
    pub fn index_of(&self, id: EventId) -> Option<usize> {
        self.order.iter().position(|key| *key == id)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Staff {
    id: StaffId,
    measures: Vector<Measure>,
}
impl Staff {
    fn new(id: StaffId, measures: usize) -> Self {
        Self {
            id,
            measures: (0..measures).map(|_| Measure::default()).collect(),
        }
    }
    pub fn id(&self) -> StaffId {
        self.id
    }
    pub fn measures(&self) -> &Vector<Measure> {
        &self.measures
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Part {
    id: PartId,
    instrument: Instrument,
    staves: Vector<Staff>,
}
impl Part {
    fn new(id: PartId, instrument: Instrument, measures: usize) -> Self {
        let mut staves: Vector<_> = std::iter::once(Staff::new(StaffId::Upper, measures)).collect();
        if instrument == Instrument::Piano {
            staves.push_back(Staff::new(StaffId::Lower, measures));
        }
        Self {
            id,
            instrument,
            staves,
        }
    }
    pub fn id(&self) -> PartId {
        self.id
    }
    pub fn instrument(&self) -> Instrument {
        self.instrument
    }
    pub fn measures(&self) -> &Vector<Measure> {
        &self.staves[0].measures
    }
    pub fn staves(&self) -> &Vector<Staff> {
        &self.staves
    }
    pub fn staff(&self, id: StaffId) -> Option<&Staff> {
        self.staves.iter().find(|staff| staff.id == id)
    }
    fn staff_mut(&mut self, id: StaffId) -> Result<&mut Staff, ScoreError> {
        self.staves
            .iter_mut()
            .find(|staff| staff.id == id)
            .ok_or(ScoreError("El pentagrama no existe"))
    }
    pub fn clef(&self, staff: StaffId) -> Clef {
        if staff == StaffId::Lower {
            Clef::Bass
        } else {
            self.instrument.clef()
        }
    }
    pub fn entry_octave(&self, staff: StaffId) -> u8 {
        if staff == StaffId::Lower {
            3
        } else {
            self.instrument.entry_octave()
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Score {
    title: String,
    time: TimeSignature,
    #[serde(default)]
    key_signature: KeySignature,
    parts: Vector<Part>,
    next_part_id: u32,
    #[serde(default)]
    connections: Vector<Connection>,
}

impl Default for Score {
    fn default() -> Self {
        Self {
            title: "Sin título".into(),
            time: TimeSignature::default(),
            key_signature: KeySignature::default(),
            parts: std::iter::once(Part::new(PartId(0), Instrument::Piano, 4)).collect(),
            next_part_id: 1,
            connections: Vector::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScoreError(pub &'static str);
impl fmt::Display for ScoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0)
    }
}
impl std::error::Error for ScoreError {}

impl Score {
    pub fn title(&self) -> &str {
        &self.title
    }
    pub fn time(&self) -> TimeSignature {
        self.time
    }
    pub fn key_signature(&self) -> KeySignature {
        self.key_signature
    }
    pub fn set_key_signature(&mut self, key: KeySignature) {
        self.key_signature = key;
        self.prune_connections();
    }
    pub fn measures(&self) -> &Vector<Measure> {
        self.parts[0].measures()
    }
    pub fn parts(&self) -> &Vector<Part> {
        &self.parts
    }
    pub fn part(&self, id: PartId) -> Option<&Part> {
        self.parts.iter().find(|part| part.id == id)
    }
    fn part_mut(&mut self, id: PartId) -> Result<&mut Part, ScoreError> {
        self.parts
            .iter_mut()
            .find(|part| part.id == id)
            .ok_or(ScoreError("El instrumento no existe"))
    }
    pub fn add_part(&mut self, instrument: Instrument) -> Result<PartId, ScoreError> {
        if self.parts.len() >= MAX_PARTS {
            return Err(ScoreError("Máximo de 14 instrumentos por partitura"));
        }
        let next = self
            .next_part_id
            .checked_add(1)
            .ok_or(ScoreError("Identificadores de instrumento agotados"))?;
        let id = PartId(self.next_part_id);
        self.parts
            .push_back(Part::new(id, instrument, self.measures().len()));
        self.next_part_id = next;
        Ok(id)
    }
    /// Rearrange complete parts, preserving their IDs, notes, staves and links.
    pub fn arrange_parts(&mut self) -> bool {
        if self
            .parts
            .iter()
            .map(|part| part.instrument.score_order())
            .is_sorted()
        {
            return false;
        }
        let mut parts: Vec<_> = self.parts.iter().cloned().collect();
        parts.sort_by_key(|part| part.instrument.score_order());
        self.parts = parts.into_iter().collect();
        true
    }
    pub fn remove_part(&mut self, id: PartId) -> Result<(), ScoreError> {
        if self.parts.len() == 1 {
            return Err(ScoreError("La partitura debe conservar un instrumento"));
        }
        let index = self
            .parts
            .iter()
            .position(|part| part.id == id)
            .ok_or(ScoreError("El instrumento no existe"))?;
        self.parts.remove(index);
        self.prune_connections();
        Ok(())
    }
    pub fn event(&self, id: ElementId) -> Option<&Event> {
        self.part(id.part)?
            .staff(id.staff)?
            .measures
            .get(id.measure)?
            .event(id.event)
    }

    pub fn insert(
        &mut self,
        measure: usize,
        index: usize,
        event: Event,
    ) -> Result<ElementId, ScoreError> {
        self.insert_in(self.parts[0].id, measure, index, event)
    }

    pub fn insert_in(
        &mut self,
        part: PartId,
        measure: usize,
        index: usize,
        event: Event,
    ) -> Result<ElementId, ScoreError> {
        self.insert_on(part, StaffId::Upper, measure, index, event)
    }

    pub fn insert_on(
        &mut self,
        part: PartId,
        staff: StaffId,
        measure: usize,
        index: usize,
        event: Event,
    ) -> Result<ElementId, ScoreError> {
        let current = self
            .part(part)
            .ok_or(ScoreError("El instrumento no existe"))?
            .staff(staff)
            .ok_or(ScoreError("El pentagrama no existe"))?
            .measures
            .get(measure)
            .ok_or(ScoreError("El compás no existe"))?;
        if index > current.len() {
            return Err(ScoreError("Posición de entrada inválida"));
        }
        event.pitches.validate()?;
        if event.notation.technique.is_some()
            && self.part(part).unwrap().instrument().family() != InstrumentFamily::Strings
        {
            return Err(ScoreError(
                "Esta técnica requiere un instrumento de cuerda con arco",
            ));
        }
        if event.pitches.is_empty() && event.notation.articulation.is_some() {
            return Err(ScoreError("Un silencio no admite articulaciones"));
        }
        if current.used_ticks() + event.duration.ticks() > self.time.capacity() {
            return Err(ScoreError("La figura no cabe en el compás"));
        }
        let current = &mut self.part_mut(part)?.staff_mut(staff)?.measures[measure];
        let id = Arc::make_mut(&mut current.events).insert(event);
        current.order.insert(index, id);
        self.prune_connections();
        Ok(ElementId {
            part,
            staff,
            measure,
            event: id,
        })
    }

    pub fn remove(&mut self, id: ElementId) -> Result<(), ScoreError> {
        let measure = self
            .part_mut(id.part)?
            .staff_mut(id.staff)?
            .measures
            .get_mut(id.measure)
            .ok_or(ScoreError("El compás no existe"))?;
        let index = measure
            .index_of(id.event)
            .ok_or(ScoreError("La nota no existe"))?;
        Arc::make_mut(&mut measure.events).remove(id.event);
        measure.order.remove(index);
        self.prune_connections();
        Ok(())
    }

    fn set_pitches(&mut self, id: ElementId, pitches: Pitches) -> Result<(), ScoreError> {
        self.event(id).ok_or(ScoreError("La nota no existe"))?;
        Arc::make_mut(
            &mut self.part_mut(id.part)?.staff_mut(id.staff)?.measures[id.measure].events,
        )[id.event]
            .pitches = pitches;
        self.prune_connections();
        Ok(())
    }
    pub fn transpose(&mut self, id: ElementId, steps: i16) -> Result<(), ScoreError> {
        let pitches = self
            .event(id)
            .ok_or(ScoreError("La nota no existe"))?
            .pitches
            .transposed(steps)?;
        self.set_pitches(id, pitches)
    }
    pub fn add_pitch(&mut self, id: ElementId, pitch: Pitch) -> Result<(), ScoreError> {
        let event = self
            .event(id)
            .ok_or(ScoreError("Selecciona una nota para formar un acorde"))?;
        if event.pitches.is_empty() {
            return Err(ScoreError("Selecciona una nota, no un silencio"));
        }
        self.set_pitches(id, event.pitches.with(pitch)?)
    }
    pub fn set_accidental(
        &mut self,
        id: ElementId,
        index: usize,
        accidental: Option<Accidental>,
    ) -> Result<(), ScoreError> {
        let pitches = self
            .event(id)
            .ok_or(ScoreError("Selecciona una nota"))?
            .pitches;
        self.set_pitches(id, pitches.with_accidental(index, accidental)?)
    }
    pub fn remove_highest_pitch(&mut self, id: ElementId) -> Result<(), ScoreError> {
        let pitches = self
            .event(id)
            .ok_or(ScoreError("Selecciona un acorde"))?
            .pitches
            .without_highest()?;
        self.set_pitches(id, pitches)
    }

    pub fn append_measure(&mut self) -> Result<(), ScoreError> {
        if self.measures().len() >= MAX_MEASURES {
            return Err(ScoreError(
                "Se alcanzó el límite de compases de esta versión",
            ));
        }
        for part in &mut self.parts {
            for staff in &mut part.staves {
                staff.measures.push_back(Measure::default());
            }
        }
        Ok(())
    }

    /// Check deserialized data before it reaches editing or layout.
    pub fn validate(&self) -> Result<(), ScoreError> {
        if !self.time.is_valid() {
            return Err(ScoreError("Indicación de compás inválida"));
        }
        if self.parts.is_empty() || self.parts.len() > MAX_PARTS {
            return Err(ScoreError("Cantidad de instrumentos inválida"));
        }
        for part in &self.parts {
            let expected = if part.instrument == Instrument::Piano {
                2
            } else {
                1
            };
            if part.staves.len() != expected
                || part.staves[0].id != StaffId::Upper
                || (expected == 2 && part.staves[1].id != StaffId::Lower)
            {
                return Err(ScoreError("Estructura de pentagramas inválida"));
            }
        }
        if self.measures().is_empty() || self.measures().len() > MAX_MEASURES {
            return Err(ScoreError("Cantidad de compases inválida"));
        }
        if self.title.len() > 1024 {
            return Err(ScoreError("Título demasiado largo"));
        }
        let mut parts_seen = std::collections::HashSet::new();
        for part in &self.parts {
            if !parts_seen.insert(part.id) || part.id.0 >= self.next_part_id {
                return Err(ScoreError("Estructura de instrumentos inválida"));
            }
            for staff in &part.staves {
                if staff.measures.len() != self.measures().len() {
                    return Err(ScoreError("Compases desalineados entre pentagramas"));
                }
                for measure in &staff.measures {
                    if measure.order.len() != measure.events.len()
                        || measure.len() > MAX_EVENTS_PER_MEASURE
                    {
                        return Err(ScoreError("Índice de notas inválido"));
                    }
                    let mut seen = std::collections::HashSet::new();
                    let mut ticks = 0u32;
                    for id in &measure.order {
                        let event = measure
                            .events
                            .get(*id)
                            .ok_or(ScoreError("Referencia a nota inexistente"))?;
                        if !seen.insert(*id) {
                            return Err(ScoreError("Referencia a nota duplicada"));
                        }
                        event.pitches.validate()?;
                        if event.notation.technique.is_some()
                            && part.instrument().family() != InstrumentFamily::Strings
                        {
                            return Err(ScoreError(
                                "Técnica de cuerda en instrumento incompatible",
                            ));
                        }
                        if event.pitches.is_empty() && event.notation.articulation.is_some() {
                            return Err(ScoreError("Articulación en silencio"));
                        }
                        ticks += u32::from(event.duration.ticks());
                    }
                    if ticks > u32::from(self.time.capacity()) {
                        return Err(ScoreError("Compás excedido"));
                    }
                }
            }
        }
        self.validate_connections()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn note() -> Event {
        Event {
            notation: Default::default(),
            pitches: Pitches::single(Pitch::new(Step::C, 4)),
            duration: Duration::Quarter,
        }
    }

    #[test]
    fn accidentals_preserve_spelling_through_transposition_and_chord_edits() {
        let sharp = Pitch::new(Step::C, 4)
            .unwrap()
            .with_accidental(Some(Accidental::Sharp));
        let flat = Pitch::new(Step::D, 4)
            .unwrap()
            .with_accidental(Some(Accidental::Flat));
        assert_eq!(sharp.to_string(), "C♯4");
        assert_eq!(flat.alteration(), -1);
        assert_eq!(sharp.shift(7).unwrap().to_string(), "C♯5");
        let pitches = Pitches::new(&[sharp, flat]).unwrap();
        assert!(pitches.with(Pitch::new(Step::C, 4).unwrap()).is_err());
        let mut score = Score::default();
        let id = score
            .insert(
                0,
                0,
                Event {
                    notation: Default::default(),
                    pitches,
                    duration: Duration::Quarter,
                },
            )
            .unwrap();
        let before = score.clone();
        score
            .set_accidental(id, 1, Some(Accidental::Natural))
            .unwrap();
        let changed = score.event(id).unwrap().pitches;
        assert_eq!(changed.first(), Some(sharp));
        assert_eq!(changed.last().unwrap().to_string(), "D♮4");
        assert_eq!(before.event(id).unwrap().pitches.last(), Some(flat));
        let before = score.clone();
        assert!(score.set_accidental(id, 8, None).is_err());
        assert_eq!(before, score);
        score.validate().unwrap();
    }

    #[test]
    fn chords_are_bounded_unique_and_do_not_consume_extra_time() {
        let mut score = Score::default();
        let id = score.insert(0, 0, note()).unwrap();
        let original = score.clone();
        for step in [Step::G, Step::E] {
            score.add_pitch(id, Pitch::new(step, 4).unwrap()).unwrap();
        }
        assert_eq!(
            score
                .event(id)
                .unwrap()
                .pitches
                .iter()
                .map(|pitch| pitch.step)
                .collect::<Vec<_>>(),
            vec![Step::C, Step::E, Step::G]
        );
        assert_eq!(score.measures()[0].used_ticks(), Duration::Quarter.ticks());
        assert_eq!(original.event(id).unwrap().pitches.len(), 1);
        let before = score.clone();
        assert!(
            score
                .add_pitch(id, Pitch::new(Step::E, 4).unwrap())
                .is_err()
        );
        assert!(score.transpose(id, 40).is_err());
        assert_eq!(score, before);
        score.transpose(id, 7).unwrap();
        assert!(
            score
                .event(id)
                .unwrap()
                .pitches
                .iter()
                .all(|pitch| pitch.octave() == 5)
        );
        score.remove_highest_pitch(id).unwrap();
        assert_eq!(score.event(id).unwrap().pitches.len(), 2);
        let pitches: Vec<_> = (0..MAX_CHORD_NOTES)
            .map(|index| Pitch::new(Step::C, index as u8).unwrap())
            .collect();
        let full = Pitches::new(&pitches).unwrap();
        assert_eq!(full.len(), MAX_CHORD_NOTES);
        assert!(full.with(Pitch::new(Step::C, 8).unwrap()).is_err());
        score.validate().unwrap();
    }

    #[test]
    fn piano_hands_have_independent_notes_and_all_staves_grow_together() {
        let mut score = Score::default();
        let piano = score.parts()[0].id();
        assert_eq!(score.parts()[0].staves().len(), 2);
        assert_eq!(score.parts()[0].clef(StaffId::Upper), Clef::Treble);
        assert_eq!(score.parts()[0].clef(StaffId::Lower), Clef::Bass);
        let right = score
            .insert_on(piano, StaffId::Upper, 0, 0, note())
            .unwrap();
        let left = score
            .insert_on(piano, StaffId::Lower, 0, 0, note())
            .unwrap();
        assert_eq!(right.event, left.event);
        assert_ne!(right, left);
        let snapshot = score.clone();
        score.transpose(left, -7).unwrap();
        assert_eq!(score.event(right), snapshot.event(right));
        assert_ne!(score.event(left), snapshot.event(left));
        score.remove(left).unwrap();
        assert!(score.event(right).is_some());
        assert!(snapshot.event(left).is_some());
        let flute = score.add_part(Instrument::Flute).unwrap();
        assert!(
            score
                .insert_on(flute, StaffId::Lower, 0, 0, note())
                .is_err()
        );
        score.append_measure().unwrap();
        assert!(
            score
                .parts()
                .iter()
                .flat_map(|part| part.staves())
                .all(|staff| staff.measures().len() == 5)
        );
        score.validate().unwrap();
    }

    #[test]
    fn parts_keep_distinct_stable_notes_and_shared_bar_count() {
        let mut score = Score::default();
        let piano = score.parts()[0].id();
        let flute = score.add_part(Instrument::Flute).unwrap();
        let a = score.insert_in(piano, 0, 0, note()).unwrap();
        let b = score.insert_in(flute, 0, 0, note()).unwrap();
        assert_eq!(a.event, b.event); // Slot keys are local; the part disambiguates them.
        assert_ne!(a, b);
        let snapshot = score.clone();
        score.transpose(b, 7).unwrap();
        assert_eq!(score.event(a), snapshot.event(a));
        assert_ne!(score.event(b), snapshot.event(b));
        score.append_measure().unwrap();
        assert!(score.parts().iter().all(|part| part.measures().len() == 5));
        score.remove_part(piano).unwrap();
        assert!(score.event(b).is_some());
        assert!(score.event(a).is_none());
        assert!(score.remove_part(flute).is_err());
        let replacement = score.add_part(Instrument::Piano).unwrap();
        assert_ne!(replacement, piano);
        score.validate().unwrap();
        for _ in score.parts().len()..MAX_PARTS {
            score.add_part(Instrument::Violin).unwrap();
        }
        let before = score.clone();
        assert!(score.add_part(Instrument::Cello).is_err());
        assert_eq!(score, before);
    }

    #[test]
    fn capacity_and_stable_ids_survive_removal() {
        let mut score = Score::default();
        let first = score.insert(0, 0, note()).unwrap();
        let second = score.insert(0, 1, note()).unwrap();
        score.insert(0, 2, note()).unwrap();
        score.insert(0, 3, note()).unwrap();
        assert!(score.insert(0, 4, note()).is_err());
        score.remove(first).unwrap();
        assert_eq!(score.event(second), Some(&note()));
        assert!(score.event(first).is_none());
        let replacement = score.insert(0, 0, note()).unwrap();
        assert_ne!(first, replacement);
        score.validate().unwrap();
    }

    #[test]
    fn snapshots_are_isolated_and_pitch_crosses_octaves() {
        let mut score = Score::default();
        let id = score.insert(0, 0, note()).unwrap();
        let snapshot = score.clone();
        score.transpose(id, -1).unwrap();
        assert_eq!(
            score.event(id).unwrap().pitches.first(),
            Pitch::new(Step::B, 3)
        );
        assert_eq!(
            snapshot.event(id).unwrap().pitches.first(),
            Pitch::new(Step::C, 4)
        );
        assert!(Pitch::new(Step::C, 0).unwrap().shift(-1).is_none());
        assert!(Pitch::new(Step::B, 8).unwrap().shift(1).is_none());
    }
}

#[cfg(test)]
mod arrangement_tests;
