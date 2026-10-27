use super::*;
use score_core::Credits;
use text_input::TextInput;

/// Title and credits being edited; nothing reaches the score until Aplicar.
pub struct HeaderEditor {
    fields: [(&'static str, Entity<TextInput>); 7],
}
impl HeaderEditor {
    pub fn new(score: &score_core::Score, cx: &mut Context<AppView>) -> Self {
        let credits = score.credits();
        let mut field = |label, value: &str| (label, cx.new(|cx| TextInput::new(value, cx)));
        Self {
            fields: [
                field("Título", score.title()),
                field("Subtítulo", &credits.subtitle),
                field("Compositor", &credits.composer),
                field("Arreglista", &credits.arranger),
                field("Letrista", &credits.lyricist),
                field("Año", &credits.year),
                field("Copyright", &credits.copyright),
            ],
        }
    }
    fn value(&self, index: usize, cx: &App) -> String {
        self.fields[index].1.read(cx).value().to_owned()
    }
    pub fn header(&self, cx: &App) -> (String, Credits) {
        (
            self.value(0, cx),
            Credits {
                subtitle: self.value(1, cx),
                composer: self.value(2, cx),
                arranger: self.value(3, cx),
                lyricist: self.value(4, cx),
                year: self.value(5, cx),
                copyright: self.value(6, cx),
            },
        )
    }
    fn focused_index(&self, window: &Window, cx: &App) -> Option<usize> {
        self.fields
            .iter()
            .position(|(_, field)| field.focus_handle(cx).is_focused(window))
    }
    pub fn focused(&self, window: &Window, cx: &App) -> bool {
        self.focused_index(window, cx).is_some()
    }
    pub fn focus_first(&self, window: &mut Window, cx: &App) {
        self.fields[0].1.focus_handle(cx).focus(window);
    }
    /// Tab and Shift-Tab walk the fields in order, wrapping around.
    pub fn focus_next(&self, back: bool, window: &mut Window, cx: &App) {
        let count = self.fields.len();
        let next = match self.focused_index(window, cx) {
            Some(index) if back => (index + count - 1) % count,
            Some(index) => (index + 1) % count,
            None => 0,
        };
        self.fields[next].1.focus_handle(cx).focus(window);
    }
}

pub(super) fn panel(view: &AppView, cx: &Context<AppView>) -> Div {
    let editor = view.header_editor.as_ref().unwrap();
    div()
        .flex()
        .flex_col()
        .gap_2()
        .children(editor.fields.iter().map(|(label, field)| {
            div()
                .flex()
                .flex_col()
                .gap_1()
                .child(caption(*label))
                .child(field.clone())
        }))
        .child(
            div()
                .flex()
                .gap(px(6.))
                .mt_2()
                .child(
                    choice(view, "apply-header", "Aplicar", true, cx, |v, _, cx| {
                        v.apply_header(cx)
                    })
                    .tooltip(described_hint(
                        "Aplicar",
                        Some("Enter".into()),
                        "Se imprime en la cabecera de la primera página; los campos vacíos no aparecen. Tab pasa al siguiente campo, Enter aplica y Esc cancela.",
                    )),
                )
                .child(close_editor("Cancelar", cx)),
        )
}
