//! Runs of empty bars that an instrumental part gathers into one
//! multi-measure rest, as Sibelius, Dorico and MuseScore do.
//!
//! Only parts gather bars; the full score always shows every bar. A run is
//! broken wherever the player has to see something at a bar.
use super::*;
use std::ops::Range;

impl Score {
    /// The runs of two or more bars that `part` shows as multi-measure rests,
    /// in order. Engraving and export decide whether to use them; see
    /// `PageSetup::separate_empty_bars`.
    pub fn multirests(&self, part: PartId) -> Vec<Range<usize>> {
        let Some(part) = self.part(part) else {
            return Vec::new();
        };
        let mut runs = Vec::new();
        let mut start: Option<usize> = None;
        for bar in 0..self.measures().len() {
            let empty = self.gathers(part, bar);
            if let Some(first) = start
                && (!empty || self.starts_multirest(part, bar))
            {
                if bar - first >= 2 {
                    runs.push(first..bar);
                }
                start = None;
            }
            if empty && start.is_none() {
                start = Some(bar);
            }
        }
        if let Some(first) = start
            && self.measures().len() - first >= 2
        {
            runs.push(first..self.measures().len());
        }
        runs
    }

    /// Whether `bar` can be part of a multi-measure rest: it has nothing but
    /// rests in every staff and voice, and nothing the player must read there.
    fn gathers(&self, part: &Part, bar: usize) -> bool {
        let silent = part.staves().iter().all(|staff| {
            let measure = &staff.measures()[bar];
            !measure.repeats_previous()
                && measure.timed_iter().all(|(_, _, _, event)| {
                    event.pitches.is_empty() && event.notation == Notation::default()
                })
        });
        let form = self.bar_form(bar);
        silent
            && !part.bar_has_divisi(bar)
            && part.divisi_changes_in(bar).next().is_none()
            && self.ending_at(bar).is_none()
            // Navigation marks and their words stay on a bar of their own.
            && !(form.segno || form.coda || form.to_coda || form.fine || form.jump.is_some())
            && !self.connections().iter().any(|c| {
                c.start.part == part.id() && c.start.measure <= bar && bar <= c.end.measure
            })
    }

    /// Whether `bar` must begin a new multi-measure rest rather than continue
    /// the one before it: something new is shown at its start, or the bar
    /// before it ends with a barline, repeat or break the player must see.
    fn starts_multirest(&self, part: &Part, bar: usize) -> bool {
        let previous = self.bar_form(bar - 1);
        self.time_change_at(bar).is_some()
            || self.key_change_at(bar).is_some()
            || self.tempo_change_at(bar).is_some()
            || self.has_rehearsal_mark(bar)
            || self.bar_form(bar).repeat_start
            || part
                .staves()
                .iter()
                .any(|staff| part.clef_change_at(staff.id(), bar).is_some())
            || previous.repeat_end.is_some()
            || previous.barline != Barline::Single
            || self.layout_break(bar - 1).is_some()
    }
}
