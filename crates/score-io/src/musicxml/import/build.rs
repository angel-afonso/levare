//! Convert positioned MusicXML streams into the native bounded rhythm model.
use super::*;

struct Lanes {
    passage: bool,
    upper: Vec<String>,
    lower: Vec<String>,
}
impl Lanes {
    fn new(part: &PartData) -> Result<Self, FileError> {
        let mut lanes = Self {
            passage: part.passage,
            upper: Vec::new(),
            lower: Vec::new(),
        };
        for bar in &part.bars {
            for event in &bar.events {
                let list = if event.staff == 1 {
                    &mut lanes.upper
                } else {
                    &mut lanes.lower
                };
                if !list.contains(&event.voice) {
                    list.push(event.voice.clone());
                }
            }
        }
        if lanes.upper.len() > 2 || lanes.lower.len() > 2 || (part.passage && lanes.lower.len() > 1)
        {
            return Err(error(
                "MusicXML: máximo de dos voces por pentagrama y una voz por rama de divisi temporal",
            ));
        }
        Ok(lanes)
    }
    fn lane(&self, staff: u8, voice: Option<&str>) -> Result<u8, FileError> {
        let list = if staff == 1 { &self.upper } else { &self.lower };
        if self.passage && staff == 2 {
            return Ok(1);
        }
        let index = if let Some(voice) = voice {
            if list.is_empty() {
                0
            } else {
                list.iter().position(|v| v == voice).ok_or_else(|| {
                    error("MusicXML: indicación que referencia una voz inexistente")
                })?
            }
        } else {
            0
        };
        Ok((if staff == 2 { 2 } else { 0 }) + index as u8)
    }
}
fn source(lane: u8) -> (StaffId, Voice) {
    (
        if lane >= 2 {
            StaffId::Lower
        } else {
            StaffId::Upper
        },
        if lane % 2 == 1 {
            Voice::Two
        } else {
            Voice::One
        },
    )
}
fn put(
    score: &mut Score,
    part: PartId,
    bar: usize,
    lane: u8,
    mut event: Event,
) -> Result<ElementId, FileError> {
    event.pitches = score
        .part(part)
        .unwrap()
        .transposition()
        .inverse()
        .pitches(event.pitches, score.key_signature_at(bar))
        .map_err(error)?;
    let (staff, voice) = source(lane);
    let index = score
        .part(part)
        .unwrap()
        .staff(staff)
        .ok_or_else(|| error("MusicXML: pentagrama inexistente"))?
        .measures()[bar]
        .voice_len(voice);
    score
        .insert_in_voice(part, staff, bar, voice, index, event)
        .map_err(error)
}
fn append_rest(
    score: &mut Score,
    part: PartId,
    bar: usize,
    lane: u8,
    tick: u16,
    length: u16,
    anchors: &mut BTreeMap<(usize, u8, u16), ElementId>,
) -> Result<(), FileError> {
    let mut at = tick;
    for duration in super::super::durations(length)? {
        let id = put(
            score,
            part,
            bar,
            lane,
            Event {
                pitches: Pitches::default(),
                duration,
                notation: Notation::default(),
            },
        )?;
        anchors.insert((bar, lane, at), id);
        at += duration.ticks();
    }
    Ok(())
}
fn midi(p: Pitch) -> i16 {
    i16::from(p.octave()) * 12
        + [0, 2, 4, 5, 7, 9, 11][p.step.index() as usize]
        + i16::from(p.alteration())
}
fn music(
    raw: &RawEvent,
    key: KeySignature,
    warnings: &mut Warnings,
) -> Result<(Pitches, Notation), FileError> {
    let mut notation = raw.notation;
    let mut tones = raw.pitches.clone();
    let natural_roles = tones.iter().any(|t| {
        t.harmonic.as_ref().is_some_and(|(h, role)| {
            *h == Harmonic::Natural && matches!(role.as_str(), "touch" | "base")
        })
    });
    if natural_roles {
        if !tones.iter().any(|t| {
            t.harmonic
                .as_ref()
                .is_some_and(|(h, role)| *h == Harmonic::Natural && role == "sound")
        }) {
            return Err(error(
                "MusicXML: un armónico natural escrito en posición base o de contacto requiere una altura sonora explícita",
            ));
        }
        tones.retain(|t| {
            t.harmonic
                .as_ref()
                .is_none_or(|(h, role)| *h != Harmonic::Natural || role == "sound")
        });
        warnings.add("Los armónicos naturales con notas auxiliares se muestran a altura sonora");
    }
    if tones.iter().any(|t| {
        t.harmonic
            .as_ref()
            .is_some_and(|(h, role)| *h == Harmonic::Natural && role == "unspecified")
    }) {
        warnings.add("Los armónicos naturales sin tipo de altura explícito se interpretan a la altura escrita");
    }
    let artificial = tones.iter().any(|t| {
        t.harmonic
            .as_ref()
            .is_some_and(|(h, _)| *h == Harmonic::Artificial)
    });
    if artificial || (tones.len() == 2 && tones.iter().any(|t| t.diamond)) {
        if tones.len() == 1
            && tones[0]
                .harmonic
                .as_ref()
                .is_some_and(|(_, r)| r == "sound")
        {
            notation.harmonic = Some(Harmonic::Natural);
            warnings.add("Los armónicos artificiales escritos solo a altura sonora se importan con círculo de armónico");
        } else {
            let base = tones
                .iter()
                .find(|t| !t.diamond && t.harmonic.as_ref().is_none_or(|(_, r)| r != "touch"))
                .cloned()
                .ok_or_else(|| error("MusicXML: armónico artificial sin nota pisada"))?;
            if tones.len() > 2
                || (tones.len() == 2
                    && tones.iter().any(|t| {
                        midi(t.pitch) != midi(base.pitch) && midi(t.pitch) - midi(base.pitch) != 5
                    }))
            {
                return Err(error(
                    "MusicXML: solo se admiten armónicos artificiales de cuarta",
                ));
            }
            tones = vec![base];
            notation.harmonic = Some(Harmonic::Artificial);
        }
    } else if tones.iter().any(|t| t.harmonic.is_some()) {
        if tones.iter().any(|t| t.harmonic.is_none()) {
            return Err(error(
                "MusicXML: un acorde con notas normales y armónicos simultáneos aún no está admitido",
            ));
        }
        notation.harmonic = Some(Harmonic::Natural);
    }
    let pitches = Pitches::new(
        &tones
            .iter()
            .map(|t| {
                let p = t.pitch;
                if p.alteration() == key.accidental(p.step).map_or(0, Accidental::semitones) {
                    p.with_accidental(None)
                } else {
                    p
                }
            })
            .collect::<Vec<_>>(),
    )
    .map_err(error)?;
    if pitches.is_empty() {
        notation.articulation = None;
        notation.harmonic = None;
    }
    Ok((pitches, notation))
}

/// A note's pizzicato attribute is local; words remain persistent. Resolve both
/// before removing redundant marks, so the following arco note is not left plucked.
fn normalize_techniques(
    score: &mut Score,
    part: PartId,
    data: &PartData,
    placed: &HashMap<(usize, usize), (ElementId, ElementId, u8)>,
    lanes: &Lanes,
) -> Result<(), FileError> {
    if data.instrument.family() != InstrumentFamily::Strings {
        return Ok(());
    }
    let divided = score.part(part).unwrap().is_divisi();
    let scope = |lane: u8| if divided || lane < 2 { 0u8 } else { 1 };
    let mut words: [BTreeMap<(usize, u16), PlayingTechnique>; 2] = Default::default();
    for (bar, b) in data.bars.iter().enumerate() {
        for m in &b.directions {
            if let MarkType::Technique(t) = m.kind {
                let lane = lanes.lane(m.staff, m.voice.as_deref())?;
                words[usize::from(scope(lane))].insert((bar, m.tick), t);
            }
        }
    }
    let mut desired = HashMap::new();
    let mut simultaneous = BTreeMap::new();
    for (bar, b) in data.bars.iter().enumerate() {
        for (i, raw) in b.events.iter().enumerate() {
            if raw.pitches.is_empty() {
                continue;
            }
            let &(id, _, lane) = placed.get(&(bar, i)).unwrap();
            let s = scope(lane);
            let wanted = raw.notation.technique.unwrap_or_else(|| {
                words[usize::from(s)]
                    .range(..=(bar, raw.onset))
                    .next_back()
                    .map_or(PlayingTechnique::Arco, |(_, t)| *t)
            });
            if simultaneous
                .insert((bar, raw.onset, s), wanted)
                .is_some_and(|t| t != wanted)
            {
                return Err(error(
                    "MusicXML: técnicas distintas simultáneas dentro del mismo pentagrama no admitidas",
                ));
            }
            desired.insert(
                id,
                (
                    wanted,
                    words[usize::from(s)].contains_key(&(bar, raw.onset)),
                ),
            );
        }
    }
    let p = score.part(part).unwrap();
    let mut events = Vec::new();
    for staff in p.staves() {
        for (bar, b) in staff.measures().iter().enumerate() {
            for (voice, tick, event, n) in b.timed_iter() {
                let lane = if staff.id() == StaffId::Lower { 2 } else { 0 } + voice.number() - 1;
                events.push((
                    bar,
                    tick,
                    lane,
                    ElementId {
                        part,
                        staff: staff.id(),
                        measure: bar,
                        event,
                    },
                    n.notation.technique,
                ));
            }
        }
    }
    events.sort_by_key(|(b, t, l, _, _)| (*b, *t, *l));
    let mut actual = [PlayingTechnique::Arco; 2];
    for (_, _, lane, id, mark) in events {
        let s = usize::from(scope(lane));
        if let Some(&(wanted, explicit)) = desired.get(&id) {
            score
                .set_technique(
                    id,
                    if wanted != actual[s] || explicit {
                        Some(wanted)
                    } else {
                        None
                    },
                )
                .map_err(error)?;
            actual[s] = wanted;
        } else if let Some(t) = mark {
            actual[s] = t;
        }
    }
    Ok(())
}

pub(super) fn score(
    title: String,
    mut parts: Vec<PartData>,
    warnings: &mut Warnings,
) -> Result<Score, FileError> {
    let count = parts.iter().map(|p| p.bars.len()).max().unwrap();
    for part in &mut parts {
        if part.bars.len() != count {
            warnings.add("Las partes de menor longitud se completan con compases vacíos");
            part.bars.resize_with(count, Bar::default);
        }
    }
    let instruments = parts.iter().map(|p| p.instrument).collect::<Vec<_>>();
    let mut score = Score::new(title, &instruments, count).map_err(error)?;
    let mut active_time = vec![TimeSignature::default(); parts.len()];
    let mut active_key = vec![KeySignature::default(); parts.len()];
    for bar in 0..count {
        for (p, data) in parts.iter().enumerate() {
            if let Some(time) = data.bars[bar].time {
                active_time[p] = time;
            }
            if let Some(key) = data.bars[bar].key {
                active_key[p] = key;
            }
        }
        if active_time.iter().any(|t| *t != active_time[0]) {
            return Err(error(
                "MusicXML: métricas distintas simultáneas entre instrumentos no admitidas",
            ));
        }
        let conductor_key = parts
            .iter()
            .enumerate()
            .find(|(_, p)| p.instrument.percussion_key().is_none())
            .map_or(KeySignature::default(), |(i, _)| active_key[i]);
        if parts
            .iter()
            .enumerate()
            .any(|(i, p)| p.instrument.percussion_key().is_none() && active_key[i] != conductor_key)
        {
            warnings.add("Las armaduras distintas entre partes se unifican; se conservan las alturas mediante alteraciones");
        }
        let mut time = active_time[0];
        let used = parts
            .iter()
            .flat_map(|p| p.bars[bar].events.iter())
            .map(|e| e.onset + e.length)
            .max()
            .unwrap_or(0);
        if parts.iter().any(|p| p.bars[bar].implicit) && used > 0 && used < time.capacity() {
            time = [1, 2, 4, 8, 16]
                .into_iter()
                .find_map(|denominator| {
                    let unit = TICKS_PER_WHOLE / u16::from(denominator);
                    if used % unit != 0 {
                        return None;
                    }
                    let numerator = u8::try_from(used / unit).ok()?;
                    let t = TimeSignature {
                        numerator,
                        denominator,
                    };
                    t.is_valid().then_some(t)
                })
                .ok_or_else(|| {
                    error("MusicXML: anacrusa no representable con la métrica admitida")
                })?;
            warnings.add("Las anacrusas se representan con un compás inicial de duración reducida");
        }
        if used > time.capacity() {
            return Err(error(format!(
                "MusicXML: notas fuera de la métrica en el compás {}",
                bar + 1
            )));
        }
        score.set_time_signature_at(bar, time).map_err(error)?;
        score
            .set_key_signature_at(bar, conductor_key)
            .map_err(error)?;
        let marks: Vec<_> = parts
            .iter()
            .filter_map(|p| p.bars[bar].tempo.as_ref())
            .collect();
        if let Some(tempo) = marks.first() {
            if marks.iter().any(|m| {
                u32::from(m.bpm) * u32::from(m.beat.ticks())
                    != u32::from(tempo.bpm) * u32::from(tempo.beat.ticks())
            }) {
                warnings
                    .add("Los tempos simultáneos distintos se unifican con el primero indicado");
            }
            score.set_tempo_at(bar, (*tempo).clone()).map_err(error)?;
        }
    }
    apply_form(&mut score, &parts, warnings);
    apply_rehearsals(&mut score, &parts, warnings)?;
    for (pi, data) in parts.iter().enumerate() {
        let part = score.parts()[pi].id();
        if let Some(interval) = data.transposition {
            score
                .set_part_transposition(part, interval)
                .map_err(error)?;
        }
        let lanes = Lanes::new(data)?;
        let two = data
            .bars
            .iter()
            .any(|b| b.staves == 2 || b.events.iter().any(|e| e.staff == 2));
        if two && !data.instrument.is_grand_staff() && !data.passage {
            score.set_part_divisi(part, true).map_err(error)?;
        }
        let explicit = data.bars.iter().any(|b| {
            b.directions
                .iter()
                .any(|m| matches!(m.kind, MarkType::Division(_)))
        });
        if data.passage {
            for (bar, b) in data.bars.iter().enumerate() {
                for m in &b.directions {
                    let divided = match m.kind {
                        MarkType::Division(d) => Some(d == Division::Divisi),
                        MarkType::Visibility(show) if !explicit => Some(show),
                        _ => None,
                    };
                    if let Some(divided) = divided {
                        if m.tick >= score.time_at(bar).capacity() {
                            continue;
                        }
                        score
                            .set_divisi_span(part, DivisiPoint::new(bar, m.tick), None, divided)
                            .map_err(error)?;
                    }
                }
            }
        }
        let mut anchors = BTreeMap::new();
        let mut placed = HashMap::new();
        let mut links: Vec<Connection> = Vec::new();
        for (bar, b) in data.bars.iter().enumerate() {
            for (&number, &clef) in &b.clefs {
                let staff = if number == 1 {
                    StaffId::Upper
                } else {
                    StaffId::Lower
                };
                let current = score.part(part).unwrap();
                if !current.can_use_clef(clef) {
                    warnings.add("La clave incompatible con el instrumento se omitió");
                } else if number == 2 && !current.instrument().is_grand_staff() {
                    if current.clef_at(StaffId::Upper, bar) != clef {
                        warnings.add("Las ramas de divisi comparten la clave de la primera rama");
                    }
                } else {
                    score.set_clef_at(part, staff, bar, clef).map_err(error)?;
                }
            }
            let mut streams: BTreeMap<u8, Vec<(usize, &RawEvent)>> = BTreeMap::new();
            for (index, e) in b.events.iter().enumerate() {
                streams
                    .entry(lanes.lane(e.staff, Some(&e.voice))?)
                    .or_default()
                    .push((index, e));
            }
            let mut cuts: BTreeMap<u8, BTreeSet<u16>> = BTreeMap::new();
            for m in &b.directions {
                if m.tick >= score.time_at(bar).capacity() {
                    continue;
                }
                if matches!(m.kind, MarkType::Visibility(_) | MarkType::Wedge(..)) {
                    continue;
                }
                let lane = if matches!(m.kind, MarkType::Division(_)) {
                    0
                } else {
                    lanes.lane(m.staff, m.voice.as_deref())?
                };
                cuts.entry(lane).or_default().insert(m.tick);
                streams.entry(lane).or_default();
            }
            for (lane, mut events) in streams {
                events.sort_by_key(|(_, e)| e.onset);
                let mut cursor = 0u16;
                let lane_cuts = cuts.get(&lane).cloned().unwrap_or_default();
                for (raw_index, raw) in events {
                    if raw.onset < cursor {
                        return Err(error(format!(
                            "MusicXML: voces superpuestas no representables en el compás {}",
                            bar + 1
                        )));
                    }
                    let mut gap = cursor;
                    for stop in lane_cuts
                        .range(cursor..raw.onset)
                        .copied()
                        .chain(std::iter::once(raw.onset))
                    {
                        if stop > gap {
                            append_rest(
                                &mut score,
                                part,
                                bar,
                                lane,
                                gap,
                                stop - gap,
                                &mut anchors,
                            )?;
                        }
                        gap = stop;
                    }
                    let (pitches, notation) = music(raw, score.key_signature_at(bar), warnings)?;
                    let end = raw.onset + raw.length;
                    let mut boundaries = vec![raw.onset];
                    boundaries.extend(
                        lane_cuts
                            .range((
                                std::ops::Bound::Excluded(raw.onset),
                                std::ops::Bound::Excluded(end),
                            ))
                            .copied(),
                    );
                    boundaries.push(end);
                    let mut ids = Vec::new();
                    for pair in boundaries.windows(2) {
                        let durations = if boundaries.len() == 2
                            && let Some(written) = raw.written
                        {
                            vec![written]
                        } else {
                            super::super::durations(pair[1] - pair[0])?
                        };
                        let mut at = pair[0];
                        for duration in durations {
                            let mut n = notation;
                            if !ids.is_empty() {
                                n.articulation = None;
                                n.dynamic = None;
                                n.technique = None;
                                n.division = None;
                            }
                            if at + duration.ticks() != end {
                                n.fermata = false;
                            }
                            // Shared rolls are resolved after both hands exist.
                            n.arpeggio = None;
                            let id = put(
                                &mut score,
                                part,
                                bar,
                                lane,
                                Event {
                                    pitches,
                                    duration,
                                    notation: n,
                                },
                            )?;
                            if let Some(&previous) = ids.last()
                                && !pitches.is_empty()
                            {
                                links.push(Connection {
                                    start: previous,
                                    end: id,
                                    kind: ConnectionKind::Tie,
                                });
                            }
                            anchors.insert((bar, lane, at), id);
                            ids.push(id);
                            at += duration.ticks();
                        }
                    }
                    placed.insert((bar, raw_index), (ids[0], *ids.last().unwrap(), lane));
                    cursor = end;
                }
                // A direction may fall into trailing silence; create an anchor without shifting it.
                for tick in lane_cuts.range(cursor..).copied() {
                    if tick > cursor {
                        append_rest(
                            &mut score,
                            part,
                            bar,
                            lane,
                            cursor,
                            tick - cursor,
                            &mut anchors,
                        )?;
                        cursor = tick;
                    }
                    if !anchors.contains_key(&(bar, lane, tick)) {
                        let next = lane_cuts
                            .range((std::ops::Bound::Excluded(tick), std::ops::Bound::Unbounded))
                            .next()
                            .copied()
                            .unwrap_or(score.time_at(bar).capacity());
                        if next > tick {
                            append_rest(
                                &mut score,
                                part,
                                bar,
                                lane,
                                tick,
                                next - tick,
                                &mut anchors,
                            )?;
                            cursor = next;
                        }
                    }
                }
            }
        }
        // Explicit per-event ties and slurs are paired in their independent native lanes.
        let mut starts: HashMap<(u8, ConnectionKind, String), ElementId> = HashMap::new();
        let mut rolls: BTreeMap<(usize, u16, String), Vec<(ElementId, Arpeggio)>> = BTreeMap::new();
        for (bar, b) in data.bars.iter().enumerate() {
            for (raw_index, raw) in b.events.iter().enumerate() {
                let &(first, last, lane) = placed.get(&(bar, raw_index)).unwrap();
                let mut endpoints = raw.endpoints.clone();
                for stop in [false, true] {
                    let tones = raw.pitches.iter().filter(|t| {
                        !t.diamond && t.harmonic.as_ref().is_none_or(|(_, role)| role != "touch")
                    });
                    if endpoints
                        .iter()
                        .any(|(k, s, _)| *k == ConnectionKind::Tie && *s == stop)
                        && !tones.clone().all(|t| t.ties[usize::from(stop)])
                    {
                        endpoints.retain(|(k, s, _)| !(*k == ConnectionKind::Tie && *s == stop));
                        warnings.add("Las ligaduras de unión que afectan solo a algunas notas de un acorde no se importan");
                    }
                }
                endpoints.sort_by_key(|(k, stop, num)| {
                    (std::cmp::Reverse(*stop), format!("{k:?}"), num.clone())
                });
                endpoints.dedup();
                for (kind, stop, number) in endpoints {
                    let key = (lane, kind, number);
                    if stop {
                        if let Some(start) = starts.remove(&key) {
                            links.push(Connection {
                                start,
                                end: first,
                                kind,
                            });
                        } else {
                            warnings
                                .add("Algunas ligaduras tienen extremos ausentes y no se importan");
                        }
                    } else {
                        starts.insert(
                            key,
                            if kind == ConnectionKind::Tie {
                                last
                            } else {
                                first
                            },
                        );
                    }
                }
                if let Some((number, kind)) = &raw.arpeggio {
                    rolls
                        .entry((bar, raw.onset, number.clone()))
                        .or_default()
                        .push((first, *kind));
                }
            }
        }
        if !starts.is_empty() {
            warnings.add("Algunas ligaduras tienen extremos ausentes y no se importan");
        }
        for (_, events) in rolls {
            let shared = data.instrument.is_grand_staff()
                && events.len() == 2
                && events[0].0.staff != events[1].0.staff
                && events[0].1 == events[1].1
                && score.piano_partner(events[0].0) == Some(events[1].0);
            if shared {
                let (id, kind) = events[0];
                // The public API permits a single tone in each hand for a shared roll.
                if score.set_arpeggio_both_hands(id, true).is_ok() {
                    score.set_arpeggio(id, Some(kind)).map_err(error)?;
                } else {
                    warnings.add("Un arpegio entre pentagramas no pudo enlazarse");
                }
            } else {
                for (id, kind) in events {
                    if score.event(id).unwrap().pitches.len() >= 2 {
                        score.set_arpeggio(id, Some(kind)).map_err(error)?;
                    } else {
                        warnings.add("Un arpegio sobre una sola nota no pudo importarse");
                    }
                }
            }
        }
        for (bar, b) in data.bars.iter().enumerate() {
            for m in &b.directions {
                if matches!(
                    m.kind,
                    MarkType::Division(_) | MarkType::Visibility(_) | MarkType::Wedge(..)
                ) {
                    continue;
                }
                if m.tick >= score.time_at(bar).capacity() {
                    warnings.add("Una indicación fuera del compás no pudo importarse");
                    continue;
                }
                let lane = lanes.lane(m.staff, m.voice.as_deref())?;
                if let Some(&id) = anchors.get(&(bar, lane, m.tick)) {
                    match m.kind {
                        MarkType::Dynamic(d) => score.set_dynamic(id, Some(d)).map_err(error)?,
                        MarkType::Technique(t) => {
                            if data.instrument.family() == InstrumentFamily::Strings {
                                score.set_technique(id, Some(t)).map_err(error)?;
                            } else {
                                warnings.add(
                                    "Una técnica de cuerda en otro instrumento no pudo importarse",
                                );
                            }
                        }
                        _ => {}
                    }
                } else {
                    warnings
                        .add("Una indicación sin nota o silencio de anclaje no pudo importarse");
                }
            }
        }
        normalize_techniques(&mut score, part, data, &placed, &lanes)?;
        let mut wedges: HashMap<(u8, String), (ElementId, ConnectionKind)> = HashMap::new();
        for (bar, b) in data.bars.iter().enumerate() {
            for m in &b.directions {
                if let MarkType::Wedge(stop, kind, number) = &m.kind {
                    let lane = lanes.lane(m.staff, m.voice.as_deref())?;
                    let exact = anchors.get(&(bar, lane, m.tick)).copied();
                    let id = exact.or_else(|| {
                        anchors
                            .range(..=(bar, lane, m.tick))
                            .rev()
                            .find(|((_, l, _), _)| *l == lane)
                            .map(|(_, id)| *id)
                    });
                    if let Some(id) = id {
                        if exact.is_none() {
                            warnings.add(
                                "Algunos extremos de regulador se ajustaron a la nota anterior",
                            );
                        }
                        let key = (lane, number.clone());
                        if *stop {
                            if let Some((start, kind)) = wedges.remove(&key) {
                                links.push(Connection {
                                    start,
                                    end: id,
                                    kind,
                                });
                            } else {
                                warnings.add("Un regulador sin inicio no pudo importarse");
                            }
                        } else {
                            wedges.insert(key, (id, *kind));
                        }
                    } else {
                        warnings.add("Un regulador sin nota de anclaje no pudo importarse");
                    }
                }
            }
        }
        if !wedges.is_empty() {
            warnings.add("Un regulador sin final no pudo importarse");
        }
        for link in links {
            if score.add_connection(link).is_err() {
                warnings.add("Algunas ligaduras o reguladores incompatibles con el modelo no pudieron importarse");
            }
        }
    }
    score.validate().map_err(error)?;
    Ok(score)
}

fn apply_form(score: &mut Score, parts: &[PartData], warnings: &mut Warnings) {
    for bar in 0..score.measures().len() {
        let sources = || parts.iter().map(|part| &part.bars[bar]);
        let mut form = BarForm {
            repeat_start: sources().any(|data| data.repeat_start),
            repeat_end: sources().find_map(|data| data.repeat_end),
            barline: sources().find_map(|data| data.barline).unwrap_or_default(),
            segno: sources().any(|data| data.segno),
            coda: sources().any(|data| data.coda),
            to_coda: sources().any(|data| data.to_coda),
            fine: sources().any(|data| data.fine),
            jump: None,
        };
        if let Some(source) = sources()
            .find(|data| data.jump.is_some() && !data.jump_from_sound)
            .or_else(|| sources().find(|data| data.jump.is_some()))
        {
            form.jump = source.jump.map(|jump| {
                if source.jump_from_sound && form.fine {
                    if jump.from_segno() {
                        Jump::DalSegnoAlFine
                    } else {
                        Jump::DaCapoAlFine
                    }
                } else if source.jump_from_sound && form.to_coda {
                    if jump.from_segno() {
                        Jump::DalSegnoAlCoda
                    } else {
                        Jump::DaCapoAlCoda
                    }
                } else {
                    jump
                }
            });
        }
        if score.set_bar_form(bar, form).is_err() {
            warnings.add("Se omitió una repetición o casilla no válida");
        }
    }
    if let Some(part) = parts
        .iter()
        .find(|part| part.bars.iter().any(|bar| bar.ending_start.is_some()))
    {
        let mut pending = None;
        for (bar, data) in part.bars.iter().enumerate() {
            if let Some(numbers) = &data.ending_start {
                if let Some((start, numbers)) = pending.take() {
                    set_ending(score, start, start, numbers, warnings);
                }
                pending = Some((bar, numbers.clone()));
            }
            if data.ending_stop
                && let Some((start, numbers)) = pending.take()
            {
                set_ending(score, start, bar, numbers, warnings);
            }
        }
        if let Some((start, numbers)) = pending {
            set_ending(score, start, start, numbers, warnings);
        }
    }
}

fn set_ending(
    score: &mut Score,
    start: usize,
    end: usize,
    numbers: Vec<u8>,
    warnings: &mut Warnings,
) {
    if score
        .set_ending(Ending {
            start,
            end,
            numbers,
        })
        .is_err()
    {
        warnings.add("Se omitió una repetición o casilla no válida");
    }
}

/// Infer the best automatic sequence, retaining exceptions as custom labels.
fn apply_rehearsals(
    score: &mut Score,
    parts: &[PartData],
    warnings: &mut Warnings,
) -> Result<(), FileError> {
    let Some(part) = parts
        .iter()
        .find(|part| part.bars.iter().any(|bar| bar.rehearsal.is_some()))
    else {
        return Ok(());
    };
    let mut marks = Vec::new();
    for (bar, data) in part.bars.iter().enumerate() {
        if let Some(text) = &data.rehearsal {
            let clean: String = text.chars().filter(|c| !c.is_control()).take(16).collect();
            if &clean != text {
                warnings.add("Algunas marcas de ensayo se recortan a 16 caracteres sin controles");
            }
            if clean.trim().is_empty() {
                continue;
            }
            score.toggle_rehearsal_mark(bar).map_err(error)?;
            marks.push((bar, clean));
        }
    }
    let mut best = RehearsalStyle::Letters;
    let mut matches = 0;
    for style in RehearsalStyle::ALL {
        score.set_rehearsal_style(style);
        let count = marks
            .iter()
            .filter(|(bar, text)| score.rehearsal_label(*bar).as_ref() == Some(text))
            .count();
        if count > matches {
            best = style;
            matches = count;
        }
    }
    score.set_rehearsal_style(best);
    for (bar, text) in marks {
        if score.rehearsal_label(bar).as_ref() != Some(&text) {
            score.set_rehearsal_custom(bar, Some(text)).map_err(error)?;
        }
    }
    Ok(())
}
