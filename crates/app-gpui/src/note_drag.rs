//! Tentative pitch changes stay outside the document until the pointer is released.
use gpui::{Pixels, Point, px};
use score_core::{ElementId, Pitch, Score};
use score_layout::LayoutEngine;

pub(super) struct NoteDrag {
    pub id: ElementId,
    pub original: Pitch,
    base: Score,
    tone: usize,
    start: Point<Pixels>,
    origin: Point<Pixels>,
    scale: f32,
    pub active: bool,
    steps: Option<i16>,
    pub target: Option<Pitch>,
    pub preview: Option<Score>,
    pub error: Option<String>,
    pub layout: LayoutEngine,
    pub geometry: score_layout::Layout,
    mode: Motion,
    pub time_target: Option<TimeTarget>,
    last_time: Option<(usize, u16)>,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct TimeTarget {
    pub measure: usize,
    pub tick: u16,
    pub swap: bool,
    pub duration: score_core::Duration,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub(super) enum Motion {
    Pending,
    Time,
    Pitch,
}

impl NoteDrag {
    pub fn new(
        score: &Score,
        id: ElementId,
        pitch: Pitch,
        start: Point<Pixels>,
        origin: Point<Pixels>,
        geometry: &score_layout::Layout,
    ) -> Option<Self> {
        let tone = score.event(id)?.pitches.iter().position(|p| p == pitch)?;
        Some(Self {
            id,
            original: pitch,
            base: score.clone(),
            tone,
            start,
            origin,
            scale: geometry.scale,
            active: false,
            steps: None,
            target: None,
            preview: None,
            error: None,
            layout: LayoutEngine::default(),
            geometry: geometry.clone(),
            mode: Motion::Pending,
            time_target: None,
            last_time: None,
        })
    }
    pub fn motion(&self, position: Point<Pixels>) -> Motion {
        if self.active {
            return self.mode;
        }
        let delta = position - self.start;
        if delta.x.abs().max(delta.y.abs()) < px(5.) {
            Motion::Pending
        } else if delta.y.abs() >= delta.x.abs() {
            Motion::Pitch
        } else {
            Motion::Time
        }
    }
    pub fn update(&mut self, position: Point<Pixels>, origin: Point<Pixels>) -> bool {
        self.active = true;
        self.mode = Motion::Pitch;
        // Compare canvas positions, so scrolling never drags the note away from
        // the pointer. Pitch is relative to the grabbed head, not a new staff.
        let delta = (position - origin) - (self.start - self.origin);
        let steps = (-f32::from(delta.y) / (score_layout::STAFF_SPACE / 2. * self.scale)).round();
        let steps = (steps as i16).clamp(-self.original.diatonic(), 62 - self.original.diatonic());
        if self.steps == Some(steps) {
            return false;
        }
        self.steps = Some(steps);
        self.target = None;
        self.preview = None;
        self.error = None;
        let pitch = self.original.shift(steps).expect("bounded pitch");
        if pitch == self.original {
            return true;
        }
        let mut score = self.base.clone();
        match score
            .replace_pitch(self.id, self.tone, pitch)
            .and_then(|()| score.validate())
        {
            Ok(()) => {
                self.target = Some(pitch);
                self.preview = Some(score);
            }
            Err(error) => self.error = Some(error.to_string()),
        }
        true
    }

    pub fn is_time(&self) -> bool {
        self.mode == Motion::Time
    }

    pub fn update_time(&mut self, position: Point<Pixels>, origin: Point<Pixels>) -> bool {
        self.active = true;
        self.mode = Motion::Time;
        let destination = self.time_at(position, origin);
        if destination == self.last_time && (destination.is_some() || self.error.is_some()) {
            return false;
        }
        self.last_time = destination;
        self.time_target = None;
        self.error = None;
        let Some((measure, tick)) = destination else {
            self.error = Some("Suelta sobre el pentagrama de la misma voz e instrumento".into());
            return true;
        };
        let source_tick = self.base.event_tick(self.id).unwrap();
        if source_tick == self.base.measure_start_tick(measure) + u64::from(tick) {
            return true;
        }
        let mut candidate = self.base.clone();
        match candidate.move_event(self.id, measure, tick) {
            Ok(_) => {
                let voice = self.base.voice_of(self.id).unwrap();
                let swap = self
                    .base
                    .part(self.id.part)
                    .unwrap()
                    .staff(self.id.staff)
                    .unwrap()
                    .measures()[measure]
                    .timed_iter()
                    .any(|(v, at, _, event)| v == voice && at == tick && !event.pitches.is_empty());
                self.time_target = Some(TimeTarget {
                    measure,
                    tick,
                    swap,
                    duration: self.base.event(self.id).unwrap().duration,
                });
            }
            Err(error) => self.error = Some(error.to_string()),
        }
        true
    }

    /// Keep the grabbed rhythmic offset and pitch row while snapping time to
    /// sixteenths (triplet sixteenths for tuplets) or an existing voice onset.
    fn time_at(&self, position: Point<Pixels>, origin: Point<Pixels>) -> Option<(usize, u16)> {
        let voice = self.base.voice_of(self.id)?;
        let source = self.geometry.event_measure(self.id)?;
        let head = source
            .content
            .events
            .iter()
            .find(|e| e.id == self.id && e.pitch == Some(self.original))?;
        let point = position - origin;
        let x = f32::from(point.x) / self.scale;
        let y = f32::from(point.y) / self.scale;
        let grabbed = f32::from((self.start - self.origin).x) / self.scale;
        let offset = grabbed - source.origin.x - source.content.x_at_tick(f64::from(head.tick));
        let measure = self
            .geometry
            .measures
            .iter()
            .filter(|m| {
                m.part == self.id.part
                    && m.source_staff == self.id.staff
                    && m.regions.iter().any(|r| r.voice == voice)
                    && x >= m.origin.x
                    && x <= m.origin.x + m.width
                    && (y - m.origin.y - head.center.y).abs() <= 80.
            })
            .min_by(|a, b| {
                (y - a.origin.y - head.center.y)
                    .abs()
                    .total_cmp(&(y - b.origin.y - head.center.y).abs())
            })?;
        let local_x = x - measure.origin.x - offset;
        let raw = measure.content.tick_at_x(local_x);
        // The grid is a sixteenth, or a thirty-second for the notes that short.
        let duration = self.base.event(self.id)?.duration;
        let fine = duration.value() == score_core::NoteValue::ThirtySecond;
        let quantum = match (fine, duration.is_triplet()) {
            (false, false) => 12,
            (false, true) => 8,
            (true, false) => 6,
            (true, true) => 4,
        };
        let capacity = self.base.time_at(measure.index).capacity();
        let mut tick = ((raw / f64::from(quantum)).round() as u16 * quantum).min(capacity);
        if let Some(event) = measure
            .content
            .events
            .iter()
            .filter(|e| e.voice == voice)
            .min_by(|a, b| {
                (measure.content.x_at_tick(f64::from(a.tick)) - local_x)
                    .abs()
                    .total_cmp(&(measure.content.x_at_tick(f64::from(b.tick)) - local_x).abs())
            })
            .filter(|e| (measure.content.x_at_tick(f64::from(e.tick)) - local_x).abs() <= 10.)
        {
            tick = event.tick;
        }
        measure
            .regions
            .iter()
            .any(|r| r.contains(voice, tick))
            .then_some((measure.index, tick))
    }
}

#[cfg(test)]
mod tests;
