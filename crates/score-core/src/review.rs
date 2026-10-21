//! Advisory proofreading. Empty bars and implicit rests are valid notation.
use crate::{BarMark, ElementId, InstrumentRange, Pitch, Score};
use std::collections::HashSet;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReviewTarget {
    Note(ElementId, Pitch),
    Bar(usize, Option<BarMark>),
}
impl ReviewTarget {
    pub fn measure(self) -> usize {
        match self {
            Self::Note(id, _) => id.measure,
            Self::Bar(bar, _) => bar,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReviewKind {
    OutOfRange(InstrumentRange),
    MissingSegno,
    MissingFine,
    MissingCoda,
    MissingToCoda,
    ToCodaWithoutJump,
    RepeatWithoutEnd,
}
impl ReviewKind {
    pub fn message(self) -> String {
        match self {
            Self::OutOfRange(range) => {
                format!("Nota fuera de rango. Registro en altura real: {range}.")
            }
            Self::MissingSegno => "El salto D.S. no tiene un Segno de destino.".into(),
            Self::MissingFine => "El salto al Fine no tiene una marca Fine en su recorrido.".into(),
            Self::MissingCoda => "No hay una Coda después de To Coda.".into(),
            Self::MissingToCoda => {
                "El salto al Coda no tiene una marca To Coda en su recorrido.".into()
            }
            Self::ToCodaWithoutJump => {
                "To Coda no tiene un salto D.C./D.S. al Coda que lo active.".into()
            }
            Self::RepeatWithoutEnd => {
                "El inicio de repetición no tiene un fin de repetición.".into()
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReviewIssue {
    pub target: ReviewTarget,
    pub kind: ReviewKind,
}

impl Score {
    /// A selection checks its exact events and global form marks in its bars.
    /// `None` checks the complete score; an empty selection checks nothing.
    pub fn review(&self, selection: Option<&[ElementId]>) -> Vec<ReviewIssue> {
        let selected: Option<HashSet<_>> = selection.map(|ids| {
            ids.iter()
                .copied()
                .filter(|id| self.event(*id).is_some())
                .collect()
        });
        let bars = selected.as_ref().and_then(|ids| {
            Some((
                ids.iter().map(|id| id.measure).min()?,
                ids.iter().map(|id| id.measure).max()?,
            ))
        });
        let mut issues = Vec::new();
        for part in self.parts() {
            for staff in part.staves() {
                for (measure, bar) in staff.measures().iter().enumerate() {
                    for (_, _, event, value) in bar.timed_iter() {
                        let id = ElementId {
                            part: part.id(),
                            staff: staff.id(),
                            measure,
                            event,
                        };
                        if selected.as_ref().is_some_and(|ids| !ids.contains(&id)) {
                            continue;
                        }
                        for pitch in value.pitches.iter() {
                            if let Some(range) = self.pitch_range_warning(id, pitch) {
                                issues.push(ReviewIssue {
                                    target: ReviewTarget::Note(id, pitch),
                                    kind: ReviewKind::OutOfRange(range),
                                });
                            }
                        }
                    }
                }
            }
        }
        let len = self.measures().len();
        let forms: Vec<_> = (0..len).map(|bar| self.bar_form(bar)).collect();
        let segno = forms.iter().position(|form| form.segno);
        let routes: Vec<_> = forms
            .iter()
            .map(|form| {
                form.jump.and_then(|jump| {
                    let start = if jump.from_segno() { segno } else { Some(0) }?;
                    Some(return_route(self, start, jump))
                })
            })
            .collect();
        let active_to_coda: HashSet<_> = forms
            .iter()
            .zip(&routes)
            .filter(|(form, _)| form.jump.is_some_and(|jump| jump.goes_to_coda()))
            .filter_map(|(_, route)| route.as_ref())
            .flatten()
            .copied()
            .filter(|bar| forms[*bar].to_coda)
            .collect();
        let mut form_issues = Vec::new();
        let mut add = |bar, mark, kind| {
            form_issues.push(ReviewIssue {
                target: ReviewTarget::Bar(bar, mark),
                kind,
            })
        };
        let mut repeat_start = None;
        for (bar, form) in forms.iter().enumerate() {
            if form.repeat_start
                && let Some(start) = repeat_start.replace(bar)
            {
                add(start, None, ReviewKind::RepeatWithoutEnd);
            }
            if form.repeat_end.is_some() {
                repeat_start = None; // A closing repeat without an opening repeat returns to bar 1.
            }
            if let Some(jump) = form.jump {
                let start = if jump.from_segno() { segno } else { Some(0) };
                if start.is_none() {
                    add(bar, Some(BarMark::Jump), ReviewKind::MissingSegno);
                }
                if let Some(route) = &routes[bar] {
                    if jump.ends_at_fine() && !route.iter().any(|bar| forms[*bar].fine) {
                        add(bar, Some(BarMark::Jump), ReviewKind::MissingFine);
                    }
                    if jump.goes_to_coda() && !route.iter().any(|bar| forms[*bar].to_coda) {
                        add(bar, Some(BarMark::Jump), ReviewKind::MissingToCoda);
                    }
                }
            }
            if form.to_coda {
                if !forms[bar + 1..].iter().any(|f| f.coda) {
                    add(bar, Some(BarMark::ToCoda), ReviewKind::MissingCoda);
                }
                if !active_to_coda.contains(&bar) {
                    add(bar, Some(BarMark::ToCoda), ReviewKind::ToCodaWithoutJump);
                }
            }
        }
        if let Some(start) = repeat_start {
            add(start, None, ReviewKind::RepeatWithoutEnd);
        }
        issues.extend(form_issues.into_iter().filter(|issue| {
            selected.is_none()
                || bars.is_some_and(|(low, high)| (low..=high).contains(&issue.target.measure()))
        }));
        issues.sort_by_key(|issue| issue.target.measure());
        issues
    }
}

/// The return journey follows the same ending, Fine and forward-Coda rules as
/// play_order. Repeats and additional D.C./D.S. commands are ignored after a jump.
fn return_route(score: &Score, start: usize, jump: crate::Jump) -> Vec<usize> {
    let len = score.measures().len();
    let mut bar = start;
    let mut route = Vec::new();
    while bar < len {
        if let Some(ending) = score.ending_at(bar).filter(|ending| ending.start == bar)
            && !score.is_last_ending(ending)
        {
            bar = ending.end + 1;
            continue;
        }
        route.push(bar);
        let form = score.bar_form(bar);
        if form.fine && jump.ends_at_fine() {
            break;
        }
        if form.to_coda
            && jump.goes_to_coda()
            && let Some(target) = (bar + 1..len).find(|bar| score.bar_form(*bar).coda)
        {
            bar = target;
        } else {
            bar += 1;
        }
        // Every transition moves forward, so the route is bounded by `len`.
    }
    route
}

#[cfg(test)]
mod tests;
