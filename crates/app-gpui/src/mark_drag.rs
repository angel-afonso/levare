//! A mark follows the pointer on a shifted copy of the displayed layout; the
//! document changes once, when the pointer is released.
use gpui::{Modifiers, Pixels, Point, px};
use score_core::{BarMarkRef, MarkRef, Offset, Score};
use score_layout::Layout;
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
}

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
        })
    }

    pub fn pending(&self, position: Point<Pixels>) -> bool {
        !self.active && {
            let delta = position - self.start;
            delta.x.abs().max(delta.y.abs()) < px(5.)
        }
    }

    /// Rebuild from the immutable displayed layout, so motion never accumulates.
    pub fn update(
        &mut self,
        position: Point<Pixels>,
        origin: Point<Pixels>,
        axis_lock: bool,
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
        let max = i32::from(Offset::MAX);
        let x = (i32::from(self.base.dx) + (x / self.scale).round() as i32).clamp(-max, max);
        let y = (i32::from(self.base.dy) + (y / self.scale).round() as i32).clamp(-max, max);
        let delta = (
            (x - i32::from(self.base.dx)) as i16,
            (y - i32::from(self.base.dy)) as i16,
        );
        if delta == self.delta {
            return false;
        }
        self.delta = delta;
        self.preview = if delta == (0, 0) {
            None
        } else {
            let mut layout = (*self.shown).clone();
            self.mark
                .translate(&mut layout, f32::from(delta.0), f32::from(delta.1));
            Some(Rc::new(layout))
        };
        true
    }

    pub fn target(&self) -> Option<Offset> {
        (self.delta != (0, 0)).then_some(Offset {
            dx: self.base.dx + self.delta.0,
            dy: self.base.dy + self.delta.1,
        })
    }

    pub fn zoom_changed(&self, zoom: f32) -> bool {
        self.scale != zoom
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
