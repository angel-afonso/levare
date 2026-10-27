//! The Selection tab: draws `SelectionView`. Every button runs a command or
//! an editor that already exists; nothing here repeats the ribbon.
use super::selection_view::{
    EditorKind, MarkGlyph, MarkRow, NoteSummary, PassageSummary, ScoreSummary, SelectionView,
    selection_view,
};
use super::toolbar::control;
use super::*;

pub(super) fn panel(view: &AppView, cx: &Context<AppView>) -> Div {
    let editor = &view.document.editor;
    let body = div()
        .flex()
        .flex_col()
        .gap_4()
        .px(px(14.))
        .pt(px(14.))
        .pb(px(20.));
    match selection_view(editor.score(), editor, view.open_editor()) {
        SelectionView::Score(summary) => score(body, view, &summary, cx),
        SelectionView::Note(summary) => note(body, view, &summary, cx),
        SelectionView::Rest(summary) => note(body, view, &summary, cx),
        SelectionView::Passage(summary) => passage(body, view, &summary, cx),
        SelectionView::Editor(kind) => open_editor(body, view, kind, cx),
    }
}

fn header(title: impl Into<SharedString>, detail: impl Into<SharedString>) -> Div {
    div()
        .flex()
        .flex_col()
        .gap(px(2.))
        .child(
            div()
                .text_size(px(15.))
                .font_weight(FontWeight::SEMIBOLD)
                .child(title.into()),
        )
        .child(
            div()
                .text_size(px(12.5))
                .text_color(rgb(SECONDARY))
                .child(detail.into()),
        )
}

/// A group of rows under a quiet name.
fn group(name: &'static str) -> Div {
    div().flex().flex_col().gap(px(6.)).child(
        div()
            .text_size(px(12.))
            .font_weight(FontWeight::MEDIUM)
            .text_color(rgb(MUTED))
            .child(name),
    )
}

fn fact(label: &'static str, value: impl Into<SharedString>) -> Div {
    div()
        .flex()
        .items_center()
        .justify_between()
        .gap_2()
        .min_h(px(28.))
        .px_2()
        .text_size(px(12.5))
        .child(div().text_color(rgb(SECONDARY)).child(label))
        .child(
            div()
                .font_weight(FontWeight::MEDIUM)
                .truncate()
                .child(value.into()),
        )
}

/// A fact that opens its editor.
fn fact_button(
    view: &AppView,
    id: &'static str,
    label: &'static str,
    value: impl Into<SharedString>,
    cx: &Context<AppView>,
    click: impl Fn(&mut AppView, &mut Window, &mut Context<AppView>) + 'static,
) -> Stateful<Div> {
    let value: SharedString = value.into();
    control(
        id,
        div()
            .flex()
            .w_full()
            .items_center()
            .justify_between()
            .gap_2()
            .child(div().text_color(rgb(SECONDARY)).child(label))
            .child(
                div()
                    .flex()
                    .min_w_0()
                    .gap_1()
                    .font_weight(FontWeight::MEDIUM)
                    .child(inline_text(view.music_font.clone(), value.as_ref()).truncate())
                    .child(div().text_color(rgb(MUTED)).child("›")),
            ),
        false,
        cx,
        click,
    )
    .w_full()
    .min_h(px(28.))
    .px_2()
    .text_size(px(12.5))
}

/// A command with its name and, when it has one, its key.
fn command_control(
    view: &AppView,
    element: &'static str,
    label: &'static str,
    command: CommandId,
    cx: &Context<AppView>,
) -> Stateful<Div> {
    control(
        element,
        div()
            .flex()
            .items_center()
            .gap(px(6.))
            .child(label)
            .children(
                view.shortcuts
                    .label(command)
                    .map(|keys| div().text_size(px(11.)).text_color(rgb(MUTED)).child(keys)),
            ),
        false,
        cx,
        move |v, window, cx| v.run(command, window, cx),
    )
    .h(px(30.))
    .px(px(10.))
    .text_size(px(12.5))
    .tooltip(command_hint(view, command))
}

fn empty(text: &'static str) -> Div {
    div()
        .px_2()
        .text_size(px(12.5))
        .text_color(rgb(MUTED))
        .child(text)
}

fn score(body: Div, view: &AppView, summary: &ScoreSummary, cx: &Context<AppView>) -> Div {
    let credit = |id, label, value: &str| {
        fact_button(
            view,
            id,
            label,
            if value.is_empty() { "—" } else { value }.to_owned(),
            cx,
            |v, window, cx| v.toggle_header_editor(window, cx),
        )
        .tooltip(command_hint(view, id::SCORE_INFO))
    };
    body.child(header("Partitura", "Nada seleccionado."))
        .child(
            group("Cabecera")
                .child(credit("score-title", "Título", &summary.title))
                .child(credit("score-subtitle", "Subtítulo", &summary.subtitle))
                .child(credit("score-composer", "Compositor", &summary.composer)),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(2.))
                .child(
                    fact_button(
                        view,
                        "score-key",
                        "Tonalidad",
                        summary.key.clone(),
                        cx,
                        |v, _, cx| {
                            v.toggle_key_picker(cx);
                            if v.key_picker {
                                v.sign_from_start(cx);
                            }
                        },
                    )
                    .tooltip(command_hint(view, id::KEY_SIGNATURE)),
                )
                .child(
                    fact_button(
                        view,
                        "score-time",
                        "Métrica",
                        summary.time.clone(),
                        cx,
                        |v, _, cx| {
                            v.toggle_time_picker(cx);
                            if v.time_picker {
                                v.sign_from_start(cx);
                            }
                        },
                    )
                    .tooltip(command_hint(view, id::TIME_SIGNATURE)),
                )
                .child(
                    fact_button(
                        view,
                        "score-tempo",
                        "Tempo inicial",
                        summary.tempo.clone(),
                        cx,
                        |v, _, cx| v.toggle_tempo_at(0, cx),
                    )
                    .tooltip(command_hint(view, id::TEMPO_TEXT)),
                )
                .child(fact("Compases", summary.bars.to_string()))
                .child(fact("Partes", summary.parts.to_string()))
                .child(
                    div()
                        .id("score-duration")
                        .child(fact("Duración", summary.duration.clone()))
                        .tooltip(described_hint(
                            "Duración",
                            None,
                            "A los tempos escritos, sin repeticiones ni calderones.",
                        )),
                ),
        )
}

fn note(body: Div, view: &AppView, summary: &NoteSummary, cx: &Context<AppView>) -> Div {
    let font = view.music_font.clone();
    let place = format!(
        "Compás {}, tiempo {} · voz {}",
        summary.bar, summary.beat, summary.voice
    );
    let rest = summary.pitch.is_none();
    body.child(header(summary.part.clone(), place))
        .child(
            div()
                .id("selected-pitch")
                .flex()
                .items_end()
                .gap(px(10.))
                .px_2()
                .child(
                    div()
                        .text_size(px(28.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(inline_text(
                            font.clone(),
                            summary.pitch.as_deref().unwrap_or("Silencio"),
                        )),
                )
                .child(
                    div()
                        .pb(px(5.))
                        .text_color(rgb(SECONDARY))
                        .child(summary.duration.to_lowercase()),
                )
                .tooltip(described_hint(
                    if rest { "Silencio" } else { "Altura escrita" },
                    None,
                    "A–G escribe detrás de la nota seleccionada o sustituye el silencio. ↑/↓ cambian la altura. Backspace convierte en silencio. La figura se elige en la cinta de Entrada.",
                )),
        )
        .when(!summary.tones.is_empty(), |body| {
            body.child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap_1()
                    .children(summary.tones.iter().enumerate().map(|(index, name)| {
                        control(
                            format!("tone-{index}"),
                            inline_text(font.clone(), name),
                            index == summary.tone,
                            cx,
                            move |v, _, cx| {
                                if !v.busy && !v.confirming && !v.agent_locked() {
                                    v.document.editor.select_tone(index);
                                    cx.notify();
                                }
                            },
                        )
                        .h(px(26.))
                        .px_2()
                        .text_size(px(12.))
                    })),
            )
        })
        .when_some(summary.sounding.clone(), |body, sounding| {
            body.child(fact("Suena", sounding))
        })
        .when_some(summary.range_warning.clone(), |body, warning| {
            body.child(
                div()
                    .px(px(10.))
                    .py_2()
                    .rounded_md()
                    .bg(rgb(ERROR_BG))
                    .text_size(px(12.5))
                    .text_color(rgb(ERROR))
                    .child(inline_text(font.clone(), warning)),
            )
        })
        .child(
            group(if rest {
                "Marcas en este silencio"
            } else {
                "Marcas en esta nota"
            })
            .child(marks(
                view,
                &summary.marks,
                false,
                "Ninguna. Añádelas desde Articulaciones o Dinámicas.",
                cx,
            )),
        )
        .child(
            group(if rest {
                "Texto en este silencio"
            } else {
                "Texto en esta nota"
            })
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap(px(6.))
                    .child(command_control(view, "note-lyric", "Letra", id::LYRICS, cx))
                    .child(command_control(
                        view,
                        "note-chord",
                        "Cifrado",
                        id::CHORD_SYMBOL,
                        cx,
                    ))
                    .child(command_control(
                        view,
                        "note-comment",
                        "Comentario",
                        id::NEW_COMMENT,
                        cx,
                    )),
            ),
        )
}

fn passage(body: Div, view: &AppView, summary: &PassageSummary, cx: &Context<AppView>) -> Div {
    let bars = if summary.first_bar == summary.last_bar {
        format!("compás {}", summary.first_bar)
    } else {
        format!("compases {}–{}", summary.first_bar, summary.last_bar)
    };
    body.child(header(
        "Pasaje",
        format!("{} · {bars}", summary.parts.join(" y ")),
    ))
    .child(
        div()
            .flex()
            .flex_col()
            .child(fact("Notas", summary.notes.to_string()))
            .child(fact("Partes", summary.parts.len().to_string()))
            .child(fact("Marcas", summary.marks.len().to_string())),
    )
    .child(
        group("Hacer con el pasaje").child(
            div().flex().flex_wrap().gap(px(6.)).children(
                [
                    ("passage-step-up", "Subir un grado", id::STEP_UP),
                    ("passage-step-down", "Bajar un grado", id::STEP_DOWN),
                    ("passage-octave-up", "Subir una octava", id::OCTAVE_UP),
                    ("passage-octave-down", "Bajar una octava", id::OCTAVE_DOWN),
                    ("passage-copy", "Copiar", id::COPY),
                    ("passage-rests", "En silencios", id::DELETE),
                    ("passage-check", "Comprobar", id::CHECK_SELECTION),
                ]
                .into_iter()
                .map(|(element, label, command)| {
                    command_control(view, element, label, command, cx)
                }),
            ),
        ),
    )
    .child(group("Marcas en el pasaje").child(marks(
        view,
        &summary.marks,
        true,
        "Sin marcas.",
        cx,
    )))
}

fn marks(
    view: &AppView,
    rows: &[MarkRow],
    with_bar: bool,
    none: &'static str,
    cx: &Context<AppView>,
) -> Div {
    if rows.is_empty() {
        return div().child(empty(none));
    }
    div()
        .flex()
        .flex_col()
        .gap(px(2.))
        .children(rows.iter().enumerate().map(|(index, row)| {
            let remove = row.remove;
            let label = row.label.clone();
            div()
                .flex()
                .items_center()
                .gap_2()
                .min_h(px(30.))
                .pl_2()
                .pr_1()
                .rounded_md()
                .bg(rgb(CARD))
                .child(
                    div()
                        .w(px(30.))
                        .flex_shrink_0()
                        .flex()
                        .justify_center()
                        .child(mark_glyph(view, &row.glyph)),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .text_size(px(12.5))
                        .child(inline_text(view.music_font.clone(), &row.label)),
                )
                .when(with_bar, |line| {
                    line.child(
                        div()
                            .flex_shrink_0()
                            .text_size(px(11.))
                            .text_color(rgb(MUTED))
                            .child(format!("c. {}", row.bar)),
                    )
                })
                .child(
                    control(
                        format!("remove-mark-{index}"),
                        "✕",
                        false,
                        cx,
                        move |v, _, cx| {
                            if v.try_edit(remove.command(), cx) {
                                v.status = match v.shortcuts.label(id::UNDO) {
                                    Some(keys) => format!("Quitado: {label}. {keys} lo recupera."),
                                    None => format!("Quitado: {label}."),
                                };
                            }
                        },
                    )
                    .w(px(26.))
                    .h(px(26.))
                    .flex_shrink_0()
                    .text_color(rgb(MUTED))
                    .tooltip(hover_hint(format!("Quitar {}", row.label), None)),
                )
        }))
}

fn mark_glyph(view: &AppView, glyph: &MarkGlyph) -> AnyElement {
    match glyph {
        MarkGlyph::Music(symbol) => {
            fitted_music_symbol(view, symbol, (24., 22.), TEXT).into_any_element()
        }
        MarkGlyph::Curve { slur } => music_connection(view, 22., TEXT, *slur).into_any_element(),
        MarkGlyph::Words(words) => div()
            .max_w(px(30.))
            .truncate()
            .font_family("Times New Roman")
            .italic()
            .text_size(px(13.))
            .child(words.clone())
            .into_any_element(),
    }
}

fn open_editor(body: Div, view: &AppView, kind: EditorKind, cx: &Context<AppView>) -> Div {
    let editor = &view.document.editor;
    let score = editor.score();
    let from = view.signature_measure();
    let scope = if from == 0 {
        "Desde el inicio".to_owned()
    } else {
        format!("Desde el compás {}", from + 1)
    };
    let place = |target: score_core::ElementId| {
        format!(
            "{} · compás {}",
            score.part_name(target.part),
            target.measure + 1
        )
    };
    let (title, detail): (String, String) = match kind {
        EditorKind::Clef => (
            "Clave".into(),
            format!(
                "{} · {}",
                score.part_name(editor.cursor().part),
                scope.to_lowercase()
            ),
        ),
        EditorKind::Key => (
            "Tonalidad".into(),
            format!("{scope}, en todos los instrumentos."),
        ),
        EditorKind::Time => (
            "Métrica".into(),
            format!("{scope} hasta el siguiente cambio."),
        ),
        EditorKind::Tempo => (
            "Tempo".into(),
            format!("{scope} hasta el siguiente cambio."),
        ),
        EditorKind::Text => view
            .text_editor
            .as_ref()
            .map_or_else(Default::default, |text| {
                (text.kind.label().to_owned(), place(text.target))
            }),
        EditorKind::Lyric => view
            .lyric_editor
            .as_ref()
            .map_or_else(Default::default, |lyric| {
                ("Letra".into(), place(lyric.target))
            }),
        EditorKind::Chord => view
            .chord_editor
            .as_ref()
            .map_or_else(Default::default, |chord| {
                ("Cifrado".into(), place(chord.target))
            }),
        EditorKind::ScoreInfo => (
            "Información de la partitura".into(),
            "Cabecera de la primera página.".into(),
        ),
    };
    body.child(header(title, detail)).child(match kind {
        EditorKind::Clef => clef_picker::panel(view, cx),
        EditorKind::Key => key_picker(view, cx),
        EditorKind::Time => time_picker(view, cx),
        EditorKind::Tempo => tempo_editor::panel(view, cx),
        EditorKind::Text => staff_text::panel(view, cx),
        EditorKind::Lyric => staff_text::lyric_panel(view, cx),
        EditorKind::Chord => staff_text::chord_panel(view, cx),
        EditorKind::ScoreInfo => header_editor::panel(view, cx),
    })
}
