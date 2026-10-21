use super::*;
use score_core::{InstrumentRange, ReviewTarget, ScoreError};
use text_input::TextInput;

pub struct RangeEditor {
    pub part: PartId,
    low: Entity<TextInput>,
    high: Entity<TextInput>,
}
impl RangeEditor {
    pub fn new(part: PartId, range: InstrumentRange, cx: &mut Context<AppView>) -> Self {
        let text = range.to_string();
        let (low, high) = text.split_once('–').unwrap();
        Self {
            part,
            low: cx.new(|cx| TextInput::new(low, cx)),
            high: cx.new(|cx| TextInput::new(high, cx)),
        }
    }

    pub fn focused(&self, window: &Window, cx: &App) -> bool {
        self.low.focus_handle(cx).is_focused(window)
            || self.high.focus_handle(cx).is_focused(window)
    }

    pub fn focus_next(&self, window: &mut Window, cx: &App) {
        if self.low.focus_handle(cx).is_focused(window) {
            self.high.focus_handle(cx).focus(window);
        } else {
            self.low.focus_handle(cx).focus(window);
        }
    }

    pub fn range(&self, cx: &App) -> Result<InstrumentRange, ScoreError> {
        parse_range(self.low.read(cx).value(), self.high.read(cx).value())
    }
}

fn parse_range(low: &str, high: &str) -> Result<InstrumentRange, ScoreError> {
    let range = InstrumentRange {
        lowest: parse_pitch(low)?,
        highest: parse_pitch(high)?,
    };
    range.validate()?;
    Ok(range)
}

/// Scientific C4 or Spanish Do4, with one sharp/flat/natural and optional spaces.
fn parse_pitch(value: &str) -> Result<u8, ScoreError> {
    let error = ScoreError("Escribe una altura como C4, Do4, F♯3 o Si♭5 (C0–G9)");
    let value: String = value
        .chars()
        .filter(|c| !c.is_whitespace())
        .flat_map(char::to_lowercase)
        .collect();
    let names = [
        ("sol", 7),
        ("do", 0),
        ("re", 2),
        ("mi", 4),
        ("fa", 5),
        ("la", 9),
        ("si", 11),
        ("c", 0),
        ("d", 2),
        ("e", 4),
        ("f", 5),
        ("g", 7),
        ("a", 9),
        ("b", 11),
    ];
    let (name, semitone) = names
        .into_iter()
        .find(|(name, _)| value.starts_with(name))
        .ok_or_else(|| error.clone())?;
    let mut rest = &value[name.len()..];
    let mut alteration = 0;
    if let Some(sign) = rest.chars().next() {
        match sign {
            '#' | '♯' => {
                alteration = 1;
                rest = &rest[sign.len_utf8()..];
            }
            'b' | '♭' => {
                alteration = -1;
                rest = &rest[sign.len_utf8()..];
            }
            '♮' => {
                rest = &rest[sign.len_utf8()..];
            }
            _ => {}
        }
    }
    if rest.len() != 1 || !rest.as_bytes()[0].is_ascii_digit() {
        return Err(error);
    }
    let octave = i16::from(rest.as_bytes()[0] - b'0');
    let midi = (octave + 1) * 12 + semitone + alteration;
    if !(12..=127).contains(&midi) {
        return Err(error);
    }
    Ok(midi as u8)
}

fn range_form(view: &AppView, cx: &Context<AppView>) -> Div {
    let editor = view.range_editor.as_ref().unwrap();
    let part = editor.part;
    section("EDITAR RANGO")
        .child(div().text_size(px(12.)).child(instrument_name(view.document.editor.score(), part)))
        .child(caption("ALTURA REAL · C4 = DO CENTRAL"))
        .child(div().flex().gap_2()
            .child(div().flex_1().min_w_0().child(caption("MÁS GRAVE")).child(editor.low.clone()))
            .child(div().flex_1().min_w_0().child(caption("MÁS AGUDA")).child(editor.high.clone())))
        .child(caption("Ejemplos: C4, Do4, F♯3, Si♭5. El rango personalizado incluye los sonidos armónicos."))
        .child(div().flex().gap_2()
            .child(button(view, "apply-instrument-range", "Aplicar", true, cx, |v, _, cx| v.apply_range(cx)).flex_1())
            .child(button(view, "cancel-instrument-range", "Cancelar", false, cx, |v, _, cx| { v.range_editor = None; cx.notify(); }).flex_1()))
        .child(button(view, "reset-edited-range", "Usar rango habitual", false, cx, move |v, _, cx| {
            v.edit(Command::SetInstrumentRange(part, None), cx);
            v.range_editor = None;
            cx.notify();
        }).w_full())
        .child(caption("Se guarda con esta parte y admite Deshacer. Tab cambia de campo; Enter aplica y Esc cancela."))
}

// Ribbon labels are kept on one line by inline_text. Review explanations need
// to wrap within the inspector while preserving Bravura accidentals.
pub(super) fn wrapped_text(view: &AppView, text: impl AsRef<str>) -> Div {
    div()
        .flex()
        .flex_wrap()
        .gap_x(px(3.))
        .gap_y(px(2.))
        .children(text.as_ref().split_whitespace().map(|word| {
            // Bound unbroken words (URLs and pasted identifiers) without
            // splitting an accent or an emoji sequence.
            use unicode_segmentation::UnicodeSegmentation;
            let glyphs: Vec<_> = word.graphemes(true).collect();
            div().flex().flex_wrap().min_w_0().children(
                glyphs
                    .chunks(16)
                    .map(|chunk| inline_text(view.music_font.clone(), chunk.concat())),
            )
        }))
}

pub(super) fn panel(view: &AppView, cx: &Context<AppView>) -> Div {
    let review = &view.review;
    let score = view.document.editor.score();
    let filtered = review.filtered();
    let count = filtered.len();
    let page_count = count.div_ceil(crate::review::PAGE_SIZE).max(1);
    let page = review.page.min(page_count - 1);
    let part = score.part(view.document.editor.cursor().part).unwrap();
    div().flex().flex_col()
        .when(view.comments.open, |body| body.child(super::comments::panel(view, cx)))
        .when(view.range_editor.is_some(), |body| body.child(range_form(view, cx)))
        .child(section("COMPROBAR")
            .child(command_button("review-check-score", "Toda la partitura", false, view, cx, id::CHECK_SCORE).w_full())
            .child(command_button("review-check-selection", "Selección actual", false, view, cx, id::CHECK_SELECTION).w_full())
            .child(caption("Comprueba rangos, repeticiones y destinos de saltos. Los avisos son orientativos.")))
        .child(section("AVISOS")
            .child(div().text_size(px(12.)).child(if review.checked() {
                format!("{count} de {} avisos · {}", review.issues.len(), if review.selection_scope { "selección" } else { "obra completa" })
            } else if review.stale { "La partitura cambió. Comprueba de nuevo la selección.".into() }
            else { "Todavía no se ha comprobado la partitura.".into() }))
            .child(button(view, "review-filter", format!("{} ▾", review.filter.map_or_else(|| "Todos los instrumentos".into(), |part| instrument_name(score, part))), review.filter_open, cx, |v, _, cx| {
                v.review.filter_open = !v.review.filter_open; cx.notify();
            }).w_full())
            .when(review.filter_open, |body| body.child(div().flex().flex_col().gap_1()
                .child(button(view, "review-filter-all", "Todos los instrumentos", review.filter.is_none(), cx, |v, _, cx| { v.review.set_filter(None); cx.notify(); }).w_full())
                .children(score.parts().iter().map(|part| {
                    let id = part.id();
                    button(view, format!("review-filter-{}", id.0), instrument_name(score, id), review.filter == Some(id), cx, move |v, _, cx| { v.review.set_filter(Some(id)); cx.notify(); }).w_full()
                }))))
            .child(div().flex().gap_2()
                .child(command_button("review-prev", "‹ Anterior", false, view, cx, id::REVIEW_PREVIOUS).flex_1())
                .child(command_button("review-next", "Siguiente ›", false, view, cx, id::REVIEW_NEXT).flex_1()))
            .when(review.checked() && count == 0, |body| body.child(caption("Sin avisos en este ámbito y filtro.")))
            .children(filtered.iter().skip(page * crate::review::PAGE_SIZE).take(crate::review::PAGE_SIZE).map(|&index| {
                let issue = review.issues[index];
                let location = match issue.target {
                    ReviewTarget::Note(id, pitch) => format!("{} · c. {} · {}", instrument_name(score, id.part), id.measure + 1, note_name(pitch.in_key(score.written_key_at(id.part, id.measure)))),
                    ReviewTarget::Bar(bar, _) => format!("Obra · compás {}", bar + 1),
                };
                action(format!("review-issue-{index}"), div().w_full().min_w_0().flex().flex_col().gap_1()
                    .child(div().text_size(px(11.)).text_color(rgb(BLUE)).child(wrapped_text(view, location)))
                    .child(div().text_size(px(11.)).child(wrapped_text(view, issue.kind.message()))), review.current == Some(index), cx, move |v, _, cx| v.goto_review_issue(index, cx))
                    .w_full().justify_start().px_2().py_2()
            }))
            .when(page_count > 1, |body| body.child(div().flex().flex_col().gap_1()
                .child(caption(format!("Página {} de {page_count}", page + 1)))
                .child(div().flex().gap_2()
                    .child(button(view, "review-list-prev", "‹ Lista", false, cx, move |v, _, cx| { v.review.page = page.saturating_sub(1); cx.notify(); }).flex_1())
                    .child(button(view, "review-list-next", "Lista ›", false, cx, move |v, _, cx| { v.review.page = (page + 1).min(page_count - 1); cx.notify(); }).flex_1()))))
            .when(review.filter.is_some(), |body| body.child(caption("Los avisos de repeticiones y saltos pertenecen a toda la obra."))))
        .child(section("RANGOS")
            .child(command_button("review-show-ranges", "Mostrar notas en rojo", review.show_ranges, view, cx, id::SHOW_RANGES).w_full())
            .child(div().text_size(px(12.)).child(instrument_name(score, part.id())))
            .child(div().text_size(px(11.)).child(wrapped_text(view, part.sounding_range().map_or_else(|| "Percusión sin altura: no se comprueba el registro.".into(), |range| format!("{}: {range} · altura real", if part.range_override().is_some() { "Personalizado" } else { "Habitual" })))))
            .when(part.sounding_range().is_some(), |body| body
                .child(command_button("review-edit-range", "Editar rango de esta parte…", view.range_editor.is_some(), view, cx, id::EDIT_RANGE).w_full())
                .when(part.range_override().is_some(), |body| body.child(command_button("review-reset-range", "Restablecer rango habitual", false, view, cx, id::RESET_RANGE).w_full())))
            .child(caption("Ocultar el rojo no desactiva las comprobaciones. Estos colores no se exportan.")))
}

#[cfg(test)]
mod tests {
    use super::{parse_pitch, parse_range};
    use score_core::InstrumentRange;
    #[test]
    fn musical_bounds_accept_spanish_and_scientific_names_without_ambiguous_numbers() {
        assert_eq!(
            parse_range(" Si♭ 3 ", "Do7").unwrap(),
            InstrumentRange {
                lowest: 58,
                highest: 96
            }
        );
        assert_eq!(parse_pitch("F#3").unwrap(), 54);
        assert_eq!(parse_pitch("Sol♮9").unwrap(), 127);
        assert_eq!(parse_range("C4", "C4").unwrap().lowest, 60);
        for text in [
            "", "60", "C", "C-1", "G#9", "C10", "D##4", "Do♭0", "C+4", "C4x",
        ] {
            assert!(parse_pitch(text).is_err(), "{text}");
        }
        assert!(parse_range("C5", "C4").is_err());
    }
}
