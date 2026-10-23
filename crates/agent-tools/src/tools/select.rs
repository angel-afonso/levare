//! Selecting a musical position and running a registry command on it. Each
//! reply shows the selected bars, so the agent sees what a toggle did.
use super::text::{bar_changes, part_header, place, score_settings, staff_lines};
use super::{EjecutarArgs, SeleccionarArgs, resolve};
use crate::catalog::alternative;
use crate::{Host, allowed_commands};
use score_core::{ElementId, PartId, Score, StaffId};
use score_edit::Editor;
use score_keymap::{CommandSpec, Registry, Status};

/// Most bars a reply shows of a long selection.
const MAX_SHOWN_BARS: usize = 32;

/// The staff and bars of the selection, or the cursor's bar without one.
#[derive(Clone, Copy, PartialEq, Eq)]
struct Focus {
    part: PartId,
    staff: StaffId,
    from: usize,
    to: usize,
}

fn focus(editor: &Editor) -> Focus {
    let ids = editor.selected_ids();
    match (ids.first(), ids.last()) {
        (Some(first), Some(last)) => Focus {
            part: first.part,
            staff: first.staff,
            from: first.measure.min(last.measure),
            to: first.measure.max(last.measure),
        },
        _ => {
            let cursor = editor.cursor();
            Focus {
                part: cursor.part,
                staff: cursor.staff,
                from: cursor.measure,
                to: cursor.measure,
            }
        }
    }
}

/// The focus bars of one staff, every voice, with `»` before `selected`.
fn block(score: &Score, focus: Focus, selected: &[ElementId]) -> Vec<String> {
    if score.part(focus.part).is_none() {
        return vec!["(la parte ya no existe)".into()];
    }
    let last = score.measures().len() - 1;
    let from = focus.from.min(last);
    let end = focus.to.min(last);
    let to = end.min(from + MAX_SHOWN_BARS - 1);
    let mut lines = bar_changes(score, from, to);
    lines.push(part_header(score, focus.part, from, to));
    lines.extend(staff_lines(
        score,
        focus.part,
        &[focus.staff],
        from,
        to,
        selected,
    ));
    if end > to {
        lines.push(format!("  … y {} compases más", end - to));
    }
    lines
}

pub(super) fn select(editor: &mut Editor, args: &SeleccionarArgs) -> Result<String, String> {
    let score = editor.score();
    let start = resolve(score, &args.desde)?;
    let end = args
        .hasta
        .as_ref()
        .map(|position| resolve(score, position))
        .transpose()?;
    if let Some(end) = end
        && (end.part != start.part
            || end.staff != start.staff
            || score.voice_of(end) != score.voice_of(start))
    {
        return Err(
            "Un pasaje empieza y acaba en la misma parte, pentagrama y voz: revisa «hasta».".into(),
        );
    }
    editor.select(start);
    if let Some(end) = end {
        editor.extend_selection(end);
    }
    let score = editor.score();
    let ids = editor.selected_ids();
    let heading = match ids.as_slice() {
        [first, .., last] => format!(
            "Seleccionados {} eventos, de {} a {}.",
            ids.len(),
            place(score, *first),
            place(score, *last)
        ),
        _ => format!("Seleccionado: {}.", place(score, start)),
    };
    let mut lines = vec![heading];
    lines.extend(block(score, focus(editor), &ids));
    Ok(lines.join("\n"))
}

pub(super) fn execute(
    host: &mut dyn Host,
    registry: &Registry,
    args: &EjecutarArgs,
) -> Result<String, String> {
    let spec = find(registry, &args.comando)?;
    let editor = host.editor();
    let before = editor.score().clone();
    let (before_ids, before_cursor) = (editor.selected_ids(), editor.cursor());
    let shown = focus(editor);
    let shown_before = block(&before, shown, &before_ids);
    host.run(spec.id)?;
    let editor = host.editor();
    let score = editor.score();
    let ids = editor.selected_ids();
    let now = focus(editor);
    let name = format!("«{}» ({})", spec.label, spec.id);
    let mut lines = Vec::new();
    if *score == before {
        lines.push(format!("{name}: sin cambios en la partitura."));
        if ids != before_ids || editor.cursor() != before_cursor {
            lines.push("Selección ahora:".into());
            lines.extend(block(score, now, &ids));
        }
        return Ok(lines.join("\n"));
    }
    let shown_after = block(score, shown, &ids);
    let unchanged = shown_after == shown_before;
    // What changed for the whole score, which no bar may show.
    let settings: Vec<String> = score_settings(score)
        .into_iter()
        .zip(score_settings(&before))
        .filter(|(now, then)| now != then)
        .map(|((setting, value), _)| format!("{setting}: {value} (ajuste de toda la partitura)."))
        .collect();
    lines.push(format!("{name} aplicado."));
    lines.extend(settings.iter().cloned());
    lines.push("antes:".into());
    lines.extend(shown_before);
    lines.push("después:".into());
    lines.extend(shown_after);
    if unchanged && settings.is_empty() {
        lines.push(
            "(Estos compases no cambian: el cambio está en otro lugar de la partitura.)".into(),
        );
    }
    if now != shown {
        lines.push("Selección ahora:".into());
        lines.extend(block(score, now, &ids));
    }
    Ok(lines.join("\n"))
}

/// An allowed command by id or exact label.
fn find<'a>(registry: &'a Registry, query: &str) -> Result<&'a CommandSpec, String> {
    let query = query.trim();
    let spec = registry
        .get(query)
        .or_else(|| registry.iter().find(|spec| spec.label == query))
        .or_else(|| {
            let lower = query.to_lowercase();
            registry
                .iter()
                .find(|spec| spec.label.to_lowercase() == lower)
        })
        .ok_or_else(|| {
            format!(
                "No hay ningún comando «{query}». La herramienta comandos lista los disponibles con su id."
            )
        })?;
    if allowed_commands(registry)
        .iter()
        .any(|allowed| allowed.id == spec.id)
    {
        return Ok(spec);
    }
    let reason = if spec.status == Status::Pending {
        "todavía no está implementado en Levare"
    } else {
        "no está disponible para el agente"
    };
    let mut message = format!("El comando «{}» ({}) {reason}.", spec.id, spec.label);
    if let Some(tool) = alternative(spec.id) {
        message.push_str(&format!(" Usa {tool}."));
    }
    Err(message)
}
