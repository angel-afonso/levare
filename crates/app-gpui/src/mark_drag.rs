//! A mark follows the pointer on a shifted copy of the displayed layout; the
//! document changes once, when the pointer is released. A mark whose place in
//! time is musical goes to the nearest note of its voice instead, unless ⌘ is
//! held: it then sounds there too.
use gpui::{Modifiers, Pixels, Point, px};
use score_core::{BarMarkRef, ElementId, MarkRef, Offset, Score};
use score_layout::{Layout, LayoutEngine, Point as ScorePoint};
use std::rc::Rc;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum DragTarget {
    Mark(MarkRef),
    Bar(BarMarkRef),
}

impl DragTarget {
    pub fn label(self, score: &Score) -> String {
        match self {
            Self::Mark(mark) => score.mark_label(mark).unwrap_or_default(),
            Self::Bar(mark) => match mark.mark {
                score_core::BarMark::Tempo => "Tempo",
                score_core::BarMark::Rehearsal => "Marca de ensayo",
                score_core::BarMark::Segno => "Segno",
                score_core::BarMark::Coda => "Coda",
                score_core::BarMark::ToCoda => "To Coda",
                score_core::BarMark::Fine => "Fine",
                score_core::BarMark::Jump => "Salto",
                score_core::BarMark::Ending => "Casilla",
            }
            .into(),
        }
    }

    fn exists(self, score: &Score) -> bool {
        match self {
            Self::Mark(mark) => mark.can_move() && score.has_mark(mark),
            Self::Bar(mark) => score.has_bar_mark(mark),
        }
    }

    fn offset(self, score: &Score) -> Offset {
        match self {
            Self::Mark(mark) => score.mark_offset(mark),
            Self::Bar(mark) => score.bar_mark_offset(mark),
        }
    }

    fn translate(self, layout: &mut Layout, dx: f32, dy: f32) {
        match self {
            Self::Mark(mark) => layout.translate_mark(mark, dx, dy),
            Self::Bar(mark) => layout.translate_bar_mark(mark, dx, dy),
        }
    }
}

pub(super) struct MarkDrag {
    pub mark: DragTarget,
    base: Offset,
    start: Point<Pixels>,
    origin: Point<Pixels>,
    scale: f32,
    pub active: bool,
    delta: (i16, i16),
    shown: Rc<Layout>,
    pub preview: Option<Rc<Layout>>,
    /// Present for marks that follow time.
    anchoring: Option<Anchoring>,
    /// Where the mark goes when released, if it leaves its note.
    pub moved: Option<MarkRef>,
    pub error: Option<&'static str>,
}

struct Anchoring {
    score: Score,
    layout: LayoutEngine,
    /// Engraved once per destination, not once per pointer move.
    engraved: Option<(MarkRef, Score, Rc<Layout>)>,
    refused: Option<(MarkRef, &'static str)>,
}

/// Vertical pointer travel ignored while a mark is dragged sideways, so a
/// slightly unsteady hand leaves its height alone.
const WOBBLE: f32 = 4.;

impl MarkDrag {
    pub fn new(
        score: &Score,
        mark: MarkRef,
        start: Point<Pixels>,
        origin: Point<Pixels>,
        scale: f32,
        shown: Rc<Layout>,
    ) -> Option<Self> {
        Self::new_target(score, DragTarget::Mark(mark), start, origin, scale, shown)
    }

    pub fn new_bar(
        score: &Score,
        mark: BarMarkRef,
        start: Point<Pixels>,
        origin: Point<Pixels>,
        scale: f32,
        shown: Rc<Layout>,
    ) -> Option<Self> {
        Self::new_target(score, DragTarget::Bar(mark), start, origin, scale, shown)
    }

    fn new_target(
        score: &Score,
        mark: DragTarget,
        start: Point<Pixels>,
        origin: Point<Pixels>,
        scale: f32,
        shown: Rc<Layout>,
    ) -> Option<Self> {
        let anchoring =
            matches!(mark, DragTarget::Mark(mark) if mark.follows_time()).then(|| Anchoring {
                score: score.clone(),
                layout: LayoutEngine::default(),
                engraved: None,
                refused: None,
            });
        mark.exists(score).then(|| Self {
            mark,
            base: mark.offset(score),
            start,
            origin,
            scale,
            active: false,
            delta: (0, 0),
            shown,
            preview: None,
            anchoring,
            moved: None,
            error: None,
        })
    }

    pub fn pending(&self, position: Point<Pixels>) -> bool {
        !self.active && {
            let delta = position - self.start;
            delta.x.abs().max(delta.y.abs()) < px(5.)
        }
    }

    /// Rebuild from the immutable displayed layout, so motion never accumulates.
    /// `free` (⌘) moves a mark that follows time without changing its note.
    pub fn update(
        &mut self,
        position: Point<Pixels>,
        origin: Point<Pixels>,
        axis_lock: bool,
        free: bool,
    ) -> bool {
        if self.pending(position) {
            return false;
        }
        self.active = true;
        let pixels = (position - origin) - (self.start - self.origin);
        let (mut x, mut y) = (f32::from(pixels.x), f32::from(pixels.y));
        if axis_lock {
            if x.abs() >= y.abs() {
                y = 0.;
            } else {
                x = 0.;
            }
        }
        let anchored = !free && self.anchoring.is_some();
        let destination = if anchored {
            y = y.signum() * (y.abs() - WOBBLE).max(0.);
            let destination = self.destination_at(x, y);
            // Sideways, the note decides where the mark sits.
            x = 0.;
            destination
        } else {
            Ok(None)
        };
        let max = i32::from(Offset::MAX);
        let x = (i32::from(self.base.dx) + (x / self.scale).round() as i32).clamp(-max, max);
        let y = (i32::from(self.base.dy) + (y / self.scale).round() as i32).clamp(-max, max);
        let delta = (
            (x - i32::from(self.base.dx)) as i16,
            (y - i32::from(self.base.dy)) as i16,
        );
        let (moved, error) = match destination {
            Ok(moved) => (moved, None),
            Err(error) => (None, Some(error)),
        };
        if delta == self.delta && moved == self.moved && error == self.error {
            return false;
        }
        self.delta = delta;
        self.moved = moved;
        self.error = error;
        self.preview = match (moved, error) {
            (_, Some(_)) => None,
            (Some(moved), None) => {
                let engraved = &self.anchoring.as_ref().unwrap().engraved;
                let mut layout = (*engraved.as_ref().unwrap().2).clone();
                layout.translate_mark(moved, 0., f32::from(delta.1));
                Some(Rc::new(layout))
            }
            (None, None) if delta == (0, 0) => None,
            (None, None) => {
                let mut layout = (*self.shown).clone();
                self.mark
                    .translate(&mut layout, f32::from(delta.0), f32::from(delta.1));
                Some(Rc::new(layout))
            }
        };
        true
    }

    /// The note nearest to where the pointer has carried the mark's own note,
    /// and the mark written there; `None` while it stays on its note.
    fn destination_at(&mut self, x: f32, y: f32) -> Result<Option<MarkRef>, &'static str> {
        let DragTarget::Mark(mark) = self.mark else {
            return Ok(None);
        };
        let anchor = mark.anchor();
        let Some(here) = event_point(&self.shown, anchor) else {
            return Ok(None);
        };
        let point = ScorePoint {
            x: here.x + x,
            y: here.y + y,
        };
        let Some(target) = crate::selection::nearest_event(&self.shown, anchor, point, false)
        else {
            return Ok(None);
        };
        if target == anchor {
            return Ok(None);
        }
        let anchoring = self.anchoring.as_mut().expect("mark that follows time");
        let to = match mark {
            MarkRef::Connection(line) => MarkRef::Connection(
                anchoring
                    .score
                    .shifted_connection(line, target)
                    .ok_or("El regulador no cabe ahí")?,
            ),
            _ => {
                let family = anchoring
                    .score
                    .mark_kind(mark)
                    .ok_or("La marca ya no está en la partitura")?
                    .family();
                MarkRef::point(family, target).ok_or("Esta marca no se puede mover")?
            }
        };
        if let Some((refused, error)) = anchoring.refused
            && refused == to
        {
            return Err(error);
        }
        if anchoring
            .engraved
            .as_ref()
            .is_none_or(|(engraved, _, _)| *engraved != to)
        {
            let mut score = anchoring.score.clone();
            let offset = Offset {
                dx: 0,
                dy: self.base.dy,
            };
            if let Err(error) = score
                .move_mark(mark, to, offset)
                .and_then(|_| score.validate())
            {
                anchoring.refused = Some((to, error.0));
                return Err(error.0);
            }
            let layout = anchoring.layout.layout_pitch_preview(&score, &self.shown);
            anchoring.engraved = Some((to, score, Rc::new(layout)));
        }
        Ok(Some(to))
    }

    /// The offset to keep when the mark stays on its note.
    pub fn target(&self) -> Option<Offset> {
        (self.moved.is_none() && self.error.is_none() && self.delta != (0, 0)).then_some(Offset {
            dx: self.base.dx + self.delta.0,
            dy: self.base.dy + self.delta.1,
        })
    }

    /// The note the mark goes to and its offset there: horizontally on the
    /// note, at the height it was dragged to.
    pub fn destination(&self) -> Option<(MarkRef, Offset)> {
        let moved = self.moved.filter(|_| self.error.is_none())?;
        Some((
            moved,
            Offset {
                dx: 0,
                dy: self.base.dy + self.delta.1,
            },
        ))
    }

    /// The score shown while the mark sits on another note.
    pub fn preview_score(&self) -> Option<&Score> {
        let moved = self.moved?;
        let (engraved, score, _) = self.anchoring.as_ref()?.engraved.as_ref()?;
        (*engraved == moved).then_some(score)
    }

    /// The mark as currently shown, on its own note or the one it goes to.
    pub fn shown_mark(&self) -> Option<MarkRef> {
        match self.mark {
            DragTarget::Mark(mark) => Some(self.moved.unwrap_or(mark)),
            DragTarget::Bar(_) => None,
        }
    }

    /// Dashed lines from the dragged mark to the note or notes it belongs to,
    /// in canvas pixels.
    pub fn anchor_lines(&self) -> Vec<(ScorePoint, ScorePoint)> {
        if !self.active || self.anchoring.is_none() {
            return Vec::new();
        }
        let Some(mark) = self.shown_mark() else {
            return Vec::new();
        };
        let layout = self.preview.as_deref().unwrap_or(&self.shown);
        let scale = layout.scale;
        if let MarkRef::Connection(line) = mark {
            let Some((start, end)) = crate::connection_input::handles(layout, line) else {
                return Vec::new();
            };
            let at = |(x, y): (f32, f32)| ScorePoint { x, y };
            return [(at(start), line.start), (at(end), line.end)]
                .into_iter()
                .filter_map(|(from, id)| Some((from, event_point(layout, id)?)))
                .collect();
        }
        let center = layout.measures.iter().find_map(|placed| {
            let found = placed.content.marks.iter().find(|box_| box_.mark == mark)?;
            Some(ScorePoint {
                x: (placed.origin.x + found.center.x) * scale,
                y: (placed.origin.y + found.center.y) * scale,
            })
        });
        center
            .zip(event_point(layout, mark.anchor()))
            .into_iter()
            .collect()
    }

    pub fn follows_time(&self) -> bool {
        self.anchoring.is_some()
    }

    pub fn zoom_changed(&self, zoom: f32) -> bool {
        self.scale != zoom
    }
}

/// "compás 5, tiempo 3" for the onset of `id`, counting beats of its bar.
pub(super) fn place_label(score: &Score, id: ElementId) -> String {
    let bar = id.measure + 1;
    let tick = score.event_tick(id).unwrap_or_default() - score.measure_start_tick(id.measure);
    let beat =
        u64::from(score_core::TICKS_PER_WHOLE / u16::from(score.time_at(id.measure).denominator));
    if tick.is_multiple_of(beat) {
        format!("compás {bar}, tiempo {}", tick / beat + 1)
    } else {
        let beat = format!("{:.2}", 1. + tick as f64 / beat as f64);
        let beat = beat.trim_end_matches('0').replace('.', ",");
        format!("compás {bar}, tiempo {beat}")
    }
}

/// Canvas position of an event's first notehead, or of its rest.
fn event_point(layout: &Layout, id: ElementId) -> Option<ScorePoint> {
    layout.measures.iter().find_map(|placed| {
        let event = placed.content.events.iter().find(|event| event.id == id)?;
        Some(ScorePoint {
            x: (placed.origin.x + event.center.x) * layout.scale,
            y: (placed.origin.y + event.center.y) * layout.scale,
        })
    })
}

/// The mark one note earlier or later in its voice, as ⌥← and ⌥→ move it.
pub(super) fn stepped(score: &Score, mark: MarkRef, later: bool) -> Option<MarkRef> {
    let near = |id| {
        if later {
            score.next_event(id)
        } else {
            score.previous_event(id)
        }
    };
    match mark {
        MarkRef::Connection(line) => score
            .shifted_connection(line, near(line.start)?)
            .map(MarkRef::Connection),
        _ => MarkRef::point(score.mark_kind(mark)?.family(), near(mark.anchor())?),
    }
}

/// ⌥← or ⌥→: whether the step goes later.
pub(super) fn step(key: &str, modifiers: Modifiers) -> Option<bool> {
    if !modifiers.alt || modifiers.shift || modifiers.control || modifiers.platform {
        return None;
    }
    match key {
        "left" => Some(false),
        "right" => Some(true),
        _ => None,
    }
}

/// Physical arrow keys keep their meaning even under a custom key profile.
pub(super) fn nudge(key: &str, modifiers: Modifiers) -> Option<(i16, i16)> {
    if modifiers.shift || modifiers.alt || modifiers.control {
        return None;
    }
    let step = if modifiers.platform { 10 } else { 2 };
    Some(match key {
        "up" => (0, -step),
        "down" => (0, step),
        "left" => (-step, 0),
        "right" => (step, 0),
        _ => return None,
    })
}

#[cfg(test)]
mod tests;
