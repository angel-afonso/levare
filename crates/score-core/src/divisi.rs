//! Lossless conversion between two voices and a divided instrumental part.
use super::*;
use std::collections::HashMap;
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Division {
    Divisi,
    Unison,
}
impl Division {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Divisi => "div.",
            Self::Unison => "unis.",
        }
    }
}
impl Score {
    pub fn set_division(
        &mut self,
        id: ElementId,
        mark: Option<Division>,
    ) -> Result<(), ScoreError> {
        self.event(id)
            .ok_or(ScoreError("Selecciona una nota o silencio"))?;
        let tick = (self.event_tick(id).unwrap() - self.measure_start_tick(id.measure)) as u16;
        let part = self.part_mut(id.part)?;
        if !part.instrument.is_grand_staff() && !part.divided {
            if let Some(mark) = mark {
                part.divisi_changes
                    .entry(id.measure)
                    .or_default()
                    .insert(tick, mark == Division::Divisi);
            } else {
                part.remove_divisi_boundary(id.measure, tick);
            }
            for staff in &mut part.staves {
                for bar in &mut staff.measures {
                    for (_, event) in Arc::make_mut(&mut bar.events).iter_mut() {
                        event.notation.division = None;
                    }
                }
            }
        }

        Arc::make_mut(
            &mut self.part_mut(id.part)?.staff_mut(id.staff)?.measures[id.measure].events,
        )[id.event]
            .notation
            .division = mark;
        Ok(())
    }
}
impl Score {
    pub fn set_part_divisi(
        &mut self,
        part: PartId,
        divided: bool,
    ) -> Result<HashMap<ElementId, ElementId>, ScoreError> {
        let current = self
            .part(part)
            .ok_or(ScoreError("El instrumento no existe"))?;
        if current.instrument().is_grand_staff() {
            return Err(ScoreError("Este instrumento ya usa dos pentagramas"));
        }
        if current.divided == divided && current.divisi_changes.is_empty() {
            return Ok(HashMap::new());
        }
        let mut next = self.clone();
        let mut remap = HashMap::new();
        let current = next.part_mut(part)?;
        current.divisi_changes.clear();
        if divided && !current.divided {
            let mut lower = Staff::new(StaffId::Lower, 0);
            for (bar, upper) in current.staves[0].measures.iter_mut().enumerate() {
                let order = std::mem::take(&mut upper.second_order);
                let mut extracted = Measure {
                    events: upper.events.clone(),
                    order: order.clone(),
                    second_order: Vector::new(),
                };
                Arc::make_mut(&mut extracted.events).retain(|id, _| order.contains(&id));
                Arc::make_mut(&mut upper.events).retain(|id, _| !order.contains(&id));
                for event in order {
                    let old = ElementId {
                        part,
                        staff: StaffId::Upper,
                        measure: bar,
                        event,
                    };
                    remap.insert(
                        old,
                        ElementId {
                            staff: StaffId::Lower,
                            ..old
                        },
                    );
                }
                lower.measures.push_back(extracted);
            }
            current.staves.push_back(lower);
        } else if !divided && current.divided {
            let lower = current.staves[1].clone();
            for (bar, source) in lower.measures.iter().enumerate() {
                let target = &mut current.staves[0].measures[bar];
                if source.has_two_voices() || (!source.is_empty() && target.has_two_voices()) {
                    return Err(ScoreError(
                        "No se pueden reunir más de dos voces. Vacía una voz antes de reunir los pentagramas",
                    ));
                }
                for (event, value) in source.iter() {
                    let new = Arc::make_mut(&mut target.events).insert(*value);
                    target.second_order.push_back(new);
                    remap.insert(
                        ElementId {
                            part,
                            staff: StaffId::Lower,
                            measure: bar,
                            event,
                        },
                        ElementId {
                            part,
                            staff: StaffId::Upper,
                            measure: bar,
                            event: new,
                        },
                    );
                }
            }
            current.staves.pop_back();
        }
        current.divided = divided;
        next.remap_comments(|old| Some(remap.get(&old).copied().unwrap_or(old)));
        for mark in &mut next.connections {
            mark.start = remap.get(&mark.start).copied().unwrap_or(mark.start);
            mark.end = remap.get(&mark.end).copied().unwrap_or(mark.end);
        }
        next.prune_connections();
        next.validate()?;
        *self = next;
        Ok(remap)
    }
}

/// A local rhythmic position; ticks use the same integer grid as note durations.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct DivisiPoint {
    pub measure: usize,
    pub tick: u16,
}
impl DivisiPoint {
    pub const fn new(measure: usize, tick: u16) -> Self {
        Self { measure, tick }
    }
}
impl Part {
    pub fn divisi_at(&self, bar: usize) -> bool {
        self.divisi_at_tick(bar, 0)
    }
    pub fn divisi_at_tick(&self, bar: usize, tick: u16) -> bool {
        self.divisi_changes
            .get(&bar)
            .and_then(|map| map.range(..=tick).next_back())
            .map(|(_, v)| *v)
            .or_else(|| {
                self.divisi_changes
                    .range(..bar)
                    .next_back()
                    .and_then(|(_, m)| m.last_key_value())
                    .map(|(_, v)| *v)
            })
            .unwrap_or(self.divided)
    }
    pub fn divisi_change_at(&self, bar: usize) -> Option<Division> {
        self.divisi_change_at_tick(bar, 0)
    }
    pub fn divisi_change_at_tick(&self, bar: usize, tick: u16) -> Option<Division> {
        self.divisi_changes.get(&bar)?.get(&tick).map(|v| {
            if *v {
                Division::Divisi
            } else {
                Division::Unison
            }
        })
    }
    pub fn divisi_changes_in(&self, bar: usize) -> impl Iterator<Item = (u16, Division)> + '_ {
        self.divisi_changes.get(&bar).into_iter().flat_map(|m| {
            m.iter().map(|(&tick, &v)| {
                (
                    tick,
                    if v {
                        Division::Divisi
                    } else {
                        Division::Unison
                    },
                )
            })
        })
    }
    pub fn has_passage_divisi(&self) -> bool {
        !self.divisi_changes.is_empty()
    }
    pub fn bar_has_divisi(&self, bar: usize) -> bool {
        self.divisi_at(bar)
            || self
                .divisi_changes_in(bar)
                .any(|(_, v)| v == Division::Divisi)
    }
    pub fn bar_fully_divisi(&self, bar: usize) -> bool {
        self.divisi_at(bar)
            && self
                .divisi_changes_in(bar)
                .all(|(_, v)| v == Division::Divisi)
    }
    pub fn divisi_intervals(&self, bar: usize, capacity: u16) -> Vec<(u16, u16)> {
        let mut result = Vec::new();
        let mut start = 0;
        let mut active = self.divisi_at(bar);
        for (tick, mark) in self.divisi_changes_in(bar).filter(|(tick, _)| *tick > 0) {
            if active {
                result.push((start, tick));
            }
            active = mark == Division::Divisi;
            start = tick;
        }
        if active && start < capacity {
            result.push((start, capacity));
        }
        result
    }
    pub fn display_staves(&self, bar: usize) -> impl Iterator<Item = StaffId> {
        std::iter::once(StaffId::Upper).chain(
            (self.instrument.is_grand_staff() || self.bar_has_divisi(bar))
                .then_some(StaffId::Lower),
        )
    }
    pub fn display_source(&self, staff: StaffId, bar: usize) -> (StaffId, Option<Voice>) {
        if !self.divided && !self.instrument.is_grand_staff() && self.bar_has_divisi(bar) {
            (
                StaffId::Upper,
                if staff == StaffId::Lower {
                    Some(Voice::Two)
                } else {
                    Some(Voice::One)
                },
            )
        } else {
            (staff, None)
        }
    }
    pub fn displayed_staff(
        &self,
        source: StaffId,
        voice: Voice,
        bar: usize,
        _tick: u16,
    ) -> StaffId {
        if self.instrument.is_grand_staff() || self.divided {
            source
        } else if voice == Voice::Two && self.bar_has_divisi(bar) {
            StaffId::Lower
        } else {
            StaffId::Upper
        }
    }
    pub fn input_source(&self, staff: StaffId, bar: usize) -> (StaffId, Option<Voice>) {
        if !self.divided && !self.instrument.is_grand_staff() && self.bar_has_divisi(bar) {
            (
                StaffId::Upper,
                Some(if staff == StaffId::Lower {
                    Voice::Two
                } else {
                    Voice::One
                }),
            )
        } else {
            (staff, None)
        }
    }
    /// Hidden events keep time as silent spacers, but receive no ink or hit targets.
    pub fn hidden_display_events(
        &self,
        staff: StaffId,
        bar: usize,
    ) -> std::collections::HashSet<EventId> {
        if self.instrument.is_grand_staff() || self.divided {
            return Default::default();
        }
        self.staves[0].measures[bar]
            .timed_iter()
            .filter(|(voice, _, _, _)| {
                *voice == Voice::Two && (self.bar_has_divisi(bar) != (staff == StaffId::Lower))
            })
            .map(|(_, _, id, _)| id)
            .collect()
    }
    pub fn display_measure(&self, staff: StaffId, bar: usize) -> Measure {
        let (source, voice) = self.display_source(staff, bar);
        let original = &self.staff(source).expect("displayed source staff").measures[bar];
        let mut measure = original.clone();
        if let Some(voice) = voice {
            measure.order = original.voice_order(voice).clone();
            measure.second_order.clear();
            let order = measure.order.clone();
            Arc::make_mut(&mut measure.events).retain(|id, _| order.contains(&id));
        }
        if self.has_passage_divisi() {
            let hidden = self.hidden_display_events(staff, bar);
            for (id, event) in Arc::make_mut(&mut measure.events).iter_mut() {
                event.notation.division = None;
                if hidden.contains(&id) {
                    event.pitches = Pitches::default();
                    event.notation = Notation::default();
                }
            }
        }
        measure
    }
    fn remove_divisi_boundary(&mut self, bar: usize, tick: u16) {
        if let Some(map) = self.divisi_changes.get_mut(&bar) {
            map.remove(&tick);
            if map.is_empty() {
                self.divisi_changes.remove(&bar);
            }
        }
    }
}
impl Score {
    fn normalize_divisi_point(
        &self,
        point: DivisiPoint,
        allow_end: bool,
    ) -> Result<DivisiPoint, ScoreError> {
        if point.measure == self.measures().len() && point.tick == 0 && allow_end {
            return Ok(point);
        }
        if point.measure >= self.measures().len()
            || point.tick > self.time_at(point.measure).capacity()
        {
            return Err(ScoreError("Posición de divisi inválida"));
        }
        let point = if point.tick == self.time_at(point.measure).capacity() {
            DivisiPoint::new(point.measure + 1, 0)
        } else {
            point
        };
        if point.measure == self.measures().len() && !allow_end {
            return Err(ScoreError("Selecciona una posición dentro de la pieza"));
        }
        Ok(point)
    }
    /// Start a boundary or change a half-open rhythmic passage, restoring its prior state at the end.
    pub fn set_divisi_span(
        &mut self,
        part: PartId,
        start: DivisiPoint,
        end: Option<DivisiPoint>,
        divided: bool,
    ) -> Result<HashMap<ElementId, ElementId>, ScoreError> {
        let start = self.normalize_divisi_point(start, false)?;
        let end = end
            .map(|p| self.normalize_divisi_point(p, true))
            .transpose()?;
        if end.is_some_and(|end| end <= start) {
            return Err(ScoreError("Rango de divisi inválido"));
        }
        let current = self
            .part(part)
            .ok_or(ScoreError("El instrumento no existe"))?;
        if current.instrument.is_grand_staff() {
            return Err(ScoreError("Este instrumento conserva sus dos pentagramas"));
        }
        let mut next = self.clone();
        let legacy = current.divided;
        let remap = if legacy {
            next.set_part_divisi(part, false)?
        } else {
            HashMap::new()
        };
        let len = next.measures().len();
        let current = next.part_mut(part)?;
        for staff in &mut current.staves {
            for bar in &mut staff.measures {
                for (_, event) in Arc::make_mut(&mut bar.events).iter_mut() {
                    event.notation.division = None;
                }
            }
        }
        if legacy {
            current.divisi_changes.entry(0).or_default().insert(0, true);
        }
        if let Some(end) = end {
            let after = current.divisi_at_tick(end.measure, end.tick);
            for (&bar, map) in current.divisi_changes.iter_mut() {
                map.retain(|tick, _| {
                    let point = DivisiPoint::new(bar, *tick);
                    point < start || point >= end
                });
            }
            current.divisi_changes.retain(|_, map| !map.is_empty());
            if end.measure < len {
                current
                    .divisi_changes
                    .entry(end.measure)
                    .or_default()
                    .insert(end.tick, after);
            }
        }
        current
            .divisi_changes
            .entry(start.measure)
            .or_default()
            .insert(start.tick, divided);
        next.validate()?;
        *self = next;
        Ok(remap)
    }
    /// Compatibility wrapper for callers choosing complete inclusive bars.
    pub fn set_passage_divisi(
        &mut self,
        part: PartId,
        start: usize,
        end: Option<usize>,
        divided: bool,
    ) -> Result<HashMap<ElementId, ElementId>, ScoreError> {
        if end.is_some_and(|end| end < start || end >= self.measures().len()) {
            return Err(ScoreError("Rango de divisi inválido"));
        }
        self.set_divisi_span(
            part,
            DivisiPoint::new(start, 0),
            end.map(|end| DivisiPoint::new(end + 1, 0)),
            divided,
        )
    }
    pub fn remove_divisi_change(&mut self, part: PartId, bar: usize) -> Result<(), ScoreError> {
        self.remove_divisi_change_at(part, DivisiPoint::new(bar, 0))
    }
    pub fn remove_divisi_change_at(
        &mut self,
        part: PartId,
        point: DivisiPoint,
    ) -> Result<(), ScoreError> {
        let point = self.normalize_divisi_point(point, false)?;
        let current = self.part_mut(part)?;
        current.remove_divisi_boundary(point.measure, point.tick);
        for staff in &mut current.staves {
            for bar in &mut staff.measures {
                for (_, event) in Arc::make_mut(&mut bar.events).iter_mut() {
                    event.notation.division = None;
                }
            }
        }
        Ok(())
    }
}

pub(super) fn divisi_rhythm_pieces(ticks: u16) -> Result<Vec<Duration>, ScoreError> {
    static PIECES: std::sync::OnceLock<Vec<Option<Vec<Duration>>>> = std::sync::OnceLock::new();
    let pieces = PIECES.get_or_init(|| {
        let mut durations = Duration::all_modified();
        durations.sort_by_key(|d| (std::cmp::Reverse(d.ticks()), d.is_triplet(), d.is_dotted()));
        let mut result = vec![None; 289];
        result[0] = Some(Vec::new());
        for tick in 1..result.len() {
            let mut best: Option<Vec<Duration>> = None;
            for &d in &durations {
                let length = usize::from(d.ticks());
                if length <= tick
                    && let Some(rest) = &result[tick - length]
                    && best.as_ref().is_none_or(|b| b.len() > rest.len() + 1)
                {
                    let mut parts = vec![d];
                    parts.extend(rest);
                    best = Some(parts);
                }
            }
            result[tick] = best;
        }
        result
    });
    pieces
        .get(usize::from(ticks))
        .cloned()
        .flatten()
        .ok_or(ScoreError(
            "Este punto divide una figura en valores aún no admitidos. Elige otra posición rítmica",
        ))
}
impl Part {
    pub(super) fn validate_divisi_cuts(&self) -> Result<(), ScoreError> {
        if self.divided || self.instrument.is_grand_staff() {
            return Ok(());
        }
        for (&bar, changes) in &self.divisi_changes {
            for (voice, tick, _, event) in self.measures()[bar].timed_iter() {
                if voice != Voice::Two {
                    continue;
                }
                let end = tick + event.duration.ticks();
                let mut at = tick;
                for (&boundary, _) in changes.range((
                    std::ops::Bound::Excluded(tick),
                    std::ops::Bound::Excluded(end),
                )) {
                    divisi_rhythm_pieces(boundary - at)?;
                    at = boundary;
                }
                if at != tick {
                    divisi_rhythm_pieces(end - at)?;
                }
            }
        }
        Ok(())
    }
}
