//! The agent panel: the conversation with the chosen agent, beside the
//! score. Plain text only, so a long conversation stays cheap to draw.
use super::*;
use crate::agent::Entry;
use agent_host::AgentKind;
use text_input::TextInput;

const WORKING: &str = "Trabajando… La partitura está bloqueada. Pulsa Detener para editar.";
const STOPPING: &str = "Deteniendo… Pulsa Detener otra vez para cerrar el agente.";
const EMPTY: &str = "Pide lo que harías en el editor, como se lo dirías a un copista: \
                     «pon un trino en el tiempo 3 del compás 4». Todo lo que haga el agente \
                     se deshace con un solo ⌘Z.";
const NONE_AVAILABLE: &str =
    "No se encontró ningún agente. Abre la lista junto al campo para ver qué falta.";
const SEARCHING: &str = "Buscando…";
const BUSY_PICKER: &str = "Detén al agente para cambiar de agente.";
const NEW_CONVERSATION: &str = "Empieza una conversación nueva";
const SWITCH_HEIGHT: f32 = 22.;
pub const MAX_MESSAGE_CHARS: usize = 2000;

/// The message field of the panel.
pub fn field(cx: &mut Context<AppView>) -> Entity<TextInput> {
    cx.new(|cx| {
        TextInput::new("", cx)
            .with_max_chars(MAX_MESSAGE_CHARS)
            .with_placeholder("Pide un cambio en la partitura…")
    })
}

impl AppView {
    pub(crate) fn agent_field_focused(&self, window: &Window, cx: &App) -> bool {
        self.agent_field.focus_handle(cx).is_focused(window)
    }

    /// Sends what the field holds and empties it; the text stays while the
    /// message cannot go out.
    pub(crate) fn send_agent_field(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.agent.can_send() || self.busy || self.confirming {
            return;
        }
        let text = self.agent_field.read(cx).value().trim().to_string();
        if text.is_empty() {
            return;
        }
        self.agent_field
            .update(cx, |field, cx| field.set_value("", cx));
        self.send_agent_message(text, window, cx);
    }
}

/// Why `kind` cannot start, once the probe ran.
fn unavailable(view: &AppView, kind: AgentKind) -> Option<String> {
    let tools = view.agent.tools.as_ref()?;
    agent_host::launch_command(kind, tools).err()
}

pub(super) fn panel(view: &AppView, cx: &Context<AppView>) -> Div {
    let turn = view.agent.turn_active();
    let stopping = view.agent.stopping();
    let entries = view.agent.log.entries();
    let none_available = view.agent.tools.is_some()
        && AgentKind::ALL
            .into_iter()
            .all(|kind| unavailable(view, kind).is_some());
    div()
        .w(px(AGENT_WIDTH))
        .flex_shrink_0()
        .bg(rgb(PANEL))
        .border_l_1()
        .border_color(rgb(BORDER))
        .flex()
        .flex_col()
        .child(
            div()
                .id("agent-log")
                .flex_1()
                .min_h_0()
                .overflow_y_scroll()
                .track_scroll(&view.agent.scroll)
                .px_3()
                .py_3()
                .flex()
                .flex_col()
                .gap_2()
                .text_size(px(12.))
                .when(entries.is_empty(), |log| {
                    log.child(div().text_color(rgb(MUTED)).child(if none_available {
                        NONE_AVAILABLE
                    } else {
                        EMPTY
                    }))
                })
                .children(
                    entries
                        .iter()
                        .enumerate()
                        .map(|(index, entry)| line(view, index, entry, cx)),
                ),
        )
        .when(turn || stopping, |panel| {
            panel.child(
                div()
                    .flex_shrink_0()
                    .px_3()
                    .py_2()
                    .border_t_1()
                    .border_color(rgb(BORDER))
                    .text_size(px(11.))
                    .text_color(rgb(WARNING))
                    .child(if stopping { STOPPING } else { WORKING }),
            )
        })
        .child(composer(view, cx))
}

/// The chosen agent, quietly at the foot of the field; it opens the list of
/// agents above itself. It stays shut while the agent works.
fn agent_switch(view: &AppView, cx: &Context<AppView>) -> Div {
    let turn = view.agent.turn_active() || view.agent.stopping();
    let open = view.ui.agent_menu && !turn;
    let trigger = div()
        .id("agent-switch")
        .h(px(SWITCH_HEIGHT))
        .px_1()
        .flex()
        .items_center()
        .gap_1()
        .rounded_md()
        .text_size(px(11.))
        .text_color(rgb(if turn { MUTED } else { SECONDARY }))
        .child(view.agent.kind.label())
        .child(div().text_color(rgb(MUTED)).child("▾"))
        .when(open, |trigger| trigger.bg(rgb(HOVER)).text_color(rgb(TEXT)))
        .map(|trigger| {
            if turn {
                trigger.tooltip(hover_hint(BUSY_PICKER, None))
            } else {
                trigger
                    .cursor_pointer()
                    .hover(|style| style.bg(rgb(HOVER)).text_color(rgb(TEXT)))
                    .on_click(cx.listener(move |v, _, _, cx| {
                        v.ui.agent_menu = !open;
                        cx.notify();
                    }))
            }
        });
    div().child(trigger).when(open, |switch| {
        switch.child(deferred(
            // The field sits at the foot of the window: open upwards.
            anchored()
                .anchor(Corner::BottomLeft)
                .offset(point(px(0.), px(-(SWITCH_HEIGHT + 4.))))
                .snap_to_window()
                .child(
                    agent_menu(view, cx).on_mouse_down_out(cx.listener(|v, _, _, cx| {
                        v.ui.agent_menu = false;
                        cx.notify();
                    })),
                ),
        ))
    })
}

/// One row per agent: the chosen one ticked, and under each name what
/// choosing it means, or what is missing for it.
fn agent_menu(view: &AppView, cx: &Context<AppView>) -> Div {
    let talking = !view.agent.log.entries().is_empty();
    toolbar::popover(div().w(px(232.)).flex().flex_col().gap(px(1.)).children(
        AgentKind::ALL.into_iter().map(|kind| {
            let chosen = view.agent.kind == kind;
            let missing = unavailable(view, kind);
            let note = match &missing {
                Some(missing) => Some(missing.clone()),
                None if view.agent.tools.is_none() => Some(SEARCHING.into()),
                None if talking && !chosen => Some(NEW_CONVERSATION.into()),
                None => None,
            };
            let row = div()
                .id(SharedString::from(format!("agent-{}", kind.label())))
                .px_2()
                .py(px(5.))
                .flex()
                .flex_col()
                .gap(px(2.))
                .rounded_md()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .text_size(px(12.))
                        .text_color(rgb(if missing.is_some() { MUTED } else { TEXT }))
                        .child(kind.label())
                        .when(chosen, |name| {
                            name.child(div().text_color(rgb(BLUE)).child("✓"))
                        }),
                )
                .children(
                    note.map(|note| div().text_size(px(10.5)).text_color(rgb(MUTED)).child(note)),
                );
            if missing.is_some() {
                row
            } else {
                row.cursor_pointer()
                    .hover(|style| style.bg(rgb(HOVER)))
                    .on_click(cx.listener(move |v, _, _, cx| {
                        v.ui.agent_menu = false;
                        v.select_agent(kind, cx);
                        cx.notify();
                    }))
            }
        }),
    ))
}

fn line(view: &AppView, index: usize, entry: &Entry, cx: &Context<AppView>) -> AnyElement {
    match entry {
        Entry::User(text) => div()
            .px_2()
            .py_1()
            .rounded_md()
            .border_1()
            .border_color(rgb(OUTLINE))
            .bg(rgb(CARD))
            .child(text.clone())
            .into_any_element(),
        Entry::Agent(text) => div()
            .text_color(rgb(SECONDARY))
            .child(text.clone())
            .into_any_element(),
        Entry::Tool { title, done, .. } => div()
            .flex()
            .gap_1()
            .text_size(px(11.))
            .text_color(rgb(MUTED))
            .child(if *done { "✓" } else { "…" })
            .child(div().min_w_0().child(if title.is_empty() {
                "Herramienta".to_string()
            } else {
                title.clone()
            }))
            .into_any_element(),
        Entry::Permission { title, options, .. } => {
            permission(view, index, title, options, cx).into_any_element()
        }
        Entry::Notice(text) => div()
            .py_1()
            .text_size(px(11.))
            .text_color(rgb(MUTED))
            .text_center()
            .child(format!("— {text} —"))
            .into_any_element(),
        Entry::Error(text) => div()
            .px_2()
            .py_1()
            .rounded_md()
            .bg(rgb(ERROR_BG))
            .text_color(rgb(ERROR))
            .child(text.clone())
            .into_any_element(),
    }
}

/// The agent asks before going on: one button per option and «Rechazar»;
/// once answered, the answer instead.
fn permission(
    view: &AppView,
    index: usize,
    title: &str,
    options: &[(String, String)],
    cx: &Context<AppView>,
) -> Div {
    let card = div()
        .p_2()
        .flex()
        .flex_col()
        .gap_2()
        .rounded_md()
        .border_1()
        .border_color(rgb(SELECT_BORDER))
        .bg(rgb(CARD))
        .child(caption("PERMISO"))
        .child(if title.is_empty() {
            "El agente pide permiso para seguir.".to_string()
        } else {
            title.to_string()
        });
    if let Some(answer) = view.agent.answer(index) {
        return card.child(
            div()
                .text_size(px(11.))
                .text_color(rgb(MUTED))
                .child(format!("Respuesta: {answer}")),
        );
    }
    card.child(
        div()
            .flex()
            .flex_wrap()
            .gap_1()
            .children(options.iter().map(|(option, label)| {
                let option = option.clone();
                button(
                    view,
                    format!("agent-permission-{index}-{option}"),
                    label.clone(),
                    false,
                    cx,
                    move |v, _, cx| v.answer_agent_permission(index, Some(option.clone()), cx),
                )
                .text_size(px(11.))
            }))
            .child(
                button(
                    view,
                    format!("agent-permission-{index}-refuse"),
                    "Rechazar",
                    false,
                    cx,
                    move |v, _, cx| v.answer_agent_permission(index, None, cx),
                )
                .text_size(px(11.)),
            ),
    )
}

/// The message field and Enviar, which is Detener during the turn and
/// until a stopped agent ends it.
fn composer(view: &AppView, cx: &Context<AppView>) -> Div {
    let turn = view.agent.turn_active() || view.agent.stopping();
    let send = if turn {
        button(view, "agent-stop", "Detener", true, cx, |v, _, cx| {
            v.stop_agent(cx)
        })
    } else {
        button(view, "agent-send", "Enviar", false, cx, |v, window, cx| {
            v.send_agent_field(window, cx)
        })
        .when(!view.agent.can_send(), |send| send.opacity(0.45))
    };
    div()
        .flex_shrink_0()
        .px_3()
        .py_3()
        .flex()
        .flex_col()
        .gap_1()
        .border_t_1()
        .border_color(rgb(BORDER))
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(div().flex_1().min_w_0().child(view.agent_field.clone()))
                .child(send.h(px(32.))),
        )
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .gap_2()
                .child(agent_switch(view, cx))
                .child(
                    div()
                        .text_size(px(10.))
                        .text_color(rgb(MUTED))
                        .child("Enter envía · Esc vuelve"),
                ),
        )
}
