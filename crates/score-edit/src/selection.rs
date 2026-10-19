//! A continuous passage on one staff, with stable anchor/focus identities.
use super::*;
impl Editor {
    pub fn selected_ids(&self) -> Vec<ElementId> {
        let Some(focus) = self.selection() else {
            return Vec::new();
        };
        let Some(anchor) = self.state.range_anchor else {
            return vec![focus];
        };
        let staff = self
            .score()
            .part(focus.part)
            .unwrap()
            .staff(focus.staff)
            .unwrap();
        let position = |id: ElementId| {
            (
                id.measure,
                staff.measures()[id.measure].index_of(id.event).unwrap(),
            )
        };
        let (a, b) = (position(anchor), position(focus));
        let (low, high) = (a.min(b), a.max(b));
        (low.0..=high.0)
            .flat_map(|measure| {
                staff.measures()[measure].iter().enumerate().filter_map(
                    move |(index, (event, _))| {
                        ((measure, index) >= low && (measure, index) <= high).then_some(ElementId {
                            measure,
                            event,
                            ..focus
                        })
                    },
                )
            })
            .collect()
    }
    pub fn extend_selection(&mut self, id: ElementId) {
        let anchor = self.state.range_anchor.or(self.selection());
        self.select(id);
        if self.selection() == Some(id) {
            self.state.range_anchor = anchor.filter(|a| {
                a.part == id.part && a.staff == id.staff && self.score().event(*a).is_some()
            });
        }
    }
    pub fn select_all(&mut self) -> bool {
        let ids = self.staff_ids();
        let (Some(first), Some(last)) = (ids.first(), ids.last()) else {
            return false;
        };
        self.select(*last);
        self.state.range_anchor = Some(*first);
        true
    }
    pub fn extend_navigation(&mut self, forward: bool) {
        let ids = self.staff_ids();
        let target = if let Some(focus) = self.selection() {
            ids.iter().position(|id| *id == focus).and_then(|i| {
                if forward {
                    ids.get(i + 1)
                } else {
                    i.checked_sub(1).and_then(|i| ids.get(i))
                }
            })
        } else {
            let cursor = self.cursor();
            let position = |id: &&ElementId| {
                let measure = &self
                    .score()
                    .part(id.part)
                    .unwrap()
                    .staff(id.staff)
                    .unwrap()
                    .measures()[id.measure];
                (id.measure, measure.index_of(id.event).unwrap())
            };
            if forward {
                ids.iter()
                    .find(|id| position(id) >= (cursor.measure, cursor.index))
            } else {
                ids.iter()
                    .rev()
                    .find(|id| position(id) < (cursor.measure, cursor.index))
            }
        };
        if let Some(id) = target {
            self.extend_selection(*id);
        }
    }
    fn staff_ids(&self) -> Vec<ElementId> {
        let cursor = self.cursor();
        self.score()
            .part(cursor.part)
            .unwrap()
            .staff(cursor.staff)
            .unwrap()
            .measures()
            .iter()
            .enumerate()
            .flat_map(|(measure, bar)| {
                bar.iter().map(move |(event, _)| ElementId {
                    part: cursor.part,
                    staff: cursor.staff,
                    measure,
                    event,
                })
            })
            .collect()
    }
}
