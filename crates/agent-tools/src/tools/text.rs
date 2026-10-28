//! How the score reads in replies: signatures, beats and bars of one part in
//! the compact notation, with `»` before the selected events.
use super::part_number;
use score_core::{
    Barline, ElementId, KeySignature, PartId, Score, StaffId, TICKS_PER_WHOLE, TempoMark,
    TimeSignature, Voice,
};

/// Ticks in one beat: a unit of the time signature's denominator.
pub(super) fn beat_unit(score: &Score, measure: usize) -> u16 {
    TICKS_PER_WHOLE / u16::from(score.time_at(measure).denominator.max(1))
}

/// A beat number as the agent writes it back: `2`, `2.5`.
pub(super) fn beat_label(tick: u16, unit: u16) -> String {
    let beat = format!("{:.3}", f32::from(tick) / f32::from(unit) + 1.);
    beat.trim_end_matches('0').trim_end_matches('.').to_owned()
}

/// A length in beats with a fraction, as a musician counts it: `4½`, `2¾`.
pub(super) fn beats(ticks: u16, unit: u16) -> String {
    let (whole, rest) = (ticks / unit, ticks % unit);
    if rest == 0 {
        return whole.to_string();
    }
    let common = gcd(rest, unit);
    let (numerator, denominator) = (rest / common, unit / common);
    let glyph = match (numerator, denominator) {
        (1, 2) => "½",
        (1, 3) => "⅓",
        (2, 3) => "⅔",
        (1, 4) => "¼",
        (3, 4) => "¾",
        (1, 6) => "⅙",
        (5, 6) => "⅚",
        (1, 8) => "⅛",
        (3, 8) => "⅜",
        (5, 8) => "⅝",
        (7, 8) => "⅞",
        _ => "",
    };
    match (whole, glyph) {
        (0, "") => format!("{numerator}/{denominator}"),
        (_, "") => format!("{whole} {numerator}/{denominator}"),
        (0, glyph) => glyph.to_owned(),
        (_, glyph) => format!("{whole}{glyph}"),
    }
}

fn gcd(a: u16, b: u16) -> u16 {
    if b == 0 { a } else { gcd(b, a % b) }
}

pub(super) fn time_text(time: TimeSignature) -> String {
    format!("{}/{}", time.numerator, time.denominator)
}

/// «Sol mayor (armadura: F#)», with the letters the notation uses.
pub(super) fn key_text(key: KeySignature) -> String {
    let sign = if key.fifths() < 0 { "b" } else { "#" };
    let steps: Vec<_> = key
        .steps()
        .iter()
        .map(|step| format!("{step:?}{sign}"))
        .collect();
    if steps.is_empty() {
        format!("{} (sin alteraciones en la armadura)", key.label())
    } else {
        format!("{} (armadura: {})", key.label(), steps.join(" "))
    }
}

pub(super) use score_text::{part_label, quoted};

pub(super) fn tempo_text(mark: &TempoMark) -> String {
    let mut text = format!("{} = {}", mark.beat.label().to_lowercase(), mark.bpm);
    if !mark.text.is_empty() {
        text.push(' ');
        text.push_str(&quoted(&mark.text));
    }
    text
}

/// A bar's rehearsal mark; a label the musician typed is quoted, one Levare
/// counts (A, 2, 17) is not.
pub(super) fn rehearsal_text(score: &Score, measure: usize) -> Option<String> {
    let label = score.rehearsal_label(measure)?;
    let custom = score
        .rehearsal_marks()
        .any(|(bar, mark)| bar == measure && mark.custom.is_some());
    Some(if custom { quoted(&label) } else { label })
}

/// One line per bar of `from..=to` that starts with something every part
/// shares: the signatures and tempo in effect at `from`, then each change,
/// each rehearsal mark and the bar's form (repeats, endings, jumps).
pub(super) fn bar_changes(score: &Score, from: usize, to: usize) -> Vec<String> {
    (from..=to)
        .filter_map(|measure| {
            let mut items = Vec::new();
            let first = measure == from;
            // `*_change_at(0)` is the opening value, so bar 1 counts only as `first`.
            if first || measure > 0 && score.time_change_at(measure).is_some() {
                items.push(format!(
                    "compás de tiempo {}",
                    time_text(score.time_at(measure))
                ));
            }
            if first || measure > 0 && score.key_change_at(measure).is_some() {
                items.push(format!(
                    "tonalidad {}",
                    key_text(score.key_signature_at(measure))
                ));
            }
            if first || measure > 0 && score.tempo_change_at(measure).is_some() {
                items.push(format!("tempo {}", tempo_text(score.tempo_at(measure))));
            }
            if let Some(label) = rehearsal_text(score, measure) {
                items.push(format!("ensayo {label}"));
            }
            items.extend(form_items(score, measure));
            (!items.is_empty()).then(|| format!("Compás {}: {}.", measure + 1, items.join(" · ")))
        })
        .collect()
}

/// The marks of a bar's form, as the ribbon names them.
fn form_items(score: &Score, measure: usize) -> Vec<String> {
    let form = score.bar_form(measure);
    let mut items = Vec::new();
    if form.repeat_start {
        items.push("inicio de repetición".to_owned());
    }
    if let Some(ending) = score.endings().find(|ending| ending.start == measure) {
        let bars = if ending.end == ending.start {
            String::new()
        } else {
            format!(" hasta el compás {}", ending.end + 1)
        };
        items.push(format!("casilla {}{bars}", ending.label()));
    }
    if form.segno {
        items.push("segno".to_owned());
    }
    if form.coda {
        items.push("coda".to_owned());
    }
    if form.to_coda {
        items.push("To Coda".to_owned());
    }
    if form.fine {
        items.push("Fine".to_owned());
    }
    if let Some(jump) = form.jump {
        items.push(jump.label().to_owned());
    }
    match form.repeat_end {
        Some(2) => items.push("fin de repetición".to_owned()),
        Some(times) => items.push(format!("fin de repetición, {times} veces")),
        None => {}
    }
    match form.barline {
        Barline::Single => {}
        Barline::Double => items.push("doble barra al final".to_owned()),
        Barline::Final => items.push("barra final".to_owned()),
    }
    items
}

pub(super) const REHEARSAL_STYLE: &str = "Estilo de las marcas de ensayo";

/// The settings of the whole score that no bar shows, `(name, value)`.
pub(super) fn score_settings(score: &Score) -> Vec<(&'static str, String)> {
    vec![
        (REHEARSAL_STYLE, score.rehearsal_style().label().to_owned()),
        (
            "Cifrado americano",
            if score.chords_sound() {
                "se oye al reproducir".to_owned()
            } else {
                "solo escrito, no se oye".to_owned()
            },
        ),
    ]
}

/// The part's number and name, and the key its players read when it is not
/// the work's key (a transposing instrument).
pub(super) fn part_header(score: &Score, part: PartId, from: usize, to: usize) -> String {
    let mut header = format!("{} {}", part_number(score, part), part_label(score, part));
    let concert = score
        .part(part)
        .is_none_or(|p| p.transposition().is_concert());
    if !concert {
        header.push_str(&format!(
            " · escrita en {}",
            key_text(score.written_key_at(part, from))
        ));
        for measure in from + 1..=to {
            let key = score.written_key_at(part, measure);
            if key != score.written_key_at(part, measure - 1) {
                header.push_str(&format!(
                    "; desde el compás {}: {}",
                    measure + 1,
                    key_text(key)
                ));
            }
        }
    }
    header
}

/// One line per staff, voice and bar: `c3: C4:4 »D4:4 …`. The staff is named
/// only in parts with two, and voice 2 only where it has events.
pub(super) fn staff_lines(
    score: &Score,
    part: PartId,
    staves: &[StaffId],
    from: usize,
    to: usize,
    selected: &[ElementId],
) -> Vec<String> {
    let Some(source) = score.part(part) else {
        return vec!["  (la parte ya no existe)".into()];
    };
    let named_staves = source.staves().len() > 1;
    let mut lines = Vec::new();
    for measure in from..=to {
        for &staff in staves {
            let Some(bar) = source.staff(staff).and_then(|s| s.measures().get(measure)) else {
                continue;
            };
            let two_voices = bar.voice_len(Voice::Two) > 0;
            for voice in Voice::ALL {
                if voice == Voice::Two && !two_voices {
                    continue;
                }
                let mut label = format!("c{}", measure + 1);
                if named_staves {
                    label.push_str(match staff {
                        StaffId::Upper => " p1",
                        StaffId::Lower => " p2",
                    });
                }
                if two_voices {
                    label.push_str(&format!(" v{}", voice.number()));
                }
                let text = score_text::format_voice(score, part, staff, measure, voice);
                let events = if text.is_empty() {
                    "(vacío)".to_owned()
                } else {
                    let ids = bar.voice_iter(voice).map(|(event, _)| ElementId {
                        part,
                        staff,
                        measure,
                        event,
                    });
                    tokens(&text)
                        .into_iter()
                        .zip(ids)
                        .map(|(token, id)| {
                            if selected.contains(&id) {
                                format!("»{token}")
                            } else {
                                token.to_owned()
                            }
                        })
                        .collect::<Vec<_>>()
                        .join(" ")
                };
                lines.push(format!("  {label}: {events}"));
            }
        }
    }
    lines
}

/// The events of a formatted voice: spaces inside a chord or a mark block
/// do not separate events.
fn tokens(text: &str) -> Vec<&str> {
    let mut tokens = Vec::new();
    let (mut depth, mut start) = (0u32, None);
    for (offset, c) in text.char_indices() {
        match c {
            '[' | '{' => depth += 1,
            ']' | '}' => depth = depth.saturating_sub(1),
            c if c.is_whitespace() && depth == 0 => {
                if let Some(begin) = start.take() {
                    tokens.push(&text[begin..offset]);
                }
                continue;
            }
            _ => {}
        }
        start.get_or_insert(offset);
    }
    if let Some(begin) = start {
        tokens.push(&text[begin..]);
    }
    tokens
}

/// A place in the score as a musician says it: «Viola, compás 2, tiempo 3».
pub(super) fn place(score: &Score, id: ElementId) -> String {
    let mut text = format!("{}, compás {}", part_label(score, id.part), id.measure + 1);
    if let Some(tick) = score
        .event_tick(id)
        .map(|tick| (tick - score.measure_start_tick(id.measure)) as u16)
    {
        text.push_str(&format!(
            ", tiempo {}",
            beat_label(tick, beat_unit(score, id.measure))
        ));
    }
    if score.part(id.part).is_some_and(|p| p.staves().len() > 1) {
        text.push_str(match id.staff {
            StaffId::Upper => ", pentagrama 1",
            StaffId::Lower => ", pentagrama 2",
        });
    }
    if let Some(voice) = score.voice_of(id).filter(|voice| *voice == Voice::Two) {
        text.push_str(&format!(", voz {}", voice.number()));
    }
    text
}
