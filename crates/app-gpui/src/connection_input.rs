//! Slurs and hairpins drawn from a passage, while typing, or with the pointer.
//! A dragged line stays outside the document until the pointer is released.
use gpui::{Pixels, Point, px};
use score_core::{Connection, ConnectionKind, ElementId, Score};
use score_layout::{Layout, LayoutEngine};
use std::rc::Rc;

/// The first and last events of a selected passage that can carry `kind`:
/// a slur joins notes or chords, a hairpin may also start or end on a rest.
pub(super) fn passage_endpoints(
    score: &Score,
    ids: &[ElementId],
    kind: ConnectionKind,
) -> Option<(ElementId, ElementId)> {
    let usable = |id: &&ElementId| kind.is_hairpin() || sounding(score, **id);
    let start = *ids.iter().find(usable)?;
    let end = *ids.iter().rfind(usable)?;
    (start != end).then_some((start, end))
}

/// The line from `start` to the note just typed, if it can reach it. A slur
/// waits on rests: it keeps ending on the last note until the next one comes.
pub(super) fn follow(
    score: &Score,
    start: ElementId,
    end: ElementId,
    kind: ConnectionKind,
) -> Option<Connection> {
    let ordered = score.event_tick(start)? < score.event_tick(end)?;
    (ordered && (kind.is_hairpin() || sounding(score, end))).then_some(Connection {
        start,
        end,
        kind,
    })
}

/// The edit that stretches the line `current` (if already written) from
/// `start` to the note just typed, and the line it leaves.
pub(super) fn follow_command(
    score: &Score,
    start: ElementId,
    end: ElementId,
    kind: ConnectionKind,
    current: Option<Connection>,
) -> Option<(Connection, score_edit::Command)> {
    let next = follow(score, start, end, kind)?;
    let command = match current {
        Some(current) if current == next => return None,
        Some(current) => score_edit::Command::ReplaceConnection(current, next),
        None => score_edit::Command::AddConnection(next),
    };
    Some((next, command))
}

fn sounding(score: &Score, id: ElementId) -> bool {
    score
        .event(id)
        .is_some_and(|event| !event.pitches.is_empty())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum End {
    Start,
    End,
}

/// The handle under `point` (canvas pixels) of a line already in the score.
pub(super) fn handle_at(layout: &Layout, connection: Connection, point: (f32, f32)) -> Option<End> {
    let (start, end) = handles(layout, connection)?;
    let reach = HANDLE_REACH * layout.scale;
    let near = |handle: (f32, f32)| {
        let (dx, dy) = (handle.0 - point.0, handle.1 - point.1);
        dx * dx + dy * dy <= reach * reach
    };
    if near(start) {
        Some(End::Start)
    } else if near(end) {
        Some(End::End)
    } else {
        None
    }
}

/// Handle radius in layout units; the pointer may land a little outside it.
pub(super) const HANDLE_RADIUS: f32 = 3.5;
const HANDLE_REACH: f32 = 7.;

/// Canvas positions of both ends, across systems: the first segment's start
/// and the last segment's end.
pub(super) fn handles(layout: &Layout, connection: Connection) -> Option<((f32, f32), (f32, f32))> {
    let scale = layout.scale;
    let at = |p: score_layout::Point| (p.x * scale, p.y * scale);
    if connection.kind.is_hairpin() {
        let mut segments = layout
            .hairpins
            .iter()
            .filter(|hairpin| hairpin.connection == connection);
        let first = segments.next()?;
        let last = segments.next_back().unwrap_or(first);
        // The open end of a wedge has two strokes; its handle sits between them.
        let middle = |a: score_layout::Point, b: score_layout::Point| score_layout::Point {
            x: (a.x + b.x) / 2.,
            y: (a.y + b.y) / 2.,
        };
        Some((
            at(middle(first.upper.from, first.lower.from)),
            at(middle(last.upper.to, last.lower.to)),
        ))
    } else {
        let mut segments = layout
            .curves
            .iter()
            .filter(|curve| curve.connection == connection);
        let first = segments.next()?;
        let last = segments.next_back().unwrap_or(first);
        Some((at(first.from), at(last.to)))
    }
}

/// A line whose moving end follows the nearest event of the fixed end's
/// voice. `original` is the line being stretched, or `None` while drawing one.
pub(super) struct ConnectionDrag {
    pub kind: ConnectionKind,
    pub fixed: ElementId,
    pub moving: End,
    pub original: Option<Connection>,
    base: Score,
    start: Point<Pixels>,
    pub active: bool,
    pub candidate: Option<Connection>,
    pub preview: Option<Score>,
    pub error: Option<&'static str>,
    layout: LayoutEngine,
    /// The layout shown when the drag began; pointer targets are found on it.
    pub geometry: Layout,
    /// Engraved once per candidate, not once per frame.
    shown: Option<Rc<Layout>>,
}

impl ConnectionDrag {
    /// Grab one end of `connection`; the other one stays.
    pub fn stretch(
        score: &Score,
        connection: Connection,
        moving: End,
        start: Point<Pixels>,
        geometry: &Layout,
    ) -> Self {
        let fixed = match moving {
            End::Start => connection.end,
            End::End => connection.start,
        };
        Self::new(
            score,
            connection.kind,
            fixed,
            moving,
            Some(connection),
            start,
            geometry,
        )
    }

    /// Start a new line at `fixed`; the pointer chooses where it ends.
    pub fn draw(
        score: &Score,
        kind: ConnectionKind,
        fixed: ElementId,
        start: Point<Pixels>,
        geometry: &Layout,
    ) -> Self {
        Self::new(score, kind, fixed, End::End, None, start, geometry)
    }

    fn new(
        score: &Score,
        kind: ConnectionKind,
        fixed: ElementId,
        moving: End,
        original: Option<Connection>,
        start: Point<Pixels>,
        geometry: &Layout,
    ) -> Self {
        Self {
            kind,
            fixed,
            moving,
            original,
            base: score.clone(),
            start,
            active: false,
            candidate: None,
            preview: None,
            error: None,
            layout: LayoutEngine::default(),
            geometry: geometry.clone(),
            shown: None,
        }
    }

    /// The previewed layout at `scale`, if the current target is valid.
    pub fn shown(&mut self, scale: f32) -> Option<Rc<Layout>> {
        let score = self.preview.as_ref()?;
        if self.shown.as_ref().is_none_or(|shown| shown.scale != scale) {
            let mut layout = self.layout.layout_pitch_preview(score, &self.geometry);
            layout.scale = scale;
            self.shown = Some(Rc::new(layout));
        }
        self.shown.clone()
    }

    pub fn pending(&self, position: Point<Pixels>) -> bool {
        !self.active && {
            let delta = position - self.start;
            delta.x.abs().max(delta.y.abs()) < px(5.)
        }
    }

    /// Point the moving end at `target`. Returns whether the preview changed.
    /// An end on the wrong side, a rest under a slur or an overlap keeps no
    /// candidate, so releasing there changes nothing.
    pub fn aim(&mut self, target: ElementId) -> bool {
        self.active = true;
        let (start, end) = match self.moving {
            End::Start => (target, self.fixed),
            End::End => (self.fixed, target),
        };
        let candidate = Connection {
            start,
            end,
            kind: self.kind,
        };
        if self.candidate == Some(candidate) {
            return false;
        }
        let mut score = self.base.clone();
        if let Some(original) = self.original {
            score.remove_connection(original);
        }
        let ordered = self.base.event_tick(start) < self.base.event_tick(end);
        let fits = ordered
            && (self.kind.is_hairpin()
                || (sounding(&self.base, start) && sounding(&self.base, end)));
        let result = if fits {
            score.add_connection(candidate).map_err(|error| error.0)
        } else if !ordered {
            Err("El final debe quedar después del inicio")
        } else {
            Err("La ligadura de expresión une notas o acordes, no silencios")
        };
        self.shown = None;
        match result {
            Ok(()) => {
                self.candidate = Some(candidate);
                self.error = None;
                self.preview = Some(score);
                true
            }
            Err(error) => {
                let changed = self.candidate.is_some() || self.error != Some(error);
                self.candidate = None;
                self.error = Some(error);
                self.preview = None;
                changed
            }
        }
    }

    /// The command for releasing the pointer, if the line changed.
    pub fn command(&self) -> Option<score_edit::Command> {
        let candidate = self.candidate?;
        match self.original {
            Some(original) if original == candidate => None,
            Some(original) => Some(score_edit::Command::ReplaceConnection(original, candidate)),
            None => Some(score_edit::Command::AddConnection(candidate)),
        }
    }
}

#[cfg(test)]
mod tests;
