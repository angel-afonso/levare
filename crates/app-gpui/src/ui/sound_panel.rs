//! The Sound tab: the general output, the sound of the chosen part and the
//! packs on demand. Instruments are added and removed from the left panel.
use super::toolbar::{self, action_button, control};
use super::*;

pub(super) fn panel(view: &AppView, cx: &Context<AppView>) -> Div {
    div()
        .flex()
        .flex_col()
        .child(mixer::panel(view, cx))
        .child(part_sound(view, cx))
        .children(more_sounds(view, cx))
}

/// A full-width choice in a list.
fn choice(
    view: &AppView,
    id: impl Into<SharedString>,
    label: impl Into<SharedString>,
    chosen: bool,
    cx: &Context<AppView>,
    click: impl Fn(&mut AppView, &mut Window, &mut Context<AppView>) + 'static,
) -> Stateful<Div> {
    control(
        id,
        inline_text(view.music_font.clone(), label.into().as_ref()),
        chosen,
        cx,
        click,
    )
    .w_full()
    .h(px(26.))
    .px_2()
    .justify_start()
    .text_size(px(12.))
}

fn interpretation(view: &AppView, cx: &Context<AppView>) -> Div {
    use score_playback::Style;
    let option = |id: &'static str, label: &'static str, style: Style, status: &'static str| {
        control(
            id,
            label,
            view.playback.style == style,
            cx,
            move |v, _, cx| {
                if v.playback.style != style {
                    v.playback.stop();
                    v.playback.style = style;
                    v.status = status.into();
                    cx.notify();
                }
            },
        )
        .h(px(26.))
        .text_size(px(12.))
    };
    div()
        .flex()
        .flex_col()
        .gap_2()
        .child(group_title("Interpretación", None))
        .child(toolbar::track([
            option(
                "interpret-natural",
                "Natural",
                Style::Natural,
                "Interpretación: natural",
            ),
            option(
                "interpret-literal",
                "Literal (exacta)",
                Style::Literal,
                "Interpretación: literal",
            ),
        ]))
}

/// The order in which combined SFZ libraries are tried.
fn priority(view: &AppView, cx: &Context<AppView>) -> Div {
    let step = |id: String, glyph: &'static str, up: bool, index: usize| {
        control(id, glyph, false, cx, move |v, _, cx| {
            match v.playback.move_sound_priority(index, up) {
                Ok(bank) => v.choose_sound(Some(bank), cx),
                Err(error) => {
                    v.playback.sound_error = Some(error.clone());
                    v.status = error;
                    cx.notify();
                }
            }
        })
        .w(px(24.))
        .h(px(22.))
    };
    let count = view.playback.sound_priority.len();
    div()
        .flex()
        .flex_col()
        .gap_1()
        .child(group_title("Prioridad", None))
        .children(
            view.playback
                .sound_priority
                .iter()
                .enumerate()
                .map(|(index, id)| {
                    div()
                        .flex()
                        .items_center()
                        .gap_1()
                        .child(
                            div()
                                .flex_1()
                                .text_size(px(12.))
                                .child(view.playback.sound_priority_name(id)),
                        )
                        .when(index > 0, |row| {
                            row.child(step(format!("priority-up-{index}"), "↑", true, index))
                        })
                        .when(index + 1 < count, |row| {
                            row.child(step(format!("priority-down-{index}"), "↓", false, index))
                        })
                }),
        )
}

fn sources(view: &AppView, cx: &Context<AppView>) -> Div {
    let playback = &view.playback;
    let combined = playback
        .selected_bank
        .and_then(|index| playback.banks.get(index))
        .is_some_and(|bank| bank.id() == "sfz-combinadas");
    div()
        .flex()
        .flex_col()
        .gap_1()
        .child(group_title(
            "Fuente de sonido",
            (!playback.banks.is_empty()).then(|| {
                "La primera carga puede tardar unos segundos. Puedes volver a GeneralUser cuando quieras."
                    .into()
            }),
        ))
        .child(choice(
            view,
            "sound-generaluser",
            "GeneralUser GS",
            playback.selected_bank.is_none(),
            cx,
            |v, _, cx| v.choose_sound(None, cx),
        ))
        .children(playback.banks.iter().enumerate().map(|(index, bank)| {
            choice(
                view,
                format!("sound-{}", bank.id()),
                bank.name().to_owned(),
                playback.selected_bank == Some(index),
                cx,
                move |v, _, cx| v.choose_sound(Some(index), cx),
            )
        }))
        .when(combined, |body| body.child(priority(view, cx)))
        .child(
            div()
                .mt_1()
                .text_size(px(10.))
                .text_color(rgb(MUTED))
                .child(format!(
                    "Instala bibliotecas SFZ en {}. Ver docs/sfz.md.",
                    crate::data_path("soundpacks").map_or_else(
                        || "la carpeta de datos".into(),
                        |path| path.display().to_string()
                    )
                )),
        )
        .children(playback.sound_discovery_errors.iter().map(|error| {
            div()
                .text_size(px(10.))
                .text_color(rgb(WARNING))
                .child(error.clone())
        }))
        .when_some(playback.sound_error.clone(), |body, error| {
            body.child(div().text_size(px(11.)).text_color(rgb(WARNING)).child(error))
        })
}

/// The chosen part's instrument and the General MIDI program and sound it
/// plays with now.
fn current(view: &AppView) -> Div {
    let editor = &view.document.editor;
    let instrument = editor
        .score()
        .part(editor.cursor().part)
        .unwrap()
        .instrument();
    let selected = editor.selection();
    let harmonic = selected
        .and_then(|id| editor.score().event(id))
        .and_then(|event| event.notation.harmonic);
    let technique = selected
        .and_then(|id| editor.score().technique_at(id))
        .unwrap_or_default();
    let line = match harmonic {
        Some(harmonic) => format!(
            "{} · {}\nSíntesis de armónicos",
            harmonic.label(),
            technique.label()
        ),
        None => format!(
            "General MIDI {}\n{}",
            technique.program(instrument) + 1,
            view.playback.sound_name()
        ),
    };
    let strings = instrument.family() == score_core::InstrumentFamily::Strings;
    div()
        .flex()
        .flex_col()
        .gap_1()
        .child(div().text_size(px(14.)).child(instrument.name()))
        .child(
            div()
                .id("current-sound")
                .text_size(px(11.))
                .text_color(rgb(MUTED))
                .child(line)
                .when(strings, |line| {
                    line.tooltip(described_hint(
                        "Armónicos",
                        None,
                        "Armónicos con timbre sintetizado: sostenidos al arco y resonantes en pizzicato.",
                    ))
                }),
        )
        .when_some(view.playback.sound_description(), |body, description| {
            body.child(
                div()
                    .text_size(px(10.))
                    .text_color(rgb(MUTED))
                    .child(description),
            )
        })
}

fn part_sound(view: &AppView, cx: &Context<AppView>) -> Div {
    panel_section()
        .child(group_title("Sonido de la parte", None))
        .child(current(view))
        .child(interpretation(view, cx))
        .child(sources(view, cx))
        .when_some(view.playback.error.clone(), |body, error| {
            body.child(
                div()
                    .text_size(px(11.))
                    .text_color(rgb(WARNING))
                    .child(error),
            )
            .child(
                action_button(
                    "reconnect-audio",
                    "Reintentar audio",
                    false,
                    cx,
                    |v, _, cx| {
                        v.playback.connect();
                        cx.notify();
                    },
                )
                .h(px(28.))
                .px_2(),
            )
        })
}

/// The packs available on demand, when Levare is built with SFZ support.
fn more_sounds(view: &AppView, cx: &Context<AppView>) -> Option<Div> {
    #[cfg(feature = "vsco")]
    {
        Some(panel_section().child(sound_store::panel(view, cx)))
    }
    #[cfg(not(feature = "vsco"))]
    {
        let _ = (view, cx);
        None
    }
}
