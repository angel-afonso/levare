//! Resolved, piecewise linear dynamics shared by playback and passage copying.
use super::*;
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug)]
pub struct DynamicSegment {
    pub start: u64,
    pub end: u64,
    pub from: u8,
    pub to: u8,
}
impl DynamicSegment {
    pub fn level_at(self, tick: u64) -> u8 {
        let position = tick.saturating_sub(self.start).min(self.end - self.start);
        let delta = i64::from(self.to) - i64::from(self.from);
        (f64::from(self.from) + delta as f64 * position as f64 / (self.end - self.start) as f64)
            .round() as u8
    }
}
#[derive(Clone, Debug)]
pub struct DynamicEnvelope {
    pub segments: Vec<DynamicSegment>,
}
impl DynamicEnvelope {
    pub fn new(score: &Score, part: PartId, staff: StaffId) -> Option<Self> {
        let staff = score.part(part)?.staff(staff)?;
        let mut marks = BTreeMap::new();
        for (index, bar) in staff.measures().iter().enumerate() {
            let mut tick = index as u64 * u64::from(score.time().capacity());
            for (_, event) in bar.iter() {
                if let Some(mark) = event.notation.dynamic {
                    marks.insert(tick, mark.velocity());
                }
                tick += u64::from(event.duration.ticks());
            }
        }
        let mut hairpins: Vec<_> = score
            .connections()
            .iter()
            .filter(|c| c.kind.is_hairpin() && c.start.part == part && c.start.staff == staff.id())
            .collect();
        hairpins.sort_by_key(|c| score.event_tick(c.start).unwrap());
        let mut segments = Vec::new();
        let mut cursor = 0;
        let mut level = Dynamic::Mf.velocity();
        let mut marks = marks.into_iter().peekable();
        let mut push = |start, end, from, to| {
            if start < end {
                segments.push(DynamicSegment {
                    start,
                    end,
                    from,
                    to,
                });
            }
        };
        for hairpin in hairpins {
            let start = score.event_tick(hairpin.start)?;
            let end = score.event_tick(hairpin.end)?;
            while marks.peek().is_some_and(|(tick, _)| *tick <= start) {
                let (tick, mark) = marks.next().unwrap();
                push(cursor, tick, level, level);
                cursor = tick;
                level = mark;
            }
            push(cursor, start, level, level);
            cursor = start;
            while marks.peek().is_some_and(|(tick, _)| *tick <= end) {
                let (tick, mark) = marks.next().unwrap();
                push(cursor, tick, level, mark);
                cursor = tick;
                level = mark;
            }
            if cursor < end {
                let fallback = Dynamic::nearest(level)
                    .shifted(if hairpin.kind == ConnectionKind::Crescendo {
                        2
                    } else {
                        -2
                    })
                    .velocity();

                push(cursor, end, level, fallback);
                level = fallback;
            }
            cursor = end;
        }
        for (tick, mark) in marks {
            push(cursor, tick, level, level);
            cursor = tick;
            level = mark;
        }
        // Include the entire notated timeline, even incomplete/empty final bars.
        let end = staff.measures().len() as u64 * u64::from(score.time().capacity());
        push(cursor, end, level, level);
        Some(Self { segments })
    }
    /// Left limit, before an explicit dynamic at an insertion cursor.
    pub fn level_before(&self, tick: u64) -> u8 {
        self.segments
            .partition_point(|s| s.start < tick)
            .checked_sub(1)
            .map_or(Dynamic::Mf.velocity(), |i| self.segments[i].level_at(tick))
    }
    pub fn level_at(&self, tick: u64) -> u8 {
        let index = self
            .segments
            .partition_point(|s| s.start <= tick)
            .saturating_sub(1);
        self.segments
            .get(index)
            .map_or(Dynamic::Mf.velocity(), |s| s.level_at(tick))
    }
}
impl Dynamic {
    pub fn nearest(level: u8) -> Self {
        Self::ALL
            .into_iter()
            .min_by_key(|d| d.velocity().abs_diff(level))
            .unwrap()
    }
    pub fn shifted(self, steps: i8) -> Self {
        let index = Self::ALL.iter().position(|d| *d == self).unwrap() as i8;
        Self::ALL[(index + steps).clamp(0, 7) as usize]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn note() -> Event {
        Event {
            pitches: Pitches::single(Pitch::new(Step::C, 4)),
            duration: Duration::Quarter,
            notation: Default::default(),
        }
    }
    #[test]
    fn hairpins_validate_ranges_allow_rests_and_touching_but_reject_overlap_and_other_staves() {
        let mut s = Score::default();
        let mut ids = Vec::new();
        for i in 0..4 {
            ids.push(s.insert(0, i, note()).unwrap());
        }
        let left = s
            .insert_on(ids[0].part, StaffId::Lower, 0, 0, note())
            .unwrap();
        let c = Connection {
            start: ids[0],
            end: ids[2],
            kind: ConnectionKind::Crescendo,
        };
        assert!(s.add_connection(Connection { end: left, ..c }).is_err());
        assert!(s.add_connection(Connection { end: ids[0], ..c }).is_err());
        assert!(
            s.add_connection(Connection {
                start: ids[2],
                end: ids[0],
                ..c
            })
            .is_err()
        );
        s.add_connection(c).unwrap();
        assert!(
            s.add_connection(Connection {
                start: ids[1],
                end: ids[3],
                kind: ConnectionKind::Diminuendo
            })
            .is_err()
        );
        s.add_connection(Connection {
            start: ids[2],
            end: ids[3],
            kind: ConnectionKind::Diminuendo,
        })
        .unwrap();
        s.validate().unwrap();
        assert!(s.connection_contains(c, ids[1]));
        s.remove_connections(ids[1], ConnectionKind::Crescendo);
        assert_eq!(s.connections().len(), 1);
        s.remove(ids[3]).unwrap();
        assert!(s.connections().is_empty());
        let mut rest = note();
        rest.pitches = Pitches::default();
        let a = s.insert(1, 0, rest).unwrap();
        let b = s.insert(2, 0, rest).unwrap();
        s.add_connection(Connection {
            start: a,
            end: b,
            kind: ConnectionKind::Crescendo,
        })
        .unwrap();
        s.validate().unwrap();
    }
    #[test]
    fn an_unmarked_endpoint_continues_from_the_last_written_anchor() {
        for (kind, from, interior, end_level) in [
            (ConnectionKind::Crescendo, Dynamic::P, Dynamic::F, 124),
            (ConnectionKind::Diminuendo, Dynamic::F, Dynamic::P, 32),
        ] {
            let mut s = Score::default();
            let a = s.insert(0, 0, note()).unwrap();
            let b = s.insert(0, 1, note()).unwrap();
            let c = s.insert(0, 2, note()).unwrap();
            s.set_dynamic(a, Some(from)).unwrap();
            s.set_dynamic(b, Some(interior)).unwrap();
            s.add_connection(Connection {
                start: a,
                end: c,
                kind,
            })
            .unwrap();
            let e = DynamicEnvelope::new(&s, a.part, a.staff).unwrap();
            assert_eq!(e.level_at(48), interior.velocity());
            assert_eq!(e.level_at(96), end_level);
            assert!(if kind == ConnectionKind::Crescendo {
                e.level_at(72) > interior.velocity()
            } else {
                e.level_at(72) < interior.velocity()
            });
        }
    }
    #[test]
    fn ramps_follow_explicit_anchors_persist_and_default_to_two_dynamic_steps() {
        let mut s = Score::default();
        let mut ids = Vec::new();
        for i in 0..4 {
            ids.push(s.insert(0, i, note()).unwrap());
        }
        s.set_dynamic(ids[0], Some(Dynamic::P)).unwrap();
        s.set_dynamic(ids[2], Some(Dynamic::F)).unwrap();
        s.add_connection(Connection {
            start: ids[0],
            end: ids[2],
            kind: ConnectionKind::Crescendo,
        })
        .unwrap();
        s.add_connection(Connection {
            start: ids[2],
            end: ids[3],
            kind: ConnectionKind::Diminuendo,
        })
        .unwrap();
        let levels = DynamicEnvelope::new(&s, ids[0].part, ids[0].staff).unwrap();
        assert_eq!(
            [
                levels.level_at(0),
                levels.level_at(48),
                levels.level_at(96),
                levels.level_at(144),
                levels.level_at(189)
            ],
            [56, 78, 100, 72, 72]
        );
        s.set_dynamic(ids[1], Some(Dynamic::Mf)).unwrap();
        let levels = DynamicEnvelope::new(&s, ids[0].part, ids[0].staff).unwrap();
        assert_eq!(levels.level_at(24), 72);
        assert_eq!(levels.level_at(72), 94);
        let lower = DynamicEnvelope::new(&s, ids[0].part, StaffId::Lower).unwrap();
        assert_eq!(lower.level_at(48), 88);
    }
}
