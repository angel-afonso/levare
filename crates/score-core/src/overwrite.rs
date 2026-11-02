//! Write into occupied time while preserving unaffected event identities.
use super::*;
use crate::rhythm::{self, TupletSpan};
use std::collections::{BTreeMap, HashMap};

#[derive(Clone, Copy)]
struct Item {
    tick: u32,
    old: Option<ElementId>,
    event: Event,
    piece: Option<usize>,
}

#[derive(Clone, Copy)]
struct Piece {
    measure: usize,
    tick: u32,
    duration: Duration,
}

const NO_ROOM_IN_TUPLET: &str = "La figura no cabe en el grupillo";
const INSIDE_A_TUPLET: &str =
    "Dentro de un grupillo solo caben sus figuras; empieza en su primera nota para sustituirlo";
const CUTS_A_TUPLET: &str =
    "La figura corta un grupillo; empieza en su primera nota para sustituirlo";
const TUPLET_ACROSS_BAR: &str = "Un grupillo no puede cruzar la barra de compás";

fn rests(
    items: &mut Vec<Item>,
    time: TimeSignature,
    start: u32,
    ticks: u32,
    spans: &[TupletSpan],
) -> Result<(), ScoreError> {
    for (tick, duration) in rhythm::rests_for_gap(time, start, ticks, spans)? {
        items.push(Item {
            tick,
            old: None,
            event: Event {
                pitches: Pitches::default(),
                duration,
                notation: Notation::default(),
            },
            piece: None,
        });
    }
    Ok(())
}

impl Score {
    /// Replace one voice's occupied interval, splitting across bars when needed.
    pub fn overwrite(
        &mut self,
        part: PartId,
        staff: StaffId,
        measure: usize,
        voice: Voice,
        tick: u32,
        event: Event,
    ) -> Result<Vec<ElementId>, ScoreError> {
        self.part(part)
            .ok_or(ScoreError("El instrumento no existe"))?
            .staff(staff)
            .ok_or(ScoreError("El pentagrama no existe"))?
            .measures
            .get(measure)
            .ok_or(ScoreError("El compás no existe"))?;
        let cap = self.time_at(measure).capacity();
        if tick >= cap {
            return Err(ScoreError("Posición de entrada inválida"));
        }
        self.validate_event_pitches(part, measure, event)?;
        if event.notation.arpeggio.is_some() && event.pitches.len() < 2 {
            return Err(ScoreError(
                "El arpegio requiere un acorde de al menos dos notas",
            ));
        }
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

        let mut next = self.clone();
        let mut pieces = Vec::new();
        let mut remaining = event.duration.ticks();
        let mut bar = measure;
        let mut at = tick;
        while remaining > 0 {
            if bar == next.measures().len() {
                next.append_measure()?;
            }
            let capacity = next.time_at(bar).capacity();
            let chunk = remaining.min(capacity - at);
            if chunk < remaining && event.duration.tuplet().is_some() {
                return Err(ScoreError(TUPLET_ACROSS_BAR));
            }
            let mut durations = if pieces.is_empty() && chunk == remaining {
                vec![event.duration]
            } else {
                event_move::notatable(chunk)?
            };
            if !event.pitches.is_empty() {
                durations.sort_by_key(|duration| std::cmp::Reverse(duration.ticks()));
            }
            for duration in durations {
                pieces.push(Piece {
                    measure: bar,
                    tick: at,
                    duration,
                });
                at += duration.ticks();
            }
            remaining -= chunk;
            bar += 1;
            at = 0;
        }

        let first = pieces[0];
        let last_index = pieces.len() - 1;
        let mut by_bar: BTreeMap<usize, Vec<Item>> = BTreeMap::new();
        let mut old_at_start = HashMap::new();
        for (index, piece) in pieces.iter().enumerate() {
            let mut written = event;
            written.duration = piece.duration;
            if index != 0 {
                written.pitches = event_move::moved_pitches(
                    event.pitches,
                    self.written_key_at(part, measure),
                    next.written_key_at(part, piece.measure),
                )?;
                written.notation = Notation::default();
                written.notation.harmonic = event.notation.harmonic;
                written.notation.offsets.harmonic = event.notation.offsets.harmonic;
            }
            if pieces.len() > 1 {
                // The fermata and the breath after the note stay on its end.
                let last = index == last_index;
                written.notation.fermata = last && event.notation.fermata;
                written.notation.pause = event.notation.pause.filter(|_| last);
                written.notation.offsets.fermata = event.notation.offsets.fermata.filter(|_| last);
                written.notation.offsets.pause = event.notation.offsets.pause.filter(|_| last);
            }
            by_bar.entry(piece.measure).or_default().push(Item {
                tick: piece.tick,
                old: None,
                event: written,
                piece: Some(index),
            });
        }
        let mut written_ids = vec![None; pieces.len()];
        for (&bar, items) in &mut by_bar {
            let time = next.time_at(bar);
            let start = items[0].tick;
            let mut zone_end =
                items.last().unwrap().tick + items.last().unwrap().event.duration.ticks();
            let source = &next.part(part).unwrap().staff(staff).unwrap().measures[bar];
            let groups = source.tuplet_groups(voice);
            let written_tuplet = items[0].event.duration.tuplet();
            // The group the written value continues, and where the gaps the
            // edit leaves are filled with that group's rests.
            let mut inside = None;
            let mut spans = Vec::new();
            let mut fill_to = zone_end;
            match groups.iter().find(|g| g.start <= start && start < g.end) {
                Some(group) if Some(group.tuplet) == written_tuplet => {
                    if zone_end > group.end {
                        return Err(ScoreError(NO_ROOM_IN_TUPLET));
                    }
                    inside = Some(group);
                    spans.push(TupletSpan::from(group));
                }
                // From its first event, another value replaces the group.
                Some(group) if group.start == start => fill_to = fill_to.max(group.end),
                Some(_) => return Err(ScoreError(INSIDE_A_TUPLET)),
                None => {}
            }
            if inside.is_none()
                && let Some(tuplet) = written_tuplet
            {
                // A new group: the rest of it is filled with its rests.
                let end = start + tuplet.span();
                if end > next.time_at(bar).capacity() {
                    return Err(ScoreError(TUPLET_ACROSS_BAR));
                }
                spans.push(TupletSpan { start, end, tuplet });
                fill_to = fill_to.max(end);
            }
            if fill_to > zone_end {
                rests(items, time, zone_end, fill_to - zone_end, &spans)?;
                zone_end = fill_to;
            }
            for group in &groups {
                let covered = start <= group.start && group.end <= zone_end;
                let overlaps = group.start < zone_end && start < group.end;
                if overlaps && !covered && inside != Some(group) {
                    return Err(ScoreError(CUTS_A_TUPLET));
                }
            }
            let mut retained = Vec::new();
            let mut original = None;
            for (_, onset, key, value) in source.timed_iter().filter(|(v, _, _, _)| *v == voice) {
                let old = ElementId {
                    part,
                    staff,
                    measure: bar,
                    event: key,
                };
                let right = onset + value.duration.ticks();
                if right <= start || onset >= zone_end {
                    retained.push(Item {
                        tick: onset,
                        old: Some(old),
                        event: *value,
                        piece: None,
                    });
                    continue;
                }
                if onset == start {
                    original = Some(old);
                    if items[0].event.notation.dynamic.is_none() {
                        items[0].event.notation.dynamic = value.notation.dynamic;
                        items[0].event.notation.offsets.dynamic = value.notation.offsets.dynamic;
                    }
                }
                if onset < start {
                    if !value.pitches.is_empty() {
                        return Err(ScoreError(
                            "La posición cae dentro de una nota; elige el inicio de una figura o un silencio",
                        ));
                    }
                    rests(&mut retained, time, onset, start - onset, &spans)?;
                }
                if right > zone_end {
                    rests(&mut retained, time, zone_end, right - zone_end, &spans)?;
                }
            }
            old_at_start.insert(bar, original);
            items.append(&mut retained);
            items.sort_by_key(|item| item.tick);
            let mut filled = Vec::new();
            let mut cursor = 0;
            for item in items.drain(..) {
                if item.tick < cursor {
                    return Err(ScoreError("La figura se solapa con otra nota"));
                }
                rests(&mut filled, time, cursor, item.tick - cursor, &spans)?;
                cursor = item.tick + item.event.duration.ticks();
                filled.push(item);
            }
            rests(
                &mut filled,
                time,
                cursor,
                // Written bars stay full, as in Sibelius: the rest of the
                // bar is rests that the next notes overwrite.
                time.capacity().saturating_sub(cursor),
                &spans,
            )?;
            let target = &mut next.part_mut(part)?.staff_mut(staff)?.measures[bar];
            let old_keys: Vec<_> = target.voice_order(voice).iter().copied().collect();
            let mut order = Vector::new();
            let events = Arc::make_mut(&mut target.events);
            // The written value takes the place of the event it starts on, so
            // a note given a new duration keeps its identity.
            let mut reuse = original;
            for item in filled {
                let key = if let Some(old) = item.old {
                    events[old.event] = item.event;
                    old.event
                } else if item.piece.is_some()
                    && let Some(old) = reuse.take()
                {
                    events[old.event] = item.event;
                    old.event
                } else {
                    events.insert(item.event)
                };
                if let Some(index) = item.piece {
                    written_ids[index] = Some(ElementId {
                        part,
                        staff,
                        measure: bar,
                        event: key,
                    });
                }
                order.push_back(key);
            }
            for key in old_keys {
                if !order.contains(&key) {
                    events.remove(key);
                }
            }
            *target.voice_order_mut(voice) = order;
        }
        let written: Vec<ElementId> = written_ids.into_iter().map(Option::unwrap).collect();
        let remap: HashMap<_, _> = old_at_start
            .into_iter()
            .filter_map(|(bar, old)| {
                old.map(|old| {
                    let new = written
                        .iter()
                        .copied()
                        .find(|id| id.measure == bar)
                        .unwrap();
                    (old, new)
                })
            })
            .collect();
        next.remap_comments(|old| Some(remap.get(&old).copied().unwrap_or(old)));
        next.rewrite_connections(|connection| {
            Some(Connection {
                start: remap
                    .get(&connection.start)
                    .copied()
                    .unwrap_or(connection.start),
                end: remap
                    .get(&connection.end)
                    .copied()
                    .unwrap_or(connection.end),
                ..connection
            })
        });
        next.piano_arpeggios = next
            .piano_arpeggios
            .iter()
            .map(|arpeggio| PianoArpeggio {
                upper: remap
                    .get(&arpeggio.upper)
                    .copied()
                    .unwrap_or(arpeggio.upper),
                lower: remap
                    .get(&arpeggio.lower)
                    .copied()
                    .unwrap_or(arpeggio.lower),
                ..*arpeggio
            })
            .collect();
        for pair in written.windows(2) {
            if !event.pitches.is_empty() {
                let tie = Connection {
                    start: pair[0],
                    end: pair[1],
                    kind: ConnectionKind::Tie,
                };
                if !next.connections.contains(&tie) {
                    next.connections.push_back(tie);
                }
            }
        }
        next.prune_connections();
        if !event.pitches.is_empty()
            && written.windows(2).any(|pair| {
                !next.connections.contains(&Connection {
                    start: pair[0],
                    end: pair[1],
                    kind: ConnectionKind::Tie,
                })
            })
        {
            return Err(ScoreError("No se pudo ligar la figura entre compases"));
        }
        next.validate()?;
        *self = next;
        debug_assert_eq!(written[0].measure, first.measure);
        Ok(written)
    }
}

impl Score {
    /// Rests after the last event of every voice that has music, up to its
    /// barline, so no bar ends early. An empty voice stays empty: one bar
    /// rest. Whether anything was added.
    pub fn fill_rests(&mut self) -> Result<bool, ScoreError> {
        self.fill_rests_where(|_, _, _, _| true)
    }

    /// `fill_rests` for one voice of one staff, in `bars`.
    pub fn fill_rests_in(
        &mut self,
        part: PartId,
        staff: StaffId,
        voice: Voice,
        bars: std::ops::RangeInclusive<usize>,
    ) -> Result<bool, ScoreError> {
        self.fill_rests_where(|p, s, bar, v| {
            p == part && s == staff && v == voice && bars.contains(&bar)
        })
    }

    /// Removes the plain rests that end a voice in a bar from `from` on, so
    /// music can be placed there as in empty space. Rests with marks, links
    /// or comments, or a note among them, leave the bar as it is.
    pub fn clear_trailing_rests(
        &mut self,
        part: PartId,
        staff: StaffId,
        voice: Voice,
        bar: usize,
        from: u32,
    ) -> Result<(), ScoreError> {
        let measure = &self
            .part(part)
            .ok_or(ScoreError("El instrumento no existe"))?
            .staff(staff)
            .ok_or(ScoreError("El pentagrama no existe"))?
            .measures()[bar];
        let tail: Vec<_> = measure
            .timed_iter()
            .filter(|(v, onset, _, _)| *v == voice && *onset >= from)
            .map(|(_, _, key, event)| (key, *event))
            .collect();
        let anchored = |key| {
            let id = ElementId {
                part,
                staff,
                measure: bar,
                event: key,
            };
            self.connections()
                .iter()
                .any(|c| c.start == id || c.end == id)
                || self.comments().iter().any(|c| c.anchors.contains(&id))
        };
        let plain = tail.iter().all(|(key, event)| {
            event.pitches.is_empty() && event.notation == Notation::default() && !anchored(*key)
        });
        if !plain {
            return Ok(());
        }
        for (key, _) in tail.into_iter().rev() {
            self.remove(ElementId {
                part,
                staff,
                measure: bar,
                event: key,
            })?;
        }
        Ok(())
    }

    fn fill_rests_where(
        &mut self,
        wanted: impl Fn(PartId, StaffId, usize, Voice) -> bool,
    ) -> Result<bool, ScoreError> {
        let mut gaps = Vec::new();
        for part in self.parts() {
            for staff in part.staves() {
                for (bar, measure) in staff.measures().iter().enumerate() {
                    for voice in Voice::ALL {
                        if !wanted(part.id(), staff.id(), bar, voice) {
                            continue;
                        }
                        let used = measure.voice_ticks(voice);
                        let capacity = self.time_at(bar).capacity();
                        if measure.voice_len(voice) == 0 || used >= capacity {
                            continue;
                        }
                        // An open tuplet group is completed with its rests.
                        let spans: Vec<_> = measure
                            .tuplet_groups(voice)
                            .last()
                            .filter(|group| !group.is_complete() && group.span_end() <= capacity)
                            .map(|group| TupletSpan {
                                start: group.start,
                                end: group.span_end(),
                                tuplet: group.tuplet,
                            })
                            .into_iter()
                            .collect();
                        let rests = rhythm::rests_for_gap(
                            self.time_at(bar),
                            used,
                            capacity - used,
                            &spans,
                        )?;
                        gaps.push((part.id(), staff.id(), bar, voice, rests));
                    }
                }
            }
        }
        let filled = !gaps.is_empty();
        for (part, staff, bar, voice, rests) in gaps {
            for (_, duration) in rests {
                let index =
                    self.part(part).unwrap().staff(staff).unwrap().measures()[bar].voice_len(voice);
                let rest = Event {
                    pitches: Pitches::default(),
                    duration,
                    notation: Notation::default(),
                };
                self.insert_in_voice(part, staff, bar, voice, index, rest)?;
            }
        }
        Ok(filled)
    }
}

#[cfg(test)]
mod tests;
