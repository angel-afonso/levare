use super::*;
use score_core::{TempoBeat, TempoMark};
use text_input::TextInput;

pub struct TempoEditor {
    pub measure: usize,
    pub beat: TempoBeat,
    pub text: Entity<TextInput>,
    pub bpm: Entity<TextInput>,
}
impl TempoEditor {
    pub fn new(mark: &TempoMark, measure: usize, cx: &mut Context<AppView>) -> Self {
        Self {
            measure,
            beat: mark.beat,
            text: cx.new(|cx| TextInput::new(&mark.text, cx)),
            bpm: cx.new(|cx| TextInput::new(&mark.bpm.to_string(), cx)),
        }
    }
    pub fn focused(&self, window: &Window, cx: &App) -> bool {
        self.text.focus_handle(cx).is_focused(window)
            || self.bpm.focus_handle(cx).is_focused(window)
    }
    pub fn mark(&self, cx: &App) -> Result<TempoMark, score_core::ScoreError> {
        let bpm = self.bpm.read(cx).value().trim().parse().map_err(|_| {
            score_core::ScoreError("Escribe un tempo de 20 a 400 pulsos por minuto")
        })?;
        let mark = TempoMark {
            bpm,
            beat: self.beat,
            text: self.text.read(cx).value().trim().to_owned(),
        };
        mark.validate()?;
        Ok(mark)
    }
}

pub(super) fn panel(view: &AppView, cx: &Context<AppView>) -> Div {
    let editor = view.tempo_picker.as_ref().unwrap();
    div().flex().flex_col().gap_2()
        .child(signature_scope(view, cx))
        .child(caption("Indicación"))
        .child(editor.text.clone())
        .child(caption("Pulsos por minuto · 20–400"))
        .child(editor.bpm.clone())
        .child(caption("Figura del pulso"))
        .children(TempoBeat::ALL.into_iter().map(|beat| choice(view, format!("tempo-beat-{beat:?}"), format!("{}  {}", beat.glyph(), beat.label()), editor.beat == beat, cx, move |v, _, cx| {
            if let Some(editor) = &mut v.tempo_picker { editor.beat = beat; } cx.notify();
        }).w_full().justify_start()))
        .child(caption("Indicaciones habituales").mt_2())
        .child(div().flex().flex_wrap().gap_1().children([
            ("Largo", 48), ("Adagio", 66), ("Andante", 84), ("Moderato", 108),
            ("Allegretto", 116), ("Allegro", 132), ("Vivace", 160), ("Presto", 184),
        ].into_iter().map(|(name, bpm)| choice(view, format!("tempo-{name}"), name, editor.text.read(cx).value() == name, cx, move |v, _, cx| {
            if let Some(editor) = &v.tempo_picker {
                editor.text.update(cx, |field, cx| field.set_value(name, cx));
                editor.bpm.update(cx, |field, cx| field.set_value(&bpm.to_string(), cx));
            } cx.notify();
        }))))
        .child(div().flex().flex_wrap().gap(px(6.)).mt_2()
            .child(act(view, "apply-tempo", "Aplicar tempo", true, cx, |v, _, cx| v.apply_tempo(cx))
                .tooltip(described_hint("Aplicar tempo", Some("Enter".into()), "Se escribe sobre el pentagrama y rige hasta el siguiente cambio, en todos los instrumentos. Enter aplica; Esc cancela.")))
            .child(close_editor("Cancelar", cx)))
        .when(view.signature_measure() > 0 && view.document.editor.score().tempo_change_at(view.signature_measure()).is_some(), |panel| {
            panel.child(act(view, "remove-tempo-change", "Quitar cambio de tempo", false, cx, |v, _, cx| {
                v.edit(Command::RemoveTempoChange(v.signature_measure()), cx);
                v.tempo_picker = None;
            }))
        })
}
