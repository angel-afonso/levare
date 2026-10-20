//! Move one rhythmic event without shifting or overwriting neighboring music.
use super::*;
use std::collections::{BTreeMap, HashMap};

#[derive(Clone, Copy)]
struct Item {
    tick: u16,
    id: Option<ElementId>,
    event: Event,
}

impl Score {
    /// Move a whole note/chord in its original staff and voice. Empty time is
    /// filled with rests; an occupied equal-duration slot is exchanged. Other
    /// sounding events retain their time. All validation precedes publication.
    pub fn move_event(
        &mut self,
        id: ElementId,
        measure: usize,
        tick: u16,
    ) -> Result<ElementId, ScoreError> {
        let event = *self.event(id).ok_or(ScoreError("La nota no existe"))?;
        if event.pitches.is_empty() {
            return Err(ScoreError("Selecciona una nota o acorde para mover"));
        }
        let voice = self.voice_of(id).ok_or(ScoreError("La voz no existe"))?;
        let source_tick =
            (self.event_tick(id).unwrap() - self.measure_start_tick(id.measure)) as u16;
        if measure >= self.measures().len()
            || tick
                .checked_add(event.duration.ticks())
                .is_none_or(|end| end > self.time_at(measure).capacity())
        {
            return Err(ScoreError("La figura no cabe en ese tiempo del compás"));
        }
        if measure == id.measure && tick == source_tick {
            return Ok(id);
        }
        let end = tick + event.duration.ticks();
        let staff = self.part(id.part).unwrap().staff(id.staff).unwrap();
        let overlaps: Vec<_> = staff.measures()[measure]
            .timed_iter()
            .filter(|(v, at, key, e)| {
                *v == voice
                    && !(measure == id.measure && *key == id.event)
                    && !e.pitches.is_empty()
                    && *at < end
                    && *at + e.duration.ticks() > tick
            })
            .collect();
        let swap = match overlaps.as_slice() {
            [] => None,
            [(_, at, key, other)]
                if *at == tick && other.duration.ticks() == event.duration.ticks() =>
            {
                Some((
                    ElementId {
                        measure,
                        event: *key,
                        ..id
                    },
                    **other,
                ))
            }
            _ => {
                return Err(ScoreError(
                    "El destino contiene música de otra duración; elige un silencio o una figura de igual duración",
                ));
            }
        };
        let mut changes: BTreeMap<usize, Vec<Item>> = BTreeMap::new();
        let mut lengths = BTreeMap::new();
        for bar in [id.measure, measure] {
            if changes.contains_key(&bar) {
                continue;
            }
            lengths.insert(bar, staff.measures()[bar].voice_ticks(voice));
            let mut items = Vec::new();
            for (v, at, key, value) in staff.measures()[bar]
                .timed_iter()
                .filter(|(v, _, _, _)| *v == voice)
            {
                let _ = v;
                let old = ElementId {
                    measure: bar,
                    event: key,
                    ..id
                };
                if old == id || swap.is_some_and(|(other, _)| other == old) {
                    continue;
                }
                let right = at + value.duration.ticks();
                if bar == measure && value.pitches.is_empty() && at < end && right > tick {
                    if value.notation != Notation::default()
                        || self
                            .connections
                            .iter()
                            .any(|link| link.start == old || link.end == old)
                    {
                        return Err(ScoreError(
                            "El silencio de destino tiene indicaciones; elige otro lugar para conservarlas",
                        ));
                    }
                    if at < tick {
                        push_rests(&mut items, at, tick - at)?;
                    }
                    if right > end {
                        push_rests(&mut items, end, right - end)?;
                    }
                } else {
                    items.push(Item {
                        tick: at,
                        id: Some(old),
                        event: *value,
                    });
                }
            }
            changes.insert(bar, items);
        }
        let mut moved = event;
        moved.pitches = moved_pitches(
            event.pitches,
            self.written_key_at(id.part, id.measure),
            self.written_key_at(id.part, measure),
        )?;
        changes.get_mut(&measure).unwrap().push(Item {
            tick,
            id: Some(id),
            event: moved,
        });
        if let Some((other, mut event)) = swap {
            event.pitches = moved_pitches(
                event.pitches,
                self.written_key_at(id.part, measure),
                self.written_key_at(id.part, id.measure),
            )?;
            changes.get_mut(&id.measure).unwrap().push(Item {
                tick: source_tick,
                id: Some(other),
                event,
            });
        }
        let mut next = self.clone();
        let mut remap = HashMap::new();
        for (bar, mut items) in changes {
            items.sort_by_key(|item| item.tick);
            let mut filled = Vec::new();
            let mut at = 0;
            for item in items {
                if item.tick < at {
                    return Err(ScoreError("La figura se solapa con otra nota"));
                }
                push_rests(&mut filled, at, item.tick - at)?;
                at = item.tick + item.event.duration.ticks();
                filled.push(item);
            }
            push_rests(&mut filled, at, lengths[&bar].saturating_sub(at))?;
            let target = &mut next.part_mut(id.part)?.staff_mut(id.staff)?.measures[bar];
            let old: Vec<_> = target.voice_order(voice).iter().copied().collect();
            let mut order = Vector::new();
            let events = Arc::make_mut(&mut target.events);
            for item in filled {
                let key = if let Some(old) = item.id.filter(|old| old.measure == bar) {
                    events[old.event] = item.event;
                    old.event
                } else {
                    events.insert(item.event)
                };
                if let Some(old) = item.id {
                    remap.insert(
                        old,
                        ElementId {
                            measure: bar,
                            event: key,
                            ..id
                        },
                    );
                }
                order.push_back(key);
            }
            for key in old {
                if !order.contains(&key) {
                    events.remove(key);
                }
            }
            *target.voice_order_mut(voice) = order;
        }
        next.connections = next
            .connections
            .iter()
            .map(|c| Connection {
                start: remap.get(&c.start).copied().unwrap_or(c.start),
                end: remap.get(&c.end).copied().unwrap_or(c.end),
                ..*c
            })
            .collect();
        next.piano_arpeggios = next
            .piano_arpeggios
            .iter()
            .map(|a| PianoArpeggio {
                upper: remap.get(&a.upper).copied().unwrap_or(a.upper),
                lower: remap.get(&a.lower).copied().unwrap_or(a.lower),
                ..*a
            })
            .collect();
        // A roll that no longer spans simultaneous hands becomes local where
        // its chord can still carry it. Incompatible ties/spans are pruned.
        for arp in next.piano_arpeggios.clone() {
            if next.event_tick(arp.upper) != next.event_tick(arp.lower) {
                for anchor in [arp.upper, arp.lower] {
                    if next.event(anchor).is_some_and(|e| e.pitches.len() >= 2) {
                        Arc::make_mut(
                            &mut next
                                .part_mut(anchor.part)?
                                .staff_mut(anchor.staff)?
                                .measures[anchor.measure]
                                .events,
                        )[anchor.event]
                            .notation
                            .arpeggio = Some(arp.kind);
                    }
                }
            }
        }
        next.prune_connections();
        next.validate()?;
        let moved = remap[&id];
        *self = next;
        Ok(moved)
    }
}

fn moved_pitches(
    pitches: Pitches,
    source: KeySignature,
    destination: KeySignature,
) -> Result<Pitches, ScoreError> {
    if source == destination {
        return Ok(pitches);
    }
    Pitches::new(
        &pitches
            .iter()
            .map(|pitch| {
                let sound = pitch.in_key(source).alteration();
                let default = destination
                    .accidental(pitch.step)
                    .map_or(0, Accidental::semitones);
                pitch.with_accidental(if sound == default {
                    None
                } else {
                    Some(match sound {
                        -1 => Accidental::Flat,
                        1 => Accidental::Sharp,
                        _ => Accidental::Natural,
                    })
                })
            })
            .collect::<Vec<_>>(),
    )
}

fn push_rests(items: &mut Vec<Item>, start: u16, ticks: u16) -> Result<(), ScoreError> {
    if ticks == 0 {
        return Ok(());
    }
    let mut values: Vec<_> = Duration::ALL
        .into_iter()
        .flat_map(|d| [d, d.with_dotted(true), d.with_triplet(true)])
        .collect();
    values.sort_by_key(|d| (std::cmp::Reverse(d.ticks()), d.is_triplet(), d.is_dotted()));
    values.dedup_by_key(|d| d.ticks());
    let mut best = vec![None; usize::from(ticks) + 1];
    best[0] = Some((0usize, Duration::Quarter));
    for n in 1..=ticks {
        for &duration in &values {
            if duration.ticks() <= n
                && let Some((count, _)) = best[usize::from(n - duration.ticks())]
            {
                let at = &mut best[usize::from(n)];
                if at.is_none_or(|(old, _)| count + 1 < old) {
                    *at = Some((count + 1, duration));
                }
            }
        }
    }
    let mut remaining = ticks;
    let mut at = start;
    while remaining > 0 {
        let (_, duration) = best[usize::from(remaining)].ok_or(ScoreError(
            "Ese desplazamiento crea un silencio que todavía no se puede representar",
        ))?;
        items.push(Item {
            tick: at,
            id: None,
            event: Event {
                pitches: Pitches::default(),
                duration,
                notation: Notation::default(),
            },
        });
        at += duration.ticks();
        remaining -= duration.ticks();
    }
    Ok(())
}

#[cfg(test)]
mod tests;
