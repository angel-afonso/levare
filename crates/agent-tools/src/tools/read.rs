//! The cheap overview, the bounded read and the command catalog.
use super::text::{bar_changes, key_text, part_header, place, staff_lines, tempo_text, time_text};
use super::{ComandosArgs, LeerArgs, fold};
use crate::allowed_commands;
use score_core::{PartId, Score, StaffId};
use score_edit::Editor;
use score_keymap::{Category, Registry};

/// Most bars one `leer` returns: a page of an orchestral score.
const MAX_READ_BARS: u32 = 32;

pub(super) fn summary(editor: &Editor) -> String {
    let score = editor.score();
    let count = score.measures().len();
    let mut lines = vec![
        if score.title().is_empty() {
            "Título: (sin título)".to_owned()
        } else {
            format!("Título: «{}»", score.title())
        },
        format!(
            "La partitura tiene {count} compases y {} partes:",
            score.parts().len()
        ),
    ];
    for (index, part) in score.parts().iter().enumerate() {
        let staves = part.staves().len();
        let mut line = format!("  {} {}", index + 1, score.part_name(part.id()));
        if staves > 1 {
            line.push_str(&format!(" ({staves} pentagramas)"));
        }
        lines.push(line);
    }
    let changes = |describe: &dyn Fn(usize) -> Option<String>| {
        (0..count)
            .filter_map(|measure| {
                describe(measure).map(|text| format!("{text} desde el compás {}", measure + 1))
            })
            .collect::<Vec<_>>()
            .join("; ")
    };
    lines.push(format!(
        "Compás de tiempo: {}.",
        changes(&|m| score.time_change_at(m).map(time_text))
    ));
    lines.push(format!(
        "Tonalidad de la obra: {}.",
        changes(&|m| score.key_change_at(m).map(key_text))
    ));
    lines.push(format!(
        "Tempo: {}.",
        changes(&|m| score.tempo_change_at(m).map(tempo_text))
    ));
    let rehearsals: Vec<_> = score
        .rehearsal_marks()
        .filter_map(|(measure, _)| {
            score
                .rehearsal_label(measure)
                .map(|label| format!("{label} en el compás {}", measure + 1))
        })
        .collect();
    if !rehearsals.is_empty() {
        lines.push(format!("Marcas de ensayo: {}.", rehearsals.join(", ")));
    }
    lines.push(selection_line(editor));
    lines.join("\n")
}

fn selection_line(editor: &Editor) -> String {
    let score = editor.score();
    let ids = editor.selected_ids();
    match ids.as_slice() {
        [] => {
            let cursor = editor.cursor();
            format!(
                "Selección: ninguna. El cursor está en {}, compás {}.",
                score.part_name(cursor.part),
                cursor.measure + 1
            )
        }
        [id] => format!("Selección: {}.", place(score, *id)),
        [first, .., last] => format!(
            "Selección: {} eventos, de {} a {}.",
            ids.len(),
            place(score, *first),
            place(score, *last)
        ),
    }
}

pub(super) fn read(score: &Score, args: &LeerArgs) -> Result<String, String> {
    if args.hasta < args.desde {
        return Err(format!(
            "El rango está al revés: «desde» ({}) debe ser menor o igual que «hasta» ({}).",
            args.desde, args.hasta
        ));
    }
    let bars = args.hasta - args.desde + 1;
    if bars > MAX_READ_BARS {
        return Err(format!(
            "Pide un rango de {MAX_READ_BARS} compases o menos: del {} al {} son {bars}. Lee por fragmentos.",
            args.desde, args.hasta
        ));
    }
    let from = score_text::bar_index(score, args.desde)?;
    let to = score_text::bar_index(score, args.hasta)?;
    let parts: Vec<PartId> = match &args.partes {
        None => score.parts().iter().map(|part| part.id()).collect(),
        Some(queries) => {
            let wanted = queries
                .iter()
                .map(|query| score_text::resolve_part(score, query))
                .collect::<Result<Vec<_>, _>>()?;
            score
                .parts()
                .iter()
                .map(|part| part.id())
                .filter(|id| wanted.contains(id))
                .collect()
        }
    };
    let mut lines = vec![format!(
        "Compases {} a {}. Alturas escritas; sin alteración, la nota sigue la armadura.",
        from + 1,
        to + 1
    )];
    lines.extend(bar_changes(score, from, to));
    for part in parts {
        lines.push(part_header(score, part, from, to));
        let staves: Vec<StaffId> = score
            .part(part)
            .map(|p| p.staves().iter().map(|staff| staff.id()).collect())
            .unwrap_or_default();
        lines.extend(staff_lines(score, part, &staves, from, to, &[]));
    }
    Ok(lines.join("\n"))
}

pub(super) fn commands(registry: &Registry, args: &ComandosArgs) -> Result<String, String> {
    let allowed = allowed_commands(registry);
    let categories: Vec<Category> = Category::ALL
        .into_iter()
        .filter(|category| allowed.iter().any(|spec| spec.category == *category))
        .collect();
    let chosen: Vec<Category> = match &args.categoria {
        None => categories.clone(),
        Some(query) => {
            let query = fold(query);
            categories
                .iter()
                .copied()
                .filter(|category| fold(category.label()).starts_with(&query))
                .collect()
        }
    };
    if chosen.is_empty() {
        let names: Vec<_> = categories.iter().map(|category| category.label()).collect();
        return Err(format!(
            "No hay ninguna categoría «{}». Categorías: {}.",
            args.categoria.as_deref().unwrap_or_default(),
            names.join(", ")
        ));
    }
    let mut lines = Vec::new();
    for category in chosen {
        lines.push(category.label().to_owned());
        lines.extend(
            allowed
                .iter()
                .filter(|spec| spec.category == category)
                .map(|spec| format!("  {} — {}", spec.id, spec.label)),
        );
    }
    Ok(lines.join("\n"))
}
