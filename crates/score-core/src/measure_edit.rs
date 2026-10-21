//! Structural edits remap every bar-indexed relationship as one operation.
use super::*;
use std::collections::BTreeMap;

fn shift_map<T>(
    map: BTreeMap<usize, T>,
    remap: impl Fn(usize) -> Option<usize>,
) -> BTreeMap<usize, T> {
    map.into_iter()
        .filter_map(|(bar, value)| Some((remap(bar)?, value)))
        .collect()
}
impl Score {
    fn remap_measures(&mut self, remap: impl Fn(usize) -> Option<usize> + Copy) {
        let element = |id: ElementId| {
            Some(ElementId {
                measure: remap(id.measure)?,
                ..id
            })
        };
        self.remap_comments(element);
        self.rewrite_connections(|mark| {
            Some(Connection {
                start: element(mark.start)?,
                end: element(mark.end)?,
                ..mark
            })
        });
        self.piano_arpeggios = self
            .piano_arpeggios
            .iter()
            .filter_map(|mark| {
                Some(PianoArpeggio {
                    upper: element(mark.upper)?,
                    lower: element(mark.lower)?,
                    ..*mark
                })
            })
            .collect();
        for part in &mut self.parts {
            part.divisi_changes = shift_map(std::mem::take(&mut part.divisi_changes), remap);
            for map in part.clefs.values_mut() {
                map.changes = shift_map(std::mem::take(&mut map.changes), remap);
            }
        }
        self.time_changes = shift_map(std::mem::take(&mut self.time_changes), remap);
        self.key_changes = shift_map(std::mem::take(&mut self.key_changes), remap);
        self.tempo_changes = shift_map(std::mem::take(&mut self.tempo_changes), remap);
        self.rehearsals = shift_map(std::mem::take(&mut self.rehearsals), remap);
        self.form = shift_map(std::mem::take(&mut self.form), remap);
        self.endings = self
            .endings
            .iter()
            .filter_map(|ending| {
                let kept: Vec<_> = (ending.start..=ending.end).filter_map(remap).collect();
                Some(Ending {
                    start: *kept.first()?,
                    end: *kept.last()?,
                    numbers: ending.numbers.clone(),
                })
            })
            .collect();
        self.bar_offsets = shift_map(std::mem::take(&mut self.bar_offsets), remap);
        self.prune_bar_offsets();
    }
    /// Insert a blank bar before `index` in every staff. Existing music keeps its signatures.
    pub fn insert_measure(&mut self, index: usize) -> Result<(), ScoreError> {
        if index > self.measures().len() {
            return Err(ScoreError("El compás no existe"));
        }
        if self.measures().len() >= MAX_MEASURES {
            return Err(ScoreError("Se alcanzó el límite de compases"));
        }
        for part in &mut self.parts {
            for staff in &mut part.staves {
                staff.measures.insert(index, Measure::default());
            }
        }
        self.remap_measures(|bar| Some(if bar >= index { bar + 1 } else { bar }));
        self.prune_connections();
        Ok(())
    }
    /// Remove the range from all staves. The first retained bar keeps its effective signatures.
    /// Deleting everything leaves one empty bar with the score's initial signatures.
    pub fn remove_measures(&mut self, start: usize, count: usize) -> Result<(), ScoreError> {
        let len = self.measures().len();
        let end = start
            .checked_add(count)
            .filter(|end| count > 0 && start < len && *end <= len)
            .ok_or(ScoreError("Rango de compases inválido"))?;
        let carry = (end < len).then(|| {
            (
                self.time_at(end),
                self.key_signature_at(end),
                self.tempo_at(end).clone(),
            )
        });
        let divisions: Vec<_> = self.parts.iter().map(|part| part.divisi_at(end)).collect();
        let clefs: Vec<_> = self
            .parts
            .iter()
            .map(|part| {
                [StaffId::Upper, StaffId::Lower]
                    .into_iter()
                    .filter(|staff| *staff == StaffId::Upper || part.instrument.is_grand_staff())
                    .map(|staff| (staff, part.clef_at(staff, end)))
                    .collect::<Vec<_>>()
            })
            .collect();
        for part in &mut self.parts {
            for staff in &mut part.staves {
                for _ in start..end {
                    staff.measures.remove(start);
                }
                if staff.measures.is_empty() {
                    staff.measures.push_back(Measure::default());
                }
            }
        }
        self.remap_measures(|bar| {
            if bar < start {
                Some(bar)
            } else if bar >= end {
                Some(bar - count)
            } else {
                None
            }
        });
        for (part, divided) in self.parts.iter_mut().zip(divisions) {
            if end < len && part.divisi_at(start) != divided {
                part.divisi_changes
                    .entry(start)
                    .or_default()
                    .insert(0, divided);
            }
        }
        if let Some((time, key, tempo)) = carry {
            for (part, clefs) in self.parts.iter_mut().zip(clefs) {
                for (staff, clef) in clefs {
                    if start == 0
                        && let Some(map) = part.clefs.get_mut(&staff)
                    {
                        // A later change shifted onto bar zero becomes the initial clef.
                        map.changes.remove(&0);
                    }
                    if part.clef_at(staff, start) != clef {
                        let map = part.clefs.entry(staff).or_default();
                        if start == 0 {
                            map.initial = Some(clef);
                        } else {
                            map.changes.insert(start, clef);
                        }
                    }
                }
            }
            if start == 0 {
                self.time = time;
                self.key_signature = key;
                self.tempo = tempo;
                self.time_changes.remove(&0);
                self.key_changes.remove(&0);
                self.tempo_changes.remove(&0);
            } else {
                if self.time_at(start) != time {
                    self.time_changes.insert(start, time);
                }
                if self.key_signature_at(start) != key {
                    self.key_changes.insert(start, key);
                }
                if self.tempo_at(start) != &tempo {
                    self.tempo_changes.insert(start, tempo);
                }
            }
        }
        self.prune_connections();
        Ok(())
    }
    /// Replace exactly one chosen chord tone, preserving the event's identity and duration.
    pub fn replace_pitch(
        &mut self,
        id: ElementId,
        index: usize,
        pitch: Pitch,
    ) -> Result<(), ScoreError> {
        let old = self
            .event(id)
            .ok_or(ScoreError("Selecciona una nota o silencio"))?
            .pitches;
        let pitches = if old.is_empty() {
            Pitches::single(Some(pitch))
        } else {
            old.replaced(index, pitch)?
        };
        self.set_pitches(id, pitches)
    }
    /// Erase sound without removing musical time. Dynamic, technique and fermata survive.
    pub fn erase(&mut self, id: ElementId) -> Result<(), ScoreError> {
        self.event(id).ok_or(ScoreError("La nota no existe"))?;
        let event = &mut Arc::make_mut(
            &mut self.part_mut(id.part)?.staff_mut(id.staff)?.measures[id.measure].events,
        )[id.event];
        event.pitches = Pitches::default();
        event.notation.arpeggio = None;
        event.notation.harmonic = None;
        event.notation.articulation = None;
        event.notation.offsets.harmonic = None;
        event.notation.offsets.articulation = None;
        event.notation.tremolo = None;
        self.prune_connections();
        Ok(())
    }
}
