//! MusicXML 4.0 partwise interchange, independent of screen and PDF layout.
//! Pitches retain the application's concert-pitch convention. A temporary
//! divisi reserves a second staff, hiding it in bars outside the passage.
use crate::{FileError, error};
use score_core::*;
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    fmt::Write as _,
    io::Write as _,
    path::Path,
};
mod import;
pub use import::{ImportResult, decode, load};

/// Resolve persistent techniques and connection endpoints once per part, rather
/// than searching the preceding score separately for every exported chord tone.
#[derive(Default)]
struct PartIndex {
    endpoints: HashMap<ElementId, Vec<(ConnectionKind, bool)>>,
    pizzicato: HashSet<ElementId>,
    shared_arpeggios: HashMap<ElementId, (Arpeggio, u8)>,
}
impl PartIndex {
    fn new(score: &Score, part: &Part) -> Self {
        let mut index = Self::default();
        for c in score
            .connections()
            .iter()
            .filter(|c| c.start.part == part.id())
        {
            index
                .endpoints
                .entry(c.start)
                .or_default()
                .push((c.kind, false));
            index
                .endpoints
                .entry(c.end)
                .or_default()
                .push((c.kind, true));
        }
        for mark in score
            .piano_arpeggios()
            .iter()
            .filter(|m| m.upper.part == part.id())
        {
            let number = score.voice_of(mark.upper).unwrap().number();
            for id in [mark.upper, mark.lower] {
                index.shared_arpeggios.insert(id, (mark.kind, number));
            }
        }
        for source in part.staves() {
            let mut techniques = BTreeMap::new();
            for staff in part.expression_staves(source.id()) {
                for (bar, measure) in staff.measures().iter().enumerate() {
                    for (_, tick, _, event) in measure.timed_iter() {
                        if let Some(mark) = event.notation.technique {
                            techniques.insert((bar, tick), mark);
                        }
                    }
                }
            }
            for (bar, measure) in source.measures().iter().enumerate() {
                for (_, tick, event, _) in measure.timed_iter() {
                    if techniques
                        .range(..=(bar, tick))
                        .next_back()
                        .is_some_and(|(_, t)| *t == PlayingTechnique::Pizzicato)
                    {
                        index.pizzicato.insert(ElementId {
                            part: part.id(),
                            staff: source.id(),
                            measure: bar,
                            event,
                        });
                    }
                }
            }
        }
        index
    }
    fn endpoints(&self, id: ElementId) -> &[(ConnectionKind, bool)] {
        self.endpoints.get(&id).map_or(&[], Vec::as_slice)
    }
}

fn escape(text: &str) -> String {
    let mut out = String::new();
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            '\t' | '\n' | '\r' => out.push(c),
            c if c >= ' ' && c != '\u{fffe}' && c != '\u{ffff}' => out.push(c),
            _ => out.push('\u{fffd}'),
        }
    }
    out
}
fn value(d: Duration) -> &'static str {
    match d.value() {
        NoteValue::Whole => "whole",
        NoteValue::Half => "half",
        NoteValue::Quarter => "quarter",
        NoteValue::Eighth => "eighth",
        NoteValue::Sixteenth => "16th",
    }
}
fn pitch(xml: &mut String, p: Pitch, alteration: i8) {
    let _ = write!(
        xml,
        "<pitch><step>{:?}</step><alter>{alteration}</alter><octave>{}</octave></pitch>",
        p.step,
        p.octave()
    );
}
fn direction(xml: &mut String, body: &str, tick: u16, voice: u8, staff: u8, sound: &str) {
    let _ = writeln!(
        xml,
        "<direction><direction-type>{body}</direction-type><offset sound=\"yes\">{tick}</offset><voice>{voice}</voice><staff>{staff}</staff>{sound}</direction>"
    );
}
fn words(xml: &mut String, text: &str, tick: u16, voice: u8, staff: u8) {
    direction(
        xml,
        &format!("<words font-style=\"italic\">{}</words>", escape(text)),
        tick,
        voice,
        staff,
        "",
    );
}
fn staff_number(part: &Part, staff: StaffId, voice: Voice, bar: usize, tick: u16) -> u8 {
    if staff == StaffId::Lower
        || (part.has_passage_divisi() && voice == Voice::Two && part.divisi_at_tick(bar, tick))
    {
        2
    } else {
        1
    }
}
fn voice_number(staff: StaffId, voice: Voice) -> u8 {
    if staff == StaffId::Lower {
        voice.number() + 2
    } else {
        voice.number()
    }
}

/// Encode a validated score as UTF-8, uncompressed `.musicxml`.
pub fn encode(score: &Score) -> Result<Vec<u8>, FileError> {
    score.validate().map_err(error)?;
    let mut xml = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<score-partwise version=\"4.0\">\n",
    );
    let _ = writeln!(
        xml,
        "<work><work-title>{}</work-title></work><identification><encoding><software>Partitura</software></encoding></identification><part-list>",
        escape(score.title())
    );
    for (i, part) in score.parts().iter().enumerate() {
        let name = escape(&score.part_name(part.id()));
        let slot = i % 14;
        let channel = if part.instrument().percussion_key().is_some() {
            10
        } else {
            slot + 1 + usize::from(slot >= 9)
        };
        let instrument_name = escape(part.instrument().name());
        let percussion = part
            .instrument()
            .percussion_key()
            .map_or_else(String::new, |key| {
                format!("<midi-unpitched>{}</midi-unpitched>", key + 1)
            });
        let _ = writeln!(
            xml,
            "<score-part id=\"P{}\"><part-name>{name}</part-name><score-instrument id=\"I{}\"><instrument-name>{instrument_name}</instrument-name></score-instrument><midi-device port=\"{}\"/><midi-instrument id=\"I{}\"><midi-channel>{channel}</midi-channel><midi-program>{}</midi-program>{percussion}</midi-instrument></score-part>",
            i + 1,
            i + 1,
            i / 14 + 1,
            i + 1,
            part.instrument().program() + 1
        );
    }
    xml.push_str("</part-list>\n");
    for (i, part) in score.parts().iter().enumerate() {
        let index = PartIndex::new(score, part);
        let _ = writeln!(xml, "<part id=\"P{}\">", i + 1);
        for bar in 0..score.measures().len() {
            let _ = writeln!(xml, "<measure number=\"{}\">", bar + 1);
            attributes(&mut xml, score, part, bar);
            directions(&mut xml, score, part, &index, bar, i == 0);
            let capacity = score.time_at(bar).capacity();
            let mut first = true;
            for source in part.staves() {
                let measure = &source.measures()[bar];
                for voice in Voice::ALL {
                    if voice == Voice::Two && measure.voice_len(voice) == 0 {
                        continue;
                    }
                    if !first {
                        let _ = writeln!(xml, "<backup><duration>{capacity}</duration></backup>");
                    }
                    first = false;
                    let mut onset = 0;
                    for (event_id, event) in measure.voice_iter(voice) {
                        let id = ElementId {
                            part: part.id(),
                            staff: source.id(),
                            measure: bar,
                            event: event_id,
                        };
                        let mut cuts = vec![onset];
                        let end = onset + event.duration.ticks();
                        if part.has_passage_divisi() && voice == Voice::Two {
                            cuts.extend(
                                part.divisi_changes_in(bar)
                                    .map(|(tick, _)| tick)
                                    .filter(|tick| *tick > onset && *tick < end),
                            );
                        }
                        cuts.push(end);
                        let mut pieces = Vec::new();
                        if cuts.len() == 2 {
                            pieces.push((onset, event.duration));
                        } else {
                            for range in cuts.windows(2) {
                                let mut at = range[0];
                                for d in durations(range[1] - range[0])? {
                                    pieces.push((at, d));
                                    at += d.ticks();
                                }
                            }
                        }
                        for (piece, &(tick, duration)) in pieces.iter().enumerate() {
                            let fragment = Fragment {
                                id,
                                event: *event,
                                duration,
                                voice,
                                staff: staff_number(part, source.id(), voice, bar, tick),
                                first: piece == 0,
                                last: piece + 1 == pieces.len(),
                                split: pieces.len() > 1,
                                two_voices: measure.has_two_voices()
                                    && !(part.has_passage_divisi() && part.bar_fully_divisi(bar)),
                            };
                            notes(&mut xml, score, &index, fragment);
                        }
                        onset = end;
                    }
                    if onset < capacity {
                        let staff = staff_number(part, source.id(), voice, bar, onset);
                        let number = voice_number(source.id(), voice);
                        let full = if onset == 0 { " measure=\"yes\"" } else { "" };
                        let _ = writeln!(
                            xml,
                            "<note><rest{full}/><duration>{}</duration><voice>{number}</voice><staff>{staff}</staff></note>",
                            capacity - onset
                        );
                    }
                }
            }
            if bar + 1 == score.measures().len() {
                xml.push_str(
                    "<barline location=\"right\"><bar-style>light-heavy</bar-style></barline>\n",
                );
            }
            xml.push_str("</measure>\n");
        }
        xml.push_str("</part>\n");
    }
    xml.push_str("</score-partwise>\n");
    Ok(xml.into_bytes())
}

/// Flush and replace atomically, leaving an existing destination intact on error.
pub fn export(path: &Path, score: &Score) -> Result<(), FileError> {
    let bytes = encode(score)?;
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let mut file = tempfile::NamedTempFile::new_in(parent).map_err(error)?;
    file.write_all(&bytes).map_err(error)?;
    file.as_file().sync_all().map_err(error)?;
    file.persist(path).map_err(error)?;
    Ok(())
}

fn attributes(xml: &mut String, score: &Score, part: &Part, bar: usize) {
    xml.push_str("<attributes>");
    if bar == 0 {
        let _ = write!(xml, "<divisions>{TICKS_PER_QUARTER}</divisions>");
    }
    if score.key_change_at(bar).is_some() {
        let key = score.written_key_at(part.id(), bar);
        let mode = if key.mode() == KeyMode::Minor {
            "minor"
        } else {
            "major"
        };
        let _ = write!(
            xml,
            "<key><fifths>{}</fifths><mode>{mode}</mode></key>",
            key.fifths()
        );
    }
    if let Some(time) = score.time_change_at(bar) {
        let _ = write!(
            xml,
            "<time><beats>{}</beats><beat-type>{}</beat-type></time>",
            time.numerator, time.denominator
        );
    }
    let two = part.staves().len() == 2 || part.has_passage_divisi();
    if bar == 0 {
        if two {
            xml.push_str("<staves>2</staves>");
        }
        for staff in 1..=if two { 2 } else { 1 } {
            let clef = part.clef(if staff == 1 {
                StaffId::Upper
            } else {
                StaffId::Lower
            });
            let (sign, line) = match clef {
                Clef::Treble => ("G", 2),
                Clef::Alto => ("C", 3),
                Clef::Bass => ("F", 4),
                Clef::Percussion => ("percussion", 2),
            };
            let _ = write!(
                xml,
                "<clef number=\"{staff}\"><sign>{sign}</sign><line>{line}</line></clef>"
            );
        }
    }
    if part.has_passage_divisi() {
        let show = if part.bar_has_divisi(bar) {
            "yes"
        } else {
            "no"
        };
        let _ = write!(
            xml,
            "<staff-details number=\"2\" print-object=\"{show}\" print-spacing=\"{show}\"/>"
        );
    }
    if bar == 0 {
        let interval = part.transposition();
        let octaves = interval.chromatic / 12;
        let _ = write!(
            xml,
            "<transpose><diatonic>{}</diatonic><chromatic>{}</chromatic><octave-change>{octaves}</octave-change></transpose>",
            interval.diatonic - octaves * 7,
            interval.chromatic - octaves * 12
        );
    }
    xml.push_str("</attributes>\n");
}

fn directions(
    xml: &mut String,
    score: &Score,
    part: &Part,
    index: &PartIndex,
    bar: usize,
    conductor: bool,
) {
    if conductor && let Some(mark) = score.tempo_change_at(bar) {
        if !mark.text.is_empty() {
            words(xml, &mark.text, 0, 1, 1);
        }
        let (unit, dot) = match mark.beat {
            TempoBeat::Half => ("half", ""),
            TempoBeat::Quarter => ("quarter", ""),
            TempoBeat::Eighth => ("eighth", ""),
            TempoBeat::DottedHalf => ("half", "<beat-unit-dot/>"),
            TempoBeat::DottedQuarter => ("quarter", "<beat-unit-dot/>"),
            TempoBeat::DottedEighth => ("eighth", "<beat-unit-dot/>"),
        };
        let bpm = f64::from(mark.bpm) * f64::from(mark.beat.ticks()) / f64::from(TICKS_PER_QUARTER);
        direction(
            xml,
            &format!(
                "<metronome><beat-unit>{unit}</beat-unit>{dot}<per-minute>{}</per-minute></metronome>",
                mark.bpm
            ),
            0,
            1,
            1,
            &format!("<sound tempo=\"{bpm}\"/>"),
        );
    }
    if part.is_divisi() && bar == 0 {
        words(xml, "div.", 0, 1, 1);
    }
    for (tick, mark) in part.divisi_changes_in(bar) {
        words(xml, mark.label(), tick, 1, 1);
    }
    for source in part.staves() {
        for (voice, tick, event_id, event) in source.measures()[bar].timed_iter() {
            let id = ElementId {
                part: part.id(),
                staff: source.id(),
                measure: bar,
                event: event_id,
            };
            let staff = staff_number(part, source.id(), voice, bar, tick);
            let number = voice_number(source.id(), voice);
            if let Some(dynamic) = event.notation.dynamic {
                direction(
                    xml,
                    &format!("<dynamics><{}/></dynamics>", dynamic.label()),
                    tick,
                    number,
                    staff,
                    &format!(
                        "<sound dynamics=\"{}\"/>",
                        f64::from(dynamic.velocity()) * 100.0 / 90.0
                    ),
                );
            }
            if let Some(technique) = event.notation.technique {
                words(xml, technique.label(), tick, number, staff);
            }
            if let Some(mark) = event.notation.division
                && !part.has_passage_divisi()
            {
                words(xml, mark.label(), tick, number, staff);
            }
            // Stop first so consecutive hairpins can reuse their number.
            for stop in [true, false] {
                for &(connection, _) in index
                    .endpoints(id)
                    .iter()
                    .filter(|(kind, end)| kind.is_hairpin() && *end == stop)
                {
                    let kind = if stop {
                        "stop"
                    } else if connection == ConnectionKind::Crescendo {
                        "crescendo"
                    } else {
                        "diminuendo"
                    };
                    direction(
                        xml,
                        &format!("<wedge type=\"{kind}\" number=\"{number}\"/>"),
                        tick,
                        number,
                        staff,
                        "",
                    );
                }
            }
        }
    }
}

struct Fragment {
    id: ElementId,
    event: Event,
    duration: Duration,
    voice: Voice,
    staff: u8,
    first: bool,
    last: bool,
    split: bool,
    two_voices: bool,
}
fn notes(xml: &mut String, score: &Score, index: &PartIndex, f: Fragment) {
    let key = score.written_key_at(f.id.part, f.id.measure);
    let mut pitches: Vec<_> = f
        .event
        .pitches
        .iter()
        .map(|p| (p, p.in_key(key).alteration(), false))
        .collect();
    if let Some((touch, alteration)) = f.event.harmonic_touch(key) {
        pitches.push((touch, alteration, true));
    }
    let count = pitches.len().max(1);
    let tie_stop = (!f.first && f.split)
        || (f.first && index.endpoints(f.id).contains(&(ConnectionKind::Tie, true)));
    let tie_start = (!f.last && f.split)
        || (f.last
            && index
                .endpoints(f.id)
                .contains(&(ConnectionKind::Tie, false)));
    let number = voice_number(f.id.staff, f.voice);
    let pizz = index.pizzicato.contains(&f.id);
    for tone in 0..count {
        let _ = write!(
            xml,
            "<note{}>",
            if pizz { " pizzicato=\"yes\"" } else { "" }
        );
        if tone > 0 {
            xml.push_str("<chord/>");
        }
        let touch = if let Some(&(p, alteration, touch)) = pitches.get(tone) {
            if score
                .part(f.id.part)
                .unwrap()
                .instrument()
                .percussion_key()
                .is_some()
            {
                xml.push_str("<unpitched><display-step>B</display-step><display-octave>4</display-octave></unpitched>");
            } else {
                pitch(xml, p, alteration);
            }
            touch
        } else {
            xml.push_str("<rest/>");
            false
        };
        let _ = write!(xml, "<duration>{}</duration>", f.duration.ticks());
        if !pitches.is_empty() {
            if tie_stop {
                xml.push_str("<tie type=\"stop\"/>");
            }
            if tie_start {
                xml.push_str("<tie type=\"start\"/>");
            }
        }
        let _ = write!(
            xml,
            "<voice>{number}</voice><type>{}</type>",
            value(f.duration)
        );
        if f.duration.is_dotted() {
            xml.push_str("<dot/>");
        }
        if let Some(&(p, _, _)) = pitches.get(tone)
            && let Some(sign) = p.accidental()
        {
            let sign = match sign {
                Accidental::Flat => "flat",
                Accidental::Natural => "natural",
                Accidental::Sharp => "sharp",
            };
            let _ = write!(xml, "<accidental>{sign}</accidental>");
        }
        if f.duration.is_triplet() {
            xml.push_str("<time-modification><actual-notes>3</actual-notes><normal-notes>2</normal-notes></time-modification>");
        }
        if f.two_voices && !pitches.is_empty() && f.duration.value() != NoteValue::Whole {
            xml.push_str(if f.voice == Voice::One {
                "<stem>up</stem>"
            } else {
                "<stem>down</stem>"
            });
        }
        if touch {
            xml.push_str("<notehead>diamond</notehead>");
        }
        let _ = write!(xml, "<staff>{}</staff><notations>", f.staff);
        if !pitches.is_empty() {
            if tie_stop {
                xml.push_str("<tied type=\"stop\"/>");
            }
            if tie_start {
                xml.push_str("<tied type=\"start\"/>");
            }
        }
        if tone == 0 {
            for stop in [true, false] {
                if ((stop && f.last) || (!stop && f.first))
                    && index
                        .endpoints(f.id)
                        .contains(&(ConnectionKind::Slur, stop))
                {
                    let kind = if stop { "stop" } else { "start" };
                    let _ = write!(xml, "<slur type=\"{kind}\" number=\"{number}\"/>");
                }
            }
            if f.first
                && let Some(art) = f.event.notation.articulation
            {
                let tag = match art {
                    Articulation::Staccato => "staccato",
                    Articulation::Tenuto => "tenuto",
                    Articulation::Accent => "accent",
                    Articulation::Marcato => "strong-accent",
                };
                let _ = write!(xml, "<articulations><{tag}/></articulations>");
            }
            if f.last && f.event.notation.fermata {
                xml.push_str("<fermata/>");
            }
        }
        if let Some(harmonic) = f.event.notation.harmonic {
            let kind = if harmonic == Harmonic::Natural {
                "natural"
            } else {
                "artificial"
            };
            let role = if harmonic == Harmonic::Natural {
                "sounding-pitch"
            } else if touch {
                "touching-pitch"
            } else {
                "base-pitch"
            };
            let _ = write!(
                xml,
                "<technical><harmonic><{kind}/><{role}/></harmonic></technical>"
            );
        }
        if f.first
            && let Some((kind, arpeggio_number)) = index
                .shared_arpeggios
                .get(&f.id)
                .copied()
                .or_else(|| f.event.notation.arpeggio.map(|kind| (kind, number + 4)))
        {
            let arrow = match kind {
                Arpeggio::Normal => "",
                Arpeggio::Up => " direction=\"up\"",
                Arpeggio::Down => " direction=\"down\"",
            };
            let _ = write!(xml, "<arpeggiate number=\"{arpeggio_number}\"{arrow}/>");
        }
        xml.push_str("</notations></note>\n");
    }
}

/// Exact spelling for a note cut by a mid-bar staff change; ties join the pieces.
fn durations(ticks: u16) -> Result<Vec<Duration>, FileError> {
    let mut values = Duration::all_modified();
    values.sort_by_key(|d| (std::cmp::Reverse(d.ticks()), d.is_triplet(), d.is_dotted()));
    let mut best = vec![None; usize::from(ticks) + 1];
    best[0] = Some(Vec::new());
    for tick in 1..best.len() {
        for &d in &values {
            let len = usize::from(d.ticks());
            if len <= tick
                && let Some(rest) = &best[tick - len]
                && best[tick].as_ref().is_none_or(|b| b.len() > rest.len() + 1)
            {
                let mut next = vec![d];
                next.extend(rest);
                best[tick] = Some(next);
            }
        }
    }
    best.pop()
        .flatten()
        .ok_or_else(|| error("No se puede representar el corte de divisi en MusicXML"))
}

#[cfg(test)]
mod tests;
