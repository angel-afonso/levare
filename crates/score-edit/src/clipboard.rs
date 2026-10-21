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
    #[serde(default)]
    pub transposition: score_core::Transposition,
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
        if self.version == 4 {
            self.version = 5;
        }
        if self.version == 5 {
            self.version = 6;
        }
        self.validate()?;
        Ok(self)
    }

    pub fn validate(&self) -> Result<(), ScoreError> {
        if self.format != "partitura-clipboard" || !(1..=6).contains(&self.version) {
            return Err(ScoreError(
                "El portapapeles no contiene música compatible de Levare",
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
        self.transposition.validate()?;
        let mut end = 0;
        for (i, item) in self.events.iter().enumerate() {
            item.event.pitches.validate()?;
            item.event
                .validate_harmonic(score_core::Instrument::Violin)?;
            if (i == 0 && item.offset != 0)
                || item.offset < end
                || item.offset % 2 != 0
                || (item.event.pitches.is_empty()
                    && (item.event.notation.articulation.is_some()
                        || item.event.notation.tremolo.is_some()))
                || (item.event.pitches.len() < 2 && item.event.notation.arpeggio.is_some())
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
        format!("Levare · {} elementos musicales", self.events.len())
    }
}
fn dynamic_before(score: &Score, cursor: Cursor) -> Dynamic {
    let staff = score
        .part(cursor.part)
        .unwrap()
        .staff(cursor.staff)
        .unwrap();
    let tick = score.measure_start_tick(cursor.measure)
        + staff.measures()[cursor.measure]
            .voice_iter(cursor.voice)
            .take(cursor.index)
            .map(|(_, e)| u64::from(e.duration.ticks()))
            .sum::<u64>();
    Dynamic::nearest(
        score_core::DynamicEnvelope::new(score, cursor.part, cursor.staff)
            .unwrap()
            .level_before(tick),
    )
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
        let result = MusicalClipboard {
            format: "partitura-clipboard".into(),
            version: 6,
            initial_technique: score.technique_at(first).unwrap(),
            key: score.written_key_at(first.part, first.measure),
            transposition: score.part(first.part).unwrap().transposition(),
            initial_dynamic: Dynamic::nearest(
                score_core::DynamicEnvelope::new(score, first.part, first.staff)
                    .unwrap()
                    .level_at(base),
            ),
            events: ids
                .iter()
                .map(|id| {
                    Ok(ClipEvent {
                        offset: score.event_tick(*id).unwrap() - base,
                        event: {
                            let mut event = *score.event(*id).unwrap();
                            // The passage contains one staff: copy the local roll, without a dangling cross-hand reference.
                            if event.pitches.len() >= 2 {
                                event.notation.arpeggio =
                                    score.arpeggio_at(*id).map(|mark| mark.kind);
                            }
                            if score
                                .part(id.part)
                                .unwrap()
                                .instrument()
                                .percussion_key()
                                .is_some()
                            {
                                event.pitches = score.sounding_pitches(*id)?;
                            }
                            event.pitches = rekey(
                                event.pitches,
                                score.written_key_at(id.part, id.measure),
                                score.written_key_at(first.part, first.measure),
                            )?;
                            event
                        },
                    })
                })
                .collect::<Result<Vec<_>, ScoreError>>()?,
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
                voice: next.score.voice_of(id).unwrap(),
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
                    .voice_index_of(next.cursor.voice, id.event)
                    .unwrap()
                    + 1,
            };
        }
        let capacity = 3072u64; // largest supported bar, for reusable rhythm decomposition tables
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
            .voice_iter(next.cursor.voice)
            .take(next.cursor.index)
            .map(|(_, event)| u64::from(event.duration.ticks()))
            .sum();
        let base = next.score.measure_start_tick(next.cursor.measure) + prefix;
        let span = clip.events.last().unwrap().offset
            + u64::from(clip.events.last().unwrap().event.duration.ticks());
        if next.score.measure_at_tick(base + span - 1).0 >= score_core::MAX_MEASURES {
            return Err(ScoreError("El pegado supera el límite de compases"));
        }
        let target_dynamic = dynamic_before(&next.score, next.cursor);
        let part = next.score.part(next.cursor.part).unwrap();
        let mut techniques = std::collections::BTreeMap::new();
        for staff in part.expression_staves(next.cursor.staff) {
            for (bar, measure) in staff
                .measures()
                .iter()
                .enumerate()
                .take(next.cursor.measure + 1)
            {
                for (_, onset, _, event) in measure.timed_iter() {
                    let tick = next.score.measure_start_tick(bar) + u64::from(onset);
                    if tick < base
                        && let Some(mark) = event.notation.technique
                    {
                        techniques.insert(tick, mark);
                    }
                }
            }
        }
        let target_technique = techniques
            .last_key_value()
            .map_or(score_core::PlayingTechnique::Arco, |(_, mark)| *mark);
        let strings = next
            .score
            .part(next.cursor.part)
            .unwrap()
            .instrument()
            .family()
            == score_core::InstrumentFamily::Strings;
        if !strings
            && (clip.initial_technique == score_core::PlayingTechnique::Pizzicato
                || clip.events.iter().any(|e| {
                    e.event.notation.technique.is_some() || e.event.notation.harmonic.is_some()
                }))
        {
            return Err(ScoreError(
                "Este pasaje contiene técnicas o armónicos de cuerda; pégalo en un instrumento de cuerda con arco",
            ));
        }
        let mut tick = base;
        let mut all_ids = Vec::new();
        let mut mapped = Vec::new();
        let mut ties = Vec::new();
        for (index, item) in clip.events.iter().enumerate() {
            while tick < base + item.offset {
                let duration = gap_pieces
                    .choose((base + item.offset - tick).min(remaining_in_bar(&next.score, tick)))?;
                let rest = Event {
                    pitches: Pitches::default(),
                    duration,
                    notation: Notation::default(),
                };
                let id = insert_piece(&mut next, tick, rest)?;
                all_ids.push(id);
                tick += u64::from(duration.ticks());
            }
            let mut event = item.event;
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
                let available = remaining_in_bar(&next.score, tick);
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
                    notation.offsets.dynamic = None;
                    notation.technique = None;
                    notation.offsets.technique = None;
                    notation.division = None;
                    notation.arpeggio = None;
                }
                // A split note keeps one fermata, over its final fragment.
                if !is_last {
                    notation.fermata = false;
                    notation.offsets.fermata = None;
                }
                if let Some(mark) = notation.articulation {
                    let belongs = if matches!(mark, Articulation::Staccato | Articulation::Tenuto) {
                        is_last
                    } else {
                        is_first
                    };
                    if !belongs {
                        notation.articulation = None;
                        notation.offsets.articulation = None;
                    }
                }
                let target_key = next
                    .score
                    .written_key_at(next.cursor.part, next.score.measure_at_tick(tick).0);
                let target_transposition =
                    next.score.part(next.cursor.part).unwrap().transposition();
                let pitches = if clip.transposition == target_transposition {
                    rekey(event.pitches, clip.key, target_key)?
                } else {
                    let concert = clip.transposition.pitches(event.pitches, clip.key)?;
                    let written = target_transposition
                        .inverse()
                        .pitches(concert, KeySignature::default())?;
                    rekey(written, KeySignature::default(), target_key)?
                };
                let id = insert_piece(
                    &mut next,
                    tick,
                    Event {
                        pitches,
                        duration,
                        notation,
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
        next.divisi_anchor = None;
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
fn insert_piece(next: &mut Snapshot, tick: u64, event: Event) -> Result<ElementId, ScoreError> {
    let measure = next.score.measure_at_tick(tick).0;
    while next.score.measures().len() <= measure {
        next.score.append_measure()?;
    }
    if next.cursor.measure != measure {
        next.cursor.measure = measure;
        next.cursor.index = 0;
    }
    let id=next.score.insert_in_voice(next.cursor.part,next.cursor.staff,measure,next.cursor.voice,next.cursor.index,event).map_err(|_|ScoreError("El pasaje no cabe sin reemplazar notas. Elige un compás vacío o un punto con espacio suficiente"))?;
    next.cursor.index += 1;
    Ok(id)
}

fn remaining_in_bar(score: &Score, tick: u64) -> u64 {
    let (bar, local) = score.measure_at_tick(tick);
    u64::from(score.time_at(bar).capacity() - local)
}
fn rekey(
    pitches: Pitches,
    source: KeySignature,
    destination: KeySignature,
) -> Result<Pitches, ScoreError> {
    Pitches::new(
        &pitches
            .iter()
            .map(|pitch| {
                if pitch.accidental().is_some()
                    || source.accidental(pitch.step) == destination.accidental(pitch.step)
                {
                    pitch
                } else {
                    pitch.with_accidental(Some(
                        source.accidental(pitch.step).unwrap_or(Accidental::Natural),
                    ))
                }
            })
            .collect::<Vec<_>>(),
    )
}
