use super::*;
use score_core::{ElementId, Score, TextKind};
use text_input::TextInput;

/// Words being written at one note or rest.
pub struct StaffTextEditor {
    pub kind: TextKind,
    pub target: ElementId,
    pub field: Entity<TextInput>,
}
impl StaffTextEditor {
    pub fn new(
        score: &Score,
        kind: TextKind,
        target: ElementId,
        cx: &mut Context<AppView>,
    ) -> Self {
        let text = score
            .text(target, kind)
            .map(|words| words.to_string())
            .unwrap_or_default();
        let field =
            cx.new(|cx| TextInput::new(&text, cx).with_max_chars(score_core::MAX_TEXT_BYTES));
        cx.observe(&field, |_, _, cx| cx.notify()).detach();
        Self {
            kind,
            target,
            field,
        }
    }
    pub fn text(&self, cx: &App) -> String {
        self.field.read(cx).value().to_owned()
    }
    pub fn focused(&self, window: &Window, cx: &App) -> bool {
        self.field.focus_handle(cx).is_focused(window)
    }
    pub fn focus(&self, window: &mut Window, cx: &App) {
        self.field.focus_handle(cx).focus(window);
    }
}

pub(super) fn panel(view: &AppView, cx: &Context<AppView>) -> Div {
    let editor = view.text_editor.as_ref().unwrap();
    let kind = editor.kind;
    let score = view.document.editor.score();
    let written = score.text(editor.target, kind).is_some();
    let place = format!(
        "{} · compás {}",
        score.part_name(editor.target.part),
        editor.target.measure + 1
    );
    let meaning = match kind {
        TextKind::Expression => {
            "Cursiva bajo el pentagrama, tras la dinámica. «cresc.» y «dim.» cambian el volumen hasta la siguiente dinámica; «legato» liga las notas."
        }
        TextKind::Technique => {
            "Sobre el pentagrama. «con sord.» suena más suave hasta «senza sord.» u «ord.»; «legato» liga las notas; «pizz.» y «arco» en cuerdas cambian la técnica."
        }
    };
    div()
        .flex()
        .flex_col()
        .gap_2()
        .pb_3()
        .border_b_1()
        .border_color(rgb(BORDER))
        .child(caption(kind.label().to_uppercase()))
        .child(div().text_size(px(11.)).text_color(rgb(MUTED)).child(place))
        .child(editor.field.clone())
        .child(caption("HABITUALES"))
        .child(
            div()
                .flex()
                .flex_wrap()
                .gap_1()
                .children(kind.suggestions().iter().map(|&words| {
                    button(
                        view,
                        format!("staff-text-{words}"),
                        words,
                        editor.text(cx).trim() == words,
                        cx,
                        move |v, _, cx| {
                            if let Some(editor) = &v.text_editor {
                                editor
                                    .field
                                    .update(cx, |field, cx| field.set_value(words, cx));
                            }
                            v.apply_text(cx);
                        },
                    )
                })),
        )
        .child(
            div()
                .flex()
                .gap_2()
                .child(
                    button(view, "apply-staff-text", "Aplicar", true, cx, |v, _, cx| {
                        v.apply_text(cx)
                    })
                    .flex_1(),
                )
                .child(
                    button(
                        view,
                        "cancel-staff-text",
                        "Cancelar",
                        false,
                        cx,
                        |v, _, cx| {
                            v.text_editor = None;
                            cx.notify();
                        },
                    )
                    .flex_1(),
                ),
        )
        .when(written, |panel| {
            panel.child(
                button(
                    view,
                    "remove-staff-text",
                    "Quitar texto",
                    false,
                    cx,
                    |v, _, cx| {
                        if let Some(editor) = v.text_editor.take() {
                            v.edit(Command::SetText(editor.target, editor.kind, None), cx);
                        }
                    },
                )
                .w_full(),
            )
        })
        .child(
            div()
                .text_size(px(10.))
                .text_color(rgb(MUTED))
                .child(format!("{meaning} Enter aplica; Esc cancela.")),
        )
}

/// The syllable being typed under one note, verse by verse.
pub struct LyricEditor {
    pub target: ElementId,
    pub verse: u8,
    pub field: Entity<TextInput>,
}
impl LyricEditor {
    pub fn new(score: &Score, target: ElementId, verse: u8, cx: &mut Context<AppView>) -> Self {
        let field = cx.new(|cx| {
            TextInput::new(&syllable_at(score, target, verse), cx)
                .with_max_chars(score_core::MAX_SYLLABLE_BYTES)
        });
        cx.observe(&field, |_, _, cx| cx.notify()).detach();
        Self {
            target,
            verse,
            field,
        }
    }
    pub fn text(&self, cx: &App) -> String {
        self.field.read(cx).value().trim().to_owned()
    }
    pub fn focused(&self, window: &Window, cx: &App) -> bool {
        self.field.focus_handle(cx).is_focused(window)
    }
    pub fn focus(&self, window: &mut Window, cx: &App) {
        self.field.focus_handle(cx).focus(window);
    }
    /// Move to another note or verse, showing what is sung there.
    pub fn retarget(&mut self, score: &Score, target: ElementId, verse: u8, cx: &mut App) {
        self.target = target;
        self.verse = verse;
        let text = syllable_at(score, target, verse);
        self.field
            .update(cx, |field, cx| field.set_value(&text, cx));
    }
}

fn syllable_at(score: &Score, target: ElementId, verse: u8) -> String {
    score
        .event(target)
        .and_then(|event| event.notation.lyrics.verse(usize::from(verse)))
        .map(|syllable| syllable.text().to_owned())
        .unwrap_or_default()
}

/// The chord symbol being typed at one note or rest.
pub struct ChordEditor {
    pub target: ElementId,
    pub field: Entity<TextInput>,
}
impl ChordEditor {
    pub fn new(score: &Score, target: ElementId, cx: &mut Context<AppView>) -> Self {
        let field = cx.new(|cx| TextInput::new(&chord_at(score, target), cx).with_max_chars(16));
        cx.observe(&field, |_, _, cx| cx.notify()).detach();
        Self { target, field }
    }
    pub fn text(&self, cx: &App) -> String {
        self.field.read(cx).value().trim().to_owned()
    }
    pub fn focused(&self, window: &Window, cx: &App) -> bool {
        self.field.focus_handle(cx).is_focused(window)
    }
    pub fn focus(&self, window: &mut Window, cx: &App) {
        self.field.focus_handle(cx).focus(window);
    }
    pub fn retarget(&mut self, score: &Score, target: ElementId, cx: &mut App) {
        self.target = target;
        let text = chord_at(score, target);
        self.field
            .update(cx, |field, cx| field.set_value(&text, cx));
    }
}

fn chord_at(score: &Score, target: ElementId) -> String {
    score
        .event(target)
        .and_then(|event| event.notation.chord)
        .map(|chord| chord.label())
        .unwrap_or_default()
}

pub(super) fn lyric_panel(view: &AppView, cx: &Context<AppView>) -> Div {
    let editor = view.lyric_editor.as_ref().unwrap();
    let score = view.document.editor.score();
    let place = format!(
        "{} · compás {}",
        score.part_name(editor.target.part),
        editor.target.measure + 1
    );
    let written = score.event(editor.target).is_some_and(|event| {
        event
            .notation
            .lyrics
            .verse(usize::from(editor.verse))
            .is_some()
    });
    div()
        .flex()
        .flex_col()
        .gap_2()
        .pb_3()
        .border_b_1()
        .border_color(rgb(BORDER))
        .child(caption("LETRA"))
        .child(div().text_size(px(11.)).text_color(rgb(MUTED)).child(place))
        .child(
            div()
                .flex()
                .gap_1()
                .children((0..score_core::MAX_VERSES as u8).map(|verse| {
                    button(
                        view,
                        format!("lyric-verse-{verse}"),
                        format!("Estrofa {}", verse + 1),
                        editor.verse == verse,
                        cx,
                        move |v, _, cx| v.choose_verse(verse, cx),
                    )
                    .text_size(px(10.))
                })),
        )
        .child(editor.field.clone())
        .child(
            div()
                .flex()
                .gap_2()
                .child(
                    button(view, "apply-lyric", "Aplicar", true, cx, |v, _, cx| {
                        v.apply_syllable(score_core::Join::Word, false, cx)
                    })
                    .flex_1(),
                )
                .child(
                    button(view, "close-lyric", "Cerrar", false, cx, |v, _, cx| {
                        v.lyric_editor = None;
                        cx.notify();
                    })
                    .flex_1(),
                ),
        )
        .when(written, |panel| {
            panel.child(
                button(view, "remove-lyric", "Quitar sílaba", false, cx, |v, _, cx| {
                    if let Some(editor) = &v.lyric_editor {
                        let (target, verse) = (editor.target, editor.verse);
                        v.edit(Command::SetSyllable(target, verse, None), cx);
                        if let Some(editor) = &mut v.lyric_editor {
                            editor.retarget(v.document.editor.score(), target, verse, cx);
                        }
                    }
                })
                .w_full(),
            )
        })
        .child(div().text_size(px(10.)).text_color(rgb(MUTED)).child(
            "Espacio: siguiente palabra · guion: la palabra sigue en la próxima nota · _: mantener la sílaba en las notas siguientes. Enter aplica; Esc cierra.",
        ))
}

pub(super) fn chord_panel(view: &AppView, cx: &Context<AppView>) -> Div {
    let editor = view.chord_editor.as_ref().unwrap();
    let score = view.document.editor.score();
    let place = format!(
        "{} · compás {}",
        score.part_name(editor.target.part),
        editor.target.measure + 1
    );
    let key = score.written_key_at(editor.target.part, editor.target.measure);
    let written = score
        .event(editor.target)
        .is_some_and(|event| event.notation.chord.is_some());
    div()
        .flex()
        .flex_col()
        .gap_2()
        .pb_3()
        .border_b_1()
        .border_color(rgb(BORDER))
        .child(caption("CIFRADO"))
        .child(div().text_size(px(11.)).text_color(rgb(MUTED)).child(place))
        .child(editor.field.clone())
        .child(caption(format!("EN {}", key.label().to_uppercase())))
        .child(div().flex().flex_wrap().gap_1().children(
            score_core::ChordSymbol::diatonic(key).into_iter().map(|chord| {
                let label = chord.label();
                button(
                    view,
                    format!("chord-{label}"),
                    label.clone(),
                    editor.text(cx) == label,
                    cx,
                    move |v, _, cx| {
                        if let Some(editor) = &v.chord_editor {
                            editor
                                .field
                                .update(cx, |field, cx| field.set_value(&label, cx));
                        }
                        v.apply_chord(false, cx);
                    },
                )
            }),
        ))
        .child(
            div()
                .flex()
                .gap_2()
                .child(
                    button(view, "apply-chord", "Aplicar", true, cx, |v, _, cx| {
                        v.apply_chord(false, cx)
                    })
                    .flex_1(),
                )
                .child(
                    button(view, "close-chord", "Cerrar", false, cx, |v, _, cx| {
                        v.chord_editor = None;
                        cx.notify();
                    })
                    .flex_1(),
                ),
        )
        .when(written, |panel| {
            panel.child(
                button(view, "remove-chord", "Quitar cifrado", false, cx, |v, _, cx| {
                    if let Some(editor) = v.chord_editor.take() {
                        v.edit(Command::SetChord(editor.target, None), cx);
                    }
                })
                .w_full(),
            )
        })
        .child(div().text_size(px(10.)).text_color(rgb(MUTED)).child(
            "Escribe C, F#m7, Bb7/D o Do, Fa#m7, Sib7/Re. Tab pasa a la nota siguiente; Enter aplica; Esc cierra.",
        ))
}
