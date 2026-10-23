//! Whole bars of one voice in one call: all of them are written, or none.
use super::text::{beats, part_header, staff_lines, time_text};
use super::{EscribirArgs, staff_of, voice_of};
use score_core::{Connection, ConnectionKind, ElementId, Score};
use score_edit::{Command, Cursor, Editor};
use score_text::WrittenEvent;

pub(super) fn write(editor: &mut Editor, args: &EscribirArgs) -> Result<String, String> {
    let score = editor.score();
    let part = score_text::resolve_part(score, &args.parte)?;
    let staff = staff_of(args.pentagrama)?;
    let voice = voice_of(args.voz)?;
    if score.part(part).and_then(|p| p.staff(staff)).is_none() {
        return Err(format!(
            "La parte {} no tiene pentagrama {}.",
            score.part_name(part),
            args.pentagrama.unwrap_or(1)
        ));
    }
    if args.compases.is_empty() {
        return Err("No hay compases que escribir: «compases» está vacío.".into());
    }
    let count = score.measures().len();
    let first = match (args.desde_compas as usize).checked_sub(1) {
        Some(first) if first <= count => first,
        _ => {
            return Err(format!(
                "No se puede empezar en el compás {}: la partitura tiene {count} compases; para añadir compases al final, empieza en el compás {}.",
                args.desde_compas,
                count + 1
            ));
        }
    };
    // Every bar is checked before the score is touched.
    let bars = args
        .compases
        .iter()
        .enumerate()
        .map(|(offset, text)| parse_bar(score, first + offset, text))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("No se ha escrito nada. {error}"))?;
    let mark = editor.mark();
    match apply(editor, part, staff, voice, first, &bars) {
        Ok(()) => {
            editor.squash(&mark);
            let score = editor.score();
            let last = first + bars.len() - 1;
            let bars = if first == last {
                format!("el compás {}", first + 1)
            } else {
                format!("los compases {} a {}", first + 1, last + 1)
            };
            let mut lines = vec![format!("Escrito {bars} de {}:", score.part_name(part))];
            lines.push(part_header(score, part, first, last));
            lines.extend(staff_lines(score, part, &[staff], first, last, &[]));
            Ok(lines.join("\n"))
        }
        Err(error) => {
            editor.rollback(&mark);
            Err(format!("No se ha escrito nada. {error}"))
        }
    }
}

/// The events of one bar, which must fill it exactly.
fn parse_bar(score: &Score, measure: usize, text: &str) -> Result<Vec<WrittenEvent>, String> {
    let bar = measure + 1;
    let events = score_text::parse_voice(text)
        .map_err(|error| format!("En el compás {bar}: {}", error.message))?;
    let time = score.time_at(measure);
    let capacity = time.capacity();
    let unit = super::text::beat_unit(score, measure);
    let total: u32 = events
        .iter()
        .map(|written| u32::from(written.event.duration.ticks()))
        .sum();
    let total = u16::try_from(total).unwrap_or(u16::MAX);
    if total != capacity {
        let (verb, difference) = if total > capacity {
            ("sobra", total - capacity)
        } else {
            ("falta", capacity - total)
        };
        return Err(format!(
            "El compás {bar} suma {} tiempos, pero un compás de {} tiene {}: {verb} {}.",
            beats(total, unit),
            time_text(time),
            beats(capacity, unit),
            beats(difference, unit)
        ));
    }
    Ok(events)
}

fn apply(
    editor: &mut Editor,
    part: score_core::PartId,
    staff: score_core::StaffId,
    voice: score_core::Voice,
    first: usize,
    bars: &[Vec<WrittenEvent>],
) -> Result<(), String> {
    let last = first + bars.len() - 1;
    while editor.score().measures().len() <= last {
        let bar = editor.score().measures().len() + 1;
        editor
            .execute(Command::AppendMeasure)
            .map_err(|error| format!("Al añadir el compás {bar}: {}", error.0))?;
    }
    editor.set_cursor(Cursor {
        part,
        staff,
        voice,
        measure: first,
        index: 0,
    });
    let cursor = editor.cursor();
    if (cursor.part, cursor.staff, cursor.voice) != (part, staff, voice) {
        return Err("No se pudo colocar el cursor en esa parte, pentagrama y voz.".into());
    }
    // Each written event with its bar, in order, for the ties.
    let mut written: Vec<(usize, ElementId, bool)> = Vec::new();
    for (offset, events) in bars.iter().enumerate() {
        let measure = first + offset;
        let mut tick = 0;
        for (index, item) in events.iter().enumerate() {
            editor
                .execute(Command::WriteAt {
                    measure,
                    tick,
                    event: item.event,
                })
                .map_err(|error| {
                    format!(
                        "En el compás {}, evento {}: {}",
                        measure + 1,
                        index + 1,
                        error.0
                    )
                })?;
            let id = editor.selection().ok_or_else(|| {
                format!("En el compás {}: el evento no quedó escrito.", measure + 1)
            })?;
            written.push((measure, id, item.tie_to_next));
            tick += item.event.duration.ticks();
        }
    }
    for (index, &(measure, start, tied)) in written.iter().enumerate() {
        if !tied {
            continue;
        }
        let end = match written.get(index + 1) {
            Some(&(_, next, _)) => next,
            None => following(editor.score(), start).ok_or_else(|| {
                format!(
                    "En el compás {}: la última nota lleva «~», pero no la sigue ninguna nota en esa voz.",
                    measure + 1
                )
            })?,
        };
        editor
            .execute(Command::AddConnection(Connection {
                start,
                end,
                kind: ConnectionKind::Tie,
            }))
            .map_err(|error| {
                format!(
                    "En el compás {}, la ligadura de unión «~»: {}",
                    measure + 1,
                    error.0
                )
            })?;
    }
    Ok(())
}

/// The first event of the same voice in the bar after `id`'s, already in the
/// score, so the last written note may tie into music that follows.
fn following(score: &Score, id: ElementId) -> Option<ElementId> {
    let measure = id.measure + 1;
    let voice = score.voice_of(id)?;
    let event = score
        .part(id.part)?
        .staff(id.staff)?
        .measures()
        .get(measure)?
        .voice_id_at(voice, 0)?;
    Some(ElementId {
        measure,
        event,
        ..id
    })
}
