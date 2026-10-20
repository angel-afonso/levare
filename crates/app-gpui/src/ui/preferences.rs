//! Preferences › Atajos: every registered command by category, with its keys
//! and controls to record, add, remove or restore them.

use super::*;
use score_keymap::{Category, KeySequence, Status};

pub struct Preferences {
    pub category: Category,
    pub recording: Option<Recording>,
    pub conflict: Option<PreferencesConflict>,
    pub message: Option<String>,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            category: Category::NoteInput,
            recording: None,
            conflict: None,
            message: None,
        }
    }
}

/// Waiting for the next combination for `command`; `add` keeps its other keys.
pub struct Recording {
    pub command: CommandId,
    pub add: bool,
}

/// The recorded keys already belong to `other`: the user decides.
pub struct PreferencesConflict {
    pub command: CommandId,
    pub keys: KeySequence,
    pub other: CommandId,
    pub add: bool,
}

fn badge(text: &'static str, color: u32) -> Div {
    div()
        .text_size(px(9.))
        .px_1()
        .rounded_sm()
        .border_1()
        .border_color(rgb(color))
        .text_color(rgb(color))
        .child(text)
}

fn start_recording(view: &mut AppView, command: CommandId, add: bool, cx: &mut Context<AppView>) {
    let label = view
        .shortcuts
        .spec(command)
        .map_or(command.as_str(), |spec| spec.label);
    if let Some(preferences) = &mut view.preferences {
        preferences.conflict = None;
        preferences.recording = Some(Recording { command, add });
        preferences.message = Some(format!("Pulsa la combinación para «{label}». Esc cancela."));
    }
    cx.notify();
}

pub fn overlay(view: &AppView, cx: &Context<AppView>) -> impl IntoElement {
    let preferences = view.preferences.as_ref().expect("preferences open");
    let shortcuts = &view.shortcuts;
    div()
        .id("preferences-overlay")
        .absolute()
        .top_0()
        .left_0()
        .size_full()
        .flex()
        .items_center()
        .justify_center()
        .bg(rgba(0x08090ccc))
        .occlude()
        .child(
            div()
                .w(px(920.))
                .max_w(relative(0.95))
                .h(px(660.))
                .max_h(relative(0.92))
                .flex()
                .flex_col()
                .bg(rgb(PANEL))
                .border_1()
                .border_color(rgb(0x3a3d46))
                .rounded_lg()
                .shadow_lg()
                .overflow_hidden()
                .child(header(view, cx))
                .child(
                    div()
                        .flex()
                        .flex_1()
                        .min_h_0()
                        .child(categories(view, cx))
                        .child(commands(view, preferences.category, cx)),
                )
                .child(
                    div()
                        .px_4()
                        .py_2()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .border_t_1()
                        .border_color(rgb(BORDER))
                        .text_size(px(10.))
                        .text_color(rgb(MUTED))
                        .when_some(preferences.message.clone(), |footer, message| {
                            footer.child(div().text_color(rgb(0xb8ceff)).child(message))
                        })
                        .children(
                            shortcuts.problems.iter().map(|problem| {
                                div().text_color(rgb(0xf2b282)).child(problem.clone())
                            }),
                        )
                        .child(match &shortcuts.path {
                            Some(path) => format!("Tus cambios: {}", path.display()),
                            None => "No hay carpeta de datos: los cambios duran esta sesión".into(),
                        }),
                ),
        )
}

fn header(view: &AppView, cx: &Context<AppView>) -> Div {
    let profile = view.shortcuts.keymap.profile();
    div()
        .px_4()
        .py_3()
        .flex()
        .items_center()
        .gap_3()
        .border_b_1()
        .border_color(rgb(BORDER))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .gap_1()
                .child(
                    div()
                        .text_size(px(15.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child("Atajos de teclado"),
                )
                .child(
                    div()
                        .text_size(px(11.))
                        .text_color(rgb(MUTED))
                        .child(format!("Perfil {} · {}", profile.name, profile.description)),
                ),
        )
        .child(
            button(
                view,
                "preferences-reset-all",
                "Restablecer todo",
                false,
                cx,
                |v, _, cx| {
                    v.shortcuts.user.reset_all();
                    v.save_shortcuts("Atajos de Sibelius restablecidos".into(), cx);
                },
            )
            .px_3(),
        )
        .child(command_button(
            "preferences-close",
            "Cerrar",
            false,
            view,
            cx,
            id::CLOSE_PANEL,
        ))
}

fn categories(view: &AppView, cx: &Context<AppView>) -> Div {
    let current = view.preferences.as_ref().map(|p| p.category);
    div()
        .w(px(200.))
        .flex_shrink_0()
        .p_2()
        .flex()
        .flex_col()
        .gap_1()
        .border_r_1()
        .border_color(rgb(BORDER))
        .children(Category::ALL.into_iter().map(|category| {
            let count = view.shortcuts.registry.in_category(category).count();
            button(
                view,
                format!("preferences-category-{category:?}"),
                format!("{}  ·  {count}", category.label()),
                current == Some(category),
                cx,
                move |v, _, cx| {
                    if let Some(preferences) = &mut v.preferences {
                        preferences.category = category;
                        preferences.recording = None;
                        preferences.conflict = None;
                    }
                    cx.notify();
                },
            )
            .w_full()
            .justify_start()
        }))
}

fn commands(view: &AppView, category: Category, cx: &Context<AppView>) -> Stateful<Div> {
    let preferences = view.preferences.as_ref().expect("preferences open");
    div()
        .id("preferences-commands")
        .flex_1()
        .min_w_0()
        .overflow_y_scroll()
        .p_3()
        .flex()
        .flex_col()
        .gap_1()
        .children(view.shortcuts.registry.in_category(category).map(|spec| {
            let command = spec.id;
            let keys = view.shortcuts.keymap.keys_for(command);
            let recording = preferences
                .recording
                .as_ref()
                .is_some_and(|recording| recording.command == command);
            let overridden = view.shortcuts.user.is_overridden(command);
            let conflict = preferences
                .conflict
                .as_ref()
                .filter(|conflict| conflict.command == command);
            div()
                .flex()
                .flex_col()
                .gap_2()
                .px_3()
                .py_2()
                .rounded_md()
                .bg(rgb(if recording { SELECT } else { FIELD }))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_3()
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .flex()
                                .items_center()
                                .gap_2()
                                .child(div().text_size(px(12.)).truncate().child(spec.label))
                                .when(spec.status == Status::Pending, |row| {
                                    row.child(badge("Pendiente", 0xa3a7b2))
                                })
                                .when(overridden, |row| row.child(badge("Modificado", BLUE))),
                        )
                        .child(
                            div()
                                .flex()
                                .gap_1()
                                .when(recording, |chips| {
                                    chips.child(
                                        div()
                                            .text_size(px(11.))
                                            .text_color(rgb(0xb8ceff))
                                            .child("Pulsa una combinación…"),
                                    )
                                })
                                .when(!recording && keys.is_empty(), |chips| {
                                    chips.child(
                                        div()
                                            .text_size(px(11.))
                                            .text_color(rgb(MUTED))
                                            .child("Sin atajo"),
                                    )
                                })
                                .when(!recording, |chips| {
                                    chips.children(keys.iter().map(|keys| {
                                        shortcut(keys.label())
                                            .text_size(px(11.))
                                            .text_color(rgb(TEXT))
                                    }))
                                }),
                        )
                        .child(
                            button(
                                view,
                                format!("preferences-change-{command}"),
                                "Cambiar",
                                recording,
                                cx,
                                move |v, _, cx| start_recording(v, command, false, cx),
                            )
                            .text_size(px(11.)),
                        )
                        .child(
                            button(
                                view,
                                format!("preferences-add-{command}"),
                                "+",
                                false,
                                cx,
                                move |v, _, cx| start_recording(v, command, true, cx),
                            )
                            .text_size(px(11.)),
                        )
                        .when(!keys.is_empty(), |row| {
                            row.child(
                                button(
                                    view,
                                    format!("preferences-remove-{command}"),
                                    "Quitar",
                                    false,
                                    cx,
                                    move |v, _, cx| {
                                        v.shortcuts.user.set(command, &[]);
                                        v.save_shortcuts("Atajo quitado".into(), cx);
                                    },
                                )
                                .text_size(px(11.)),
                            )
                        })
                        .when(overridden, |row| {
                            row.child(
                                button(
                                    view,
                                    format!("preferences-reset-{command}"),
                                    "Restablecer",
                                    false,
                                    cx,
                                    move |v, _, cx| {
                                        v.shortcuts.user.reset(command);
                                        v.save_shortcuts(
                                            "Atajo de Sibelius restablecido".into(),
                                            cx,
                                        );
                                    },
                                )
                                .text_size(px(11.)),
                            )
                        }),
                )
                .when_some(conflict, |row, conflict| {
                    let other = view
                        .shortcuts
                        .spec(conflict.other)
                        .map_or(conflict.other.as_str(), |spec| spec.label);
                    row.child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .text_size(px(11.))
                            .text_color(rgb(0xf2b282))
                            .child(div().flex_1().child(format!(
                                "{} ya se usa para «{other}».",
                                conflict.keys.label()
                            )))
                            .child(button(
                                view,
                                format!("preferences-reassign-{command}"),
                                "Reasignar",
                                true,
                                cx,
                                |v, _, cx| {
                                    if let Some(conflict) =
                                        v.preferences.as_mut().and_then(|p| p.conflict.take())
                                    {
                                        v.assign(
                                            conflict.command,
                                            conflict.keys,
                                            conflict.add,
                                            Some(conflict.other),
                                            cx,
                                        );
                                    }
                                },
                            ))
                            .child(button(
                                view,
                                format!("preferences-keep-{command}"),
                                "Cancelar",
                                false,
                                cx,
                                |v, _, cx| {
                                    if let Some(preferences) = &mut v.preferences {
                                        preferences.conflict = None;
                                        preferences.message = Some("Sin cambios".into());
                                    }
                                    cx.notify();
                                },
                            )),
                    )
                })
        }))
}
