//! Musical positions as an agent says them ("beat 3 of bar 4 of the violas"),
//! resolved against a score. Errors are Spanish sentences meant to be shown as
//! they are.
use crate::part_label;
use score_core::{ElementId, PartId, Score, StaffId, TICKS_PER_WHOLE, Voice};

/// Part by 1-based number or by name; ignores case and accents, accepts a
/// prefix and a simple plural ("violas" finds "Viola").
pub fn resolve_part(score: &Score, query: &str) -> Result<PartId, String> {
    let parts: Vec<(PartId, String)> = score
        .parts()
        .iter()
        .map(|part| (part.id(), score.part_name(part.id())))
        .collect();
    let query = query.trim();
    if !query.is_empty() && query.chars().all(|c| c.is_ascii_digit()) {
        return query
            .parse::<usize>()
            .ok()
            .and_then(|number| number.checked_sub(1))
            .and_then(|index| parts.get(index))
            .map(|(id, _)| *id)
            .ok_or_else(|| format!("No hay una parte número {query}. {}", listing(score)));
    }
    let folded = fold(query);
    if folded.is_empty() {
        return Err(format!("Falta indicar la parte. {}", listing(score)));
    }
    let mut variants = vec![folded.as_str()];
    for suffix in ["es", "s"] {
        if let Some(singular) = folded.strip_suffix(suffix)
            && !singular.is_empty()
        {
            variants.push(singular);
        }
    }
    let names: Vec<String> = parts.iter().map(|(_, name)| fold(name)).collect();
    // An exact name beats a prefix: "violín II" or "viola" never match a longer name.
    let matching = |matches: fn(&str, &str) -> bool| -> Vec<usize> {
        (0..parts.len())
            .filter(|&i| variants.iter().any(|v| matches(&names[i], v)))
            .collect()
    };
    let mut found = matching(|name, variant| name == variant);
    if found.is_empty() {
        found = matching(|name, variant| name.starts_with(variant));
    }
    match found[..] {
        [] => Err(format!(
            "Ninguna parte se llama «{query}». {}",
            listing(score)
        )),
        [index] => Ok(parts[index].0),
        _ => {
            let candidates: Vec<_> = found
                .iter()
                .map(|&i| format!("{} {}", i + 1, part_label(score, parts[i].0)))
                .collect();
            Err(format!(
                "«{query}» puede ser varias partes: {}. Pregunta al usuario cuál quiere: no elijas tú.",
                candidates.join(", ")
            ))
        }
    }
}

/// 0-based measure index of a bar number as Levare prints it: bars count from
/// 1 and a pickup bar is bar 1 (the layout shows `index + 1` on every bar).
pub fn bar_index(score: &Score, bar: u32) -> Result<usize, String> {
    let count = score.measures().len();
    match (bar as usize).checked_sub(1) {
        Some(index) if index < count => Ok(index),
        _ => Err(format!(
            "El compás {bar} no existe: la partitura tiene {count} compases (del 1 al {count})."
        )),
    }
}

/// How far from a tick a beat may be, in beats: half a unit of the third decimal
/// that `beat_label` prints, with slack for `f32` rounding.
const BEAT_TOLERANCE: f32 = 0.001;

/// Tick of a 1-based beat in a measure, counted in units of the time
/// signature's denominator (eighth notes in 6/8).
pub fn beat_tick(score: &Score, measure: usize, beat: f32) -> Result<u16, String> {
    if measure >= score.measures().len() {
        return Err(format!("El compás {} no existe.", measure + 1));
    }
    let time = score.time_at(measure);
    let unit = TICKS_PER_WHOLE / u16::from(time.denominator.max(1));
    if !beat.is_finite() || beat < 1. {
        return Err(format!(
            "El tiempo {beat} no es válido: los tiempos empiezan en 1."
        ));
    }
    let ticks = (beat - 1.) * f32::from(unit);
    // Onset labels carry three decimals of a beat, so accept what they round to.
    if (ticks - ticks.round()).abs() > BEAT_TOLERANCE * f32::from(unit) {
        return Err(format!(
            "El tiempo {beat} no cae en una posición exacta del compás {}.",
            measure + 1
        ));
    }
    let tick = ticks.round() as u16;
    if tick >= time.capacity() {
        return Err(format!(
            "El compás {} es de {}/{}: el tiempo debe estar entre 1 y {} (recibido {beat}).",
            measure + 1,
            time.numerator,
            time.denominator,
            time.numerator,
        ));
    }
    Ok(tick)
}

/// The event that starts exactly at `tick` in one voice of one measure.
pub fn event_at(
    score: &Score,
    part: PartId,
    staff: StaffId,
    voice: Voice,
    measure: usize,
    tick: u16,
) -> Result<ElementId, String> {
    let name = part_label(score, part);
    let Some(source) = score.part(part).and_then(|p| p.staff(staff)) else {
        return Err(format!("La parte {name} no tiene ese pentagrama."));
    };
    let Some(bar) = source.measures().get(measure) else {
        return Err(format!("El compás {} no existe.", measure + 1));
    };
    let onsets: Vec<_> = bar
        .timed_iter()
        .filter(|(v, ..)| *v == voice)
        .map(|(_, start, id, _)| (start, id))
        .collect();
    if let Some(&(_, event)) = onsets.iter().find(|(start, _)| *start == tick) {
        return Ok(ElementId {
            part,
            staff,
            measure,
            event,
        });
    }
    let place = format!(
        "compás {} de {name} (voz {})",
        measure + 1,
        match voice {
            Voice::One => 1,
            Voice::Two => 2,
        }
    );
    if onsets.is_empty() {
        return Err(format!("No hay eventos en el {place}."));
    }
    let unit = TICKS_PER_WHOLE / u16::from(score.time_at(measure).denominator.max(1));
    let beats: Vec<_> = onsets
        .iter()
        .map(|(start, _)| beat_label(*start, unit))
        .collect();
    Err(format!(
        "En el {place} no empieza ningún evento en el tiempo {}. Empiezan en {}.",
        beat_label(tick, unit),
        beats.join(", ")
    ))
}

fn beat_label(tick: u16, unit: u16) -> String {
    let beat = format!("{:.3}", f32::from(tick) / f32::from(unit) + 1.);
    beat.trim_end_matches('0').trim_end_matches('.').to_owned()
}

fn listing(score: &Score) -> String {
    let all: Vec<_> = score
        .parts()
        .iter()
        .enumerate()
        .map(|(i, part)| format!("{} {}", i + 1, part_label(score, part.id())))
        .collect();
    format!("Partes disponibles: {}.", all.join(", "))
}

/// Lowercase with Spanish accents removed, so "Violín" and "VIOLIN" compare equal.
fn fold(text: &str) -> String {
    text.trim()
        .chars()
        .flat_map(char::to_lowercase)
        .map(|c| match c {
            'á' => 'a',
            'é' => 'e',
            'í' => 'i',
            'ó' => 'o',
            'ú' | 'ü' => 'u',
            'ñ' => 'n',
            c => c,
        })
        .collect()
}
