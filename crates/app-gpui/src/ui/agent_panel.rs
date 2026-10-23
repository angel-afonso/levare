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
    "No se encontró ningún agente. Pasa el cursor por cada botón para ver qué falta.";
const SEARCHING: &str = "Buscando los agentes instalados…";
const BUSY_PICKER: &str = "Detén al agente para cambiar de agente.";
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
        .child(picker(view, cx))
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

/// The three agents; one that is not installed is shown off, with what is
/// missing on hover.
fn picker(view: &AppView, cx: &Context<AppView>) -> Div {
    let turn = view.agent.turn_active();
    let talking = !view.agent.log.entries().is_empty();
    div()
        .min_h(px(45.))
        .flex_shrink_0()
        .flex()
        .flex_wrap()
        .items_center()
        .gap_1()
        .px_2()
        .border_b_1()
        .border_color(rgb(BORDER))
        .children(AgentKind::ALL.into_iter().map(|kind| {
            let chosen = view.agent.kind == kind;
            let missing = unavailable(view, kind);
            let id = format!("agent-{}", kind.label());
            let hint = if let Some(missing) = &missing {
                missing.clone()
            } else if view.agent.tools.is_none() {
                SEARCHING.into()
            } else if chosen {
                format!("Conversas con {}", kind.label())
            } else if turn {
                BUSY_PICKER.into()
            } else if talking {
                format!("Cambiar a {} empieza una conversación nueva.", kind.label())
            } else {
                format!("Usar {}", kind.label())
            };
            let enabled = missing.is_none() && (chosen || !turn);
            let element = if enabled {
                button(view, id, kind.label(), chosen, cx, move |v, _, cx| {
                    v.select_agent(kind, cx)
                })
            } else {
                div()
                    .id(SharedString::from(id))
                    .flex()
                    .items_center()
                    .justify_center()
                    .flex_shrink_0()
                    .px_2()
                    .h(px(29.))
                    .rounded_md()
                    .border_1()
                    .border_color(transparent_black())
                    .bg(rgb(FIELD))
                    .text_color(rgb(MUTED))
                    .opacity(0.55)
                    .child(kind.label())
            };
            // The chosen agent keeps the selected look of `button`.
            element
                .text_size(px(11.))
                .when(!chosen, |element| element.bg(rgb(PANEL)))
                .tooltip(hover_hint(hint, None))
        }))
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
                .text_size(px(10.))
                .text_color(rgb(MUTED))
                .child("Enter envía · Esc vuelve a la partitura"),
        )
}
