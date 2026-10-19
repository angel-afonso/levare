//! Portable musical passages, independent of the operating system clipboard.
use super::*;
use score_core::{
    Accidental, Articulation, Connection, ConnectionKind, Dynamic, KeySignature, Notation, Pitches,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub const MAX_CLIPBOARD_BYTES: usize = 16 * 1024 * 1024;
const MAX_EVENTS: usize = 65_536;
const MAX_TICKS: u64 = score_core::MAX_MEASURES as u64 * 3072;
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MusicalClipboard {
    pub format: String,
    pub version: u32,
    pub key: KeySignature,
    pub initial_dynamic: Dynamic,
    #[serde(default)]
    pub initial_technique: score_core::PlayingTechnique,
    pub events: Vec<ClipEvent>,
    pub links: Vec<ClipLink>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClipEvent {
    pub offset: u64,
    pub event: Event,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClipLink {
    pub start: usize,
    pub end: usize,
    pub kind: ConnectionKind,
}
impl MusicalClipboard {
    pub fn upgraded(mut self) -> Result<Self, ScoreError> {
        if (1..=3).contains(&self.version) {
            for item in &mut self.events {
                if item.offset % 4 != 0
                    || item.event.duration.is_dotted()
                    || item.event.duration.is_triplet()
                {
                    return Err(ScoreError("Ritmo inválido en un portapapeles antiguo"));
                }
                item.offset = item
                    .offset
                    .checked_mul(3)
                    .ok_or(ScoreError("Pasaje demasiado largo"))?;
            }
            self.version = 4;
        }
        self.validate()?;
        Ok(self)
    }

    pub fn validate(&self) -> Result<(), ScoreError> {
        if self.format != "partitura-clipboard" || !(1..=4).contains(&self.version) {
            return Err(ScoreError(
                "El portapapeles no contiene música compatible de Partitura",
            ));
        }
        if self.events.is_empty() || self.events.len() > MAX_EVENTS || self.links.len() > 4096 {
            return Err(ScoreError(
                "Cantidad de elementos del portapapeles inválida",
            ));
        }
        if self.version < 4 {
            return self.clone().upgraded().map(|_| ());
        }
        let mut end = 0;
        for (i, item) in self.events.iter().enumerate() {
            item.event.pitches.validate()?;
            if (i == 0 && item.offset != 0)
                || item.offset < end
                || item.offset % 2 != 0
                || (item.event.pitches.is_empty() && item.event.notation.articulation.is_some())
            {
                return Err(ScoreError("Pasaje del portapapeles inválido"));
            }
            end = item
                .offset
                .checked_add(u64::from(item.event.duration.ticks()))
                .filter(|n| *n <= MAX_TICKS)
                .ok_or(ScoreError("El pasaje es demasiado largo"))?;
        }
        for link in &self.links {
            if link.start >= link.end || link.end >= self.events.len() {
                return Err(ScoreError(
                    "Extremos de ligadura inválidos en el portapapeles",
                ));
            }
        }
        Ok(())
    }
    pub fn summary(&self) -> String {
        format!("Partitura · {} elementos musicales", self.events.len())
    }
}
fn dynamic_before(score: &Score, cursor: Cursor) -> Dynamic {
    let mut dynamic = Dynamic::Mf;
    let staff = score
        .part(cursor.part)
        .unwrap()
        .staff(cursor.staff)
        .unwrap();
    'marks: for (measure, bar) in staff.measures().iter().enumerate().take(cursor.measure + 1) {
        for (index, (_, event)) in bar.iter().enumerate() {
            if measure == cursor.measure && index >= cursor.index {
                break 'marks;
            }
            if let Some(mark) = event.notation.dynamic {
                dynamic = mark
            }
        }
    }
    let tick = cursor.measure as u64 * u64::from(score.time().capacity())
        + staff.measures()[cursor.measure]
            .iter()
            .take(cursor.index)
            .map(|(_, e)| u64::from(e.duration.ticks()))
            .sum::<u64>();
    if score.connections().iter().any(|c| {
        c.kind.is_hairpin() && c.start.part == cursor.part && c.start.staff == cursor.staff
    }) {
        Dynamic::nearest(
            score_core::DynamicEnvelope::new(score, cursor.part, cursor.staff)
                .unwrap()
                .level_before(tick),
        )
    } else {
        dynamic
    }
}
impl Editor {
    pub fn copy_selection(&self) -> Result<MusicalClipboard, ScoreError> {
        let ids = self.selected_ids();
        let first = *ids.first().ok_or(ScoreError(
            "Selecciona notas, acordes o silencios para copiar",
        ))?;
        if ids.len() > MAX_EVENTS {
            return Err(ScoreError("Selecciona un pasaje más corto para copiar"));
        }
        let score = self.score();
        let base = score.event_tick(first).unwrap();
        let lookup: HashMap<_, _> = ids
            .iter()
            .enumerate()
            .map(|(index, id)| (*id, index))
            .collect();
        let index = score
            .part(first.part)
            .unwrap()
            .staff(first.staff)
            .unwrap()
            .measures()[first.measure]
            .index_of(first.event)
            .unwrap();
        let result = MusicalClipboard {
            format: "partitura-clipboard".into(),
            version: 4,
            initial_technique: score.technique_at(first).unwrap(),
            key: score.key_signature(),
            initial_dynamic: dynamic_before(
                score,
                Cursor {
                    part: first.part,
                    staff: first.staff,
                    measure: first.measure,
                    index,
                },
            ),
            events: ids
                .iter()
                .map(|id| ClipEvent {
                    offset: score.event_tick(*id).unwrap() - base,
                    event: *score.event(*id).unwrap(),
                })
                .collect(),
            links: score
                .connections()
                .iter()
                .filter_map(|link| {
                    Some(ClipLink {
                        start: *lookup.get(&link.start)?,
                        end: *lookup.get(&link.end)?,
                        kind: link.kind,
                    })
                })
                .collect(),
        };
        result.validate()?;
        Ok(result)
    }
    /// Insert after the selection (or at the empty cursor), without overwriting
    /// existing music. The whole operation, including new bars and ties, is atomic.
    pub fn paste(&mut self, clip: &MusicalClipboard) -> Result<usize, ScoreError> {
        let upgraded;
        let clip = if clip.version < 4 {
            upgraded = clip.clone().upgraded()?;
            &upgraded
        } else {
            clip
        };
        clip.validate()?;
        let mut next = self.state.clone();
        if let Some(id) = self.selected_ids().last().copied() {
            next.cursor = Cursor {
                part: id.part,
                staff: id.staff,
                measure: id.measure,
                index: next
                    .score
                    .part(id.part)
                    .unwrap()
                    .staff(id.staff)
                    .unwrap()
                    .measures()[id.measure]
                    .index_of(id.event)
                    .unwrap()
                    + 1,
            };
        }
        let capacity = u64::from(next.score.time().capacity());
        let plain_pieces = RhythmPieces::new(Duration::ALL.to_vec(), capacity as usize);
        let triplet_pieces = RhythmPieces::new(
            Duration::ALL
                .into_iter()
                .map(|d| d.with_triplet(true))
                .collect(),
            capacity as usize,
        );
        let gap_pieces = RhythmPieces::new(Duration::all_modified(), capacity as usize);
        let prefix: u64 = next
            .score
            .part(next.cursor.part)
            .unwrap()
            .staff(next.cursor.staff)
            .unwrap()
            .measures()[next.cursor.measure]
            .iter()
            .take(next.cursor.index)
            .map(|(_, event)| u64::from(event.duration.ticks()))
            .sum();
        let base = next.cursor.measure as u64 * capacity + prefix;
        let span = clip.events.last().unwrap().offset
            + u64::from(clip.events.last().unwrap().event.duration.ticks());
        if (base + span - 1) / capacity >= score_core::MAX_MEASURES as u64 {
            return Err(ScoreError("El pegado supera el límite de compases"));
        }
        let target_dynamic = dynamic_before(&next.score, next.cursor);
        let mut target_technique = score_core::PlayingTechnique::Arco;
        let staff = next
            .score
            .part(next.cursor.part)
            .unwrap()
            .staff(next.cursor.staff)
            .unwrap();
        for (measure, bar) in staff
            .measures()
            .iter()
            .enumerate()
            .take(next.cursor.measure + 1)
        {
            for (_, event) in bar.iter().take(if measure == next.cursor.measure {
                next.cursor.index
            } else {
                bar.len()
            }) {
                if let Some(mark) = event.notation.technique {
                    target_technique = mark;
                }
            }
        }
        let strings = next
            .score
            .part(next.cursor.part)
            .unwrap()
            .instrument()
            .family()
            == score_core::InstrumentFamily::Strings;
        if !strings
            && (clip.initial_technique == score_core::PlayingTechnique::Pizzicato
                || clip
                    .events
                    .iter()
                    .any(|e| e.event.notation.technique.is_some()))
        {
            return Err(ScoreError(
                "Este pasaje contiene pizzicato o arco; pégalo en un instrumento de cuerda con arco",
            ));
        }
        let mut tick = base;
        let mut all_ids = Vec::new();
        let mut mapped = Vec::new();
        let mut ties = Vec::new();
        for (index, item) in clip.events.iter().enumerate() {
            while tick < base + item.offset {
                let duration = gap_pieces
                    .choose((base + item.offset - tick).min(capacity - tick % capacity))?;
                let rest = Event {
                    pitches: Pitches::default(),
                    duration,
                    notation: Notation::default(),
                };
                let id = insert_piece(&mut next, tick, capacity, rest)?;
                all_ids.push(id);
                tick += u64::from(duration.ticks());
            }
            let mut event = item.event;
            event.pitches = Pitches::new(
                &event
                    .pitches
                    .iter()
                    .map(|pitch| {
                        if pitch.accidental().is_some()
                            || clip.key.accidental(pitch.step)
                                == next.score.key_signature().accidental(pitch.step)
                        {
                            pitch
                        } else {
                            pitch.with_accidental(Some(
                                clip.key
                                    .accidental(pitch.step)
                                    .unwrap_or(Accidental::Natural),
                            ))
                        }
                    })
                    .collect::<Vec<_>>(),
            )?;
            if index == 0
                && event.notation.dynamic.is_none()
                && clip.initial_dynamic != target_dynamic
            {
                event.notation.dynamic = Some(clip.initial_dynamic);
            }
            if index == 0
                && strings
                && event.notation.technique.is_none()
                && clip.initial_technique != target_technique
            {
                event.notation.technique = Some(clip.initial_technique);
            }
            let mut remaining = u64::from(event.duration.ticks());
            let mut first = None;
            let mut last = None;
            while remaining > 0 {
                let available = capacity - tick % capacity;
                let duration = if first.is_none() && remaining <= available {
                    event.duration
                } else {
                    let pieces = if event.duration.is_triplet() {
                        &triplet_pieces
                    } else {
                        &plain_pieces
                    };
                    pieces.choose(remaining.min(available))?
                };
                let length = u64::from(duration.ticks());
                let is_first = first.is_none();
                let is_last = remaining == length;
                let mut notation = event.notation;
                if !is_first {
                    notation.dynamic = None;
                    notation.technique = None;
                }
                if let Some(mark) = notation.articulation {
                    let belongs = if matches!(mark, Articulation::Staccato | Articulation::Tenuto) {
                        is_last
                    } else {
                        is_first
                    };
                    if !belongs {
                        notation.articulation = None;
                    }
                }
                let id = insert_piece(
                    &mut next,
                    tick,
                    capacity,
                    Event {
                        duration,
                        notation,
                        ..event
                    },
                )?;
                if let Some(previous) = last
                    && !event.pitches.is_empty()
                {
                    ties.push(Connection {
                        start: previous,
                        end: id,
                        kind: ConnectionKind::Tie,
                    });
                }
                first.get_or_insert(id);
                last = Some(id);
                all_ids.push(id);
                tick += length;
                remaining -= length;
            }
            mapped.push((first.unwrap(), last.unwrap()));
        }
        for tie in ties {
            next.score.add_connection(tie)?;
        }
        for link in &clip.links {
            let start = if link.kind == ConnectionKind::Tie {
                mapped[link.start].1
            } else {
                mapped[link.start].0
            };
            let end = if link.kind == ConnectionKind::Tie || link.kind.is_hairpin() {
                mapped[link.end].0
            } else {
                mapped[link.end].1
            };
            next.score.add_connection(Connection {
                start,
                end,
                kind: link.kind,
            })?;
        }
        next.score.validate()?;
        next.range_anchor = all_ids.first().copied();
        next.selection = all_ids.last().copied();
        next.tone = 0;
        self.commit(next);
        Ok(clip.events.len())
    }
}
/// A bounded decomposition table prevents greedy dead ends and rejects rhythms
/// that cannot be split at this destination before committing any changes.
struct RhythmPieces {
    durations: Vec<Duration>,
    representable: Vec<bool>,
}
impl RhythmPieces {
    fn new(mut durations: Vec<Duration>, capacity: usize) -> Self {
        durations.sort_by_key(|d| (std::cmp::Reverse(d.ticks()), d.is_triplet(), d.is_dotted()));
        let mut representable = vec![false; capacity + 1];
        representable[0] = true;
        for tick in 1..=capacity {
            representable[tick] = durations.iter().any(|d| {
                usize::from(d.ticks()) <= tick && representable[tick - usize::from(d.ticks())]
            });
        }
        Self {
            durations,
            representable,
        }
    }
    fn choose(&self, ticks: u64) -> Result<Duration, ScoreError> {
        let ticks = ticks as usize;
        self.durations.iter().copied().find(|d| usize::from(d.ticks()) <= ticks && self.representable.get(ticks - usize::from(d.ticks())) == Some(&true))
            .ok_or(ScoreError("Este ritmo no se puede dividir en ese punto. Elige un compás vacío o un destino alineado con el pasaje"))
    }
}
fn insert_piece(
    next: &mut Snapshot,
    tick: u64,
    capacity: u64,
    event: Event,
) -> Result<ElementId, ScoreError> {
    let measure = (tick / capacity) as usize;
    while next.score.measures().len() <= measure {
        next.score.append_measure()?;
    }
    if next.cursor.measure != measure {
        next.cursor.measure = measure;
        next.cursor.index = 0;
    }
    let id=next.score.insert_on(next.cursor.part,next.cursor.staff,measure,next.cursor.index,event).map_err(|_|ScoreError("El pasaje no cabe sin reemplazar notas. Elige un compás vacío o un punto con espacio suficiente"))?;
    next.cursor.index += 1;
    Ok(id)
}
