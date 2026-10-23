use std::collections::HashMap;
use std::path::PathBuf;
use std::pin::pin;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use agent_client_protocol::schema::ProtocolVersion;
use agent_client_protocol::schema::v1::{
    CancelNotification, ContentBlock, HttpHeader, Implementation, InitializeRequest,
    McpServer as AcpMcpServer, McpServerHttp, NewSessionRequest, PromptRequest,
    RequestPermissionOutcome, RequestPermissionRequest, RequestPermissionResponse,
    SelectedPermissionOutcome, SessionNotification, SessionUpdate, ToolCallStatus,
};
use agent_client_protocol::{
    self as acp, AcpAgent, AcpAgentConfig, Agent, Client, ConnectTo, ConnectionTo, Responder,
};
use futures::StreamExt;
use futures::channel::mpsc::{self, UnboundedReceiver, UnboundedSender};
use futures::channel::oneshot;
use tokio::task::AbortHandle;

use crate::agents::{LaunchCommand, NO_LEVARE_TOOLS};
use crate::calls::{self, Calls, Described};
use crate::{McpServer, Runtime};

pub(crate) const BUSY: &str = "Espera a que termine la petición actual.";
const CLOSED: &str = "El agente se cerró.";
pub(crate) const NO_START: &str = "El agente no respondió al arrancar en 2 minutos. \
                                   Vuelve a enviar el mensaje para intentarlo otra vez.";
/// How long the agent may take to answer `initialize` and `session/new`:
/// the first run downloads the adapter through npx, which can be slow.
const START_TIMEOUT: Duration = Duration::from_secs(120);

/// What the agent's session reports, in order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AgentEvent {
    /// A piece of the agent's answer, to append to the previous ones.
    Message(String),
    /// A tool call started (`done: false`) or finished.
    Tool {
        id: String,
        title: String,
        done: bool,
    },
    /// The agent asks before going on; answer with
    /// [`SessionHandle::answer_permission`]. `options` are `(id, name)`.
    Permission {
        request: u64,
        title: String,
        options: Vec<(String, String)>,
    },
    /// The prompt in flight finished, normally or cancelled.
    TurnEnded,
    /// The raw text of a failure, for [`crate::explain_error`]. It ends the
    /// prompt in flight, if any, as `TurnEnded` would; the session is over
    /// when the event stream ends after it.
    Failed(String),
}

enum Command {
    Prompt(String),
    Cancel,
    Answer {
        request: u64,
        option_id: Option<String>,
    },
}

/// Talks to the session task without waiting on it. Dropping it ends the
/// session and the agent's process.
pub struct SessionHandle {
    commands: UnboundedSender<Command>,
    task: AbortHandle,
}

impl SessionHandle {
    /// Sends a prompt. It ends with `TurnEnded` or `Failed`; while it runs,
    /// another prompt fails at once.
    pub fn prompt(&self, text: String) {
        self.send(Command::Prompt(text));
    }

    /// Asks the agent to stop the prompt in flight; it still ends with
    /// `TurnEnded`. Pending permission requests are answered as cancelled.
    pub fn cancel(&self) {
        self.send(Command::Cancel);
    }

    /// Answers a `Permission` event with one of its option ids, or `None`
    /// to cancel.
    pub fn answer_permission(&self, request: u64, option_id: Option<String>) {
        self.send(Command::Answer { request, option_id });
    }

    fn send(&self, command: Command) {
        let _ = self.commands.unbounded_send(command);
    }
}

impl Drop for SessionHandle {
    /// Dropping the session task drops the connection, which kills the
    /// agent's process group.
    fn drop(&mut self) {
        self.task.abort();
    }
}

/// Where the agent reaches Levare's tools.
pub(crate) struct McpEndpoint {
    pub url: String,
    pub token: String,
}

/// Launches the agent and opens a session in `cwd` with Levare's MCP
/// server. Returns at once; starting up happens on the runtime, and its
/// failures arrive as `Failed`.
pub fn start_session(
    rt: &Runtime,
    launch: LaunchCommand,
    cwd: PathBuf,
    mcp: &McpServer,
) -> (SessionHandle, UnboundedReceiver<AgentEvent>) {
    let agent = AcpAgent::new(
        AcpAgentConfig::new(launch.program)
            .args(launch.args)
            .envs(launch.env),
    );
    let endpoint = McpEndpoint {
        url: mcp.url.clone(),
        token: mcp.token.clone(),
    };
    spawn_session(rt, agent, cwd, endpoint, START_TIMEOUT)
}

/// [`start_session`] over any transport to an agent.
pub(crate) fn spawn_session(
    rt: &Runtime,
    agent: impl ConnectTo<Client>,
    cwd: PathBuf,
    mcp: McpEndpoint,
    start_timeout: Duration,
) -> (SessionHandle, UnboundedReceiver<AgentEvent>) {
    let (commands, received) = mpsc::unbounded();
    let (events, stream) = mpsc::unbounded();
    let task = rt.handle().spawn(async move {
        let started = run(agent, cwd, mcp, start_timeout, received, events.clone());
        let failure = match started.await {
            Ok(Ended::Done) => return,
            Ok(Ended::Closed) => CLOSED.to_string(),
            Err(error) => describe(&error),
        };
        let _ = events.unbounded_send(AgentEvent::Failed(failure));
    });
    let handle = SessionHandle {
        commands,
        task: task.abort_handle(),
    };
    (handle, stream)
}

/// The message with its JSON-RPC code, which tells `auth_required` apart.
fn describe(error: &acp::Error) -> String {
    format!("{error} ({})", i32::from(error.code))
}

/// Permission requests waiting for the user, by the id given to the UI.
#[derive(Default)]
struct Permissions {
    next: u64,
    pending: HashMap<u64, Responder<RequestPermissionResponse>>,
}

type SharedPermissions = Arc<Mutex<Permissions>>;

fn answer(
    permissions: &SharedPermissions,
    request: u64,
    option_id: Option<String>,
) -> Result<(), acp::Error> {
    let responder = permissions.lock().unwrap().pending.remove(&request);
    let Some(responder) = responder else {
        return Ok(());
    };
    let outcome = match option_id {
        Some(id) => RequestPermissionOutcome::Selected(SelectedPermissionOutcome::new(id)),
        None => RequestPermissionOutcome::Cancelled,
    };
    responder.respond(RequestPermissionResponse::new(outcome))
}

async fn run(
    agent: impl ConnectTo<Client>,
    cwd: PathBuf,
    mcp: McpEndpoint,
    start_timeout: Duration,
    commands: UnboundedReceiver<Command>,
    events: UnboundedSender<AgentEvent>,
) -> Result<Ended, acp::Error> {
    let permissions = SharedPermissions::default();
    // Updates and permission requests may leave out what a call was opened with.
    let calls = Arc::new(Mutex::new(Calls::default()));
    let (error, transport_error) = oneshot::channel();
    let transport = Watched {
        transport: agent,
        error,
    };
    Client
        .builder()
        .on_receive_notification(
            {
                let events = events.clone();
                let calls = calls.clone();
                async move |notification: SessionNotification, _cx: ConnectionTo<Agent>| {
                    let event = event_for(notification.update, &mut calls.lock().unwrap());
                    if let Some(event) = event {
                        let _ = events.unbounded_send(event);
                    }
                    Ok(())
                }
            },
            acp::on_receive_notification!(),
        )
        .on_receive_request(
            {
                let events = events.clone();
                let permissions = permissions.clone();
                let calls = calls.clone();
                // Kept for the user to answer; returning frees the dispatch loop.
                async move |request: RequestPermissionRequest,
                            responder: Responder<RequestPermissionResponse>,
                            _cx: ConnectionTo<Agent>| {
                    let (allowed, title) = {
                        let calls = calls.lock().unwrap();
                        let call = &request.tool_call;
                        let title = call.fields.title.clone().or_else(|| {
                            calls
                                .title(&call.tool_call_id.to_string())
                                .map(String::from)
                        });
                        (calls::auto_answer(&calls, &request), title)
                    };
                    // The user asked for the edits: Levare's own tools run
                    // without a card.
                    if let Some(option) = allowed {
                        let outcome = RequestPermissionOutcome::Selected(
                            SelectedPermissionOutcome::new(option),
                        );
                        return responder.respond(RequestPermissionResponse::new(outcome));
                    }
                    let id = {
                        let mut permissions = permissions.lock().unwrap();
                        let id = permissions.next;
                        permissions.next += 1;
                        permissions.pending.insert(id, responder);
                        id
                    };
                    let options = request
                        .options
                        .into_iter()
                        .map(|option| (option.option_id.to_string(), option.name))
                        .collect();
                    let _ = events.unbounded_send(AgentEvent::Permission {
                        request: id,
                        title: title.unwrap_or_default(),
                        options,
                    });
                    Ok(())
                }
            },
            acp::on_receive_request!(),
        )
        .connect_with(transport, async move |cx: ConnectionTo<Agent>| {
            let ended = drive(
                &cx,
                cwd,
                mcp,
                start_timeout,
                commands,
                &events,
                &permissions,
            )
            .await;
            let closed = match &ended {
                Ok(Ended::Done) => false,
                Ok(Ended::Closed) => true,
                Err(_) => cx.is_incoming_closed(),
            };
            if !closed {
                return ended;
            }
            // Report why the process ended (its exit status and stderr)
            // rather than the request it cut short. A process that exited
            // cleanly reports nothing.
            match tokio::time::timeout(EXIT_REPORT_WAIT, transport_error).await {
                Ok(Ok(error)) => Err(error),
                _ => Ok(Ended::Closed),
            }
        })
        .await
}

/// How long a closed transport may take to report the process's exit.
const EXIT_REPORT_WAIT: Duration = Duration::from_secs(2);

/// A transport that also hands over the error it ends with.
struct Watched<T> {
    transport: T,
    error: oneshot::Sender<acp::Error>,
}

impl<T: ConnectTo<Client>> ConnectTo<Client> for Watched<T> {
    async fn connect_to(self, client: impl ConnectTo<Agent>) -> Result<(), acp::Error> {
        let result = self.transport.connect_to(client).await;
        if let Err(error) = &result {
            let _ = self.error.send(error.clone());
        }
        result
    }
}

enum Ended {
    /// The handle was dropped, or the failure was already reported.
    Done,
    /// The agent closed its end without an error of its own.
    Closed,
}

/// Opens the session, then runs one prompt at a time until the handle is
/// dropped or the agent goes away.
async fn drive(
    cx: &ConnectionTo<Agent>,
    cwd: PathBuf,
    mcp: McpEndpoint,
    start_timeout: Duration,
    mut commands: UnboundedReceiver<Command>,
    events: &UnboundedSender<AgentEvent>,
    permissions: &SharedPermissions,
) -> Result<Ended, acp::Error> {
    // A start that never answers fails, and dropping the connection ends
    // the process.
    let no_start = || {
        let _ = events.unbounded_send(AgentEvent::Failed(NO_START.into()));
        Ok(Ended::Done)
    };
    let initialize = InitializeRequest::new(ProtocolVersion::V1)
        .client_info(Implementation::new("levare", env!("CARGO_PKG_VERSION")));
    let initialized =
        tokio::time::timeout(start_timeout, cx.send_request(initialize).block_task()).await;
    let Ok(agent) = initialized else {
        return no_start();
    };
    if !agent?.agent_capabilities.mcp_capabilities.http {
        let _ = events.unbounded_send(AgentEvent::Failed(NO_LEVARE_TOOLS.into()));
        return Ok(Ended::Done);
    }
    let levare = McpServerHttp::new(calls::SERVER, mcp.url).headers(vec![HttpHeader::new(
        "Authorization",
        format!("Bearer {}", mcp.token),
    )]);
    let new_session = NewSessionRequest::new(cwd).mcp_servers(vec![AcpMcpServer::Http(levare)]);
    let opened =
        tokio::time::timeout(start_timeout, cx.send_request(new_session).block_task()).await;
    let Ok(opened) = opened else {
        return no_start();
    };
    let session = opened?.session_id;

    let mut turn = None;
    let mut closed = pin!(cx.incoming_closed());
    loop {
        tokio::select! {
            command = commands.next() => match command {
                None => return Ok(Ended::Done),
                Some(Command::Prompt(_)) if turn.is_some() => {
                    let _ = events.unbounded_send(AgentEvent::Failed(BUSY.into()));
                }
                Some(Command::Prompt(text)) => {
                    let prompt = PromptRequest::new(session.clone(), vec![ContentBlock::from(text)]);
                    turn = Some(Box::pin(cx.send_request(prompt).block_task()));
                }
                Some(Command::Cancel) => {
                    if turn.is_some() {
                        let pending: Vec<u64> =
                            permissions.lock().unwrap().pending.keys().copied().collect();
                        for request in pending {
                            answer(permissions, request, None)?;
                        }
                        cx.send_notification(CancelNotification::new(session.clone()))?;
                    }
                }
                Some(Command::Answer { request, option_id }) => {
                    answer(permissions, request, option_id)?;
                }
            },
            result = async { turn.as_mut().expect("guarded by the condition").await }, if turn.is_some() => {
                turn = None;
                // Requests the agent left unanswered no longer matter.
                permissions.lock().unwrap().pending.clear();
                match result {
                    Ok(_) => {
                        let _ = events.unbounded_send(AgentEvent::TurnEnded);
                    }
                    // The process is gone: one failure ends the session.
                    Err(error) if cx.is_incoming_closed() => return Err(error),
                    Err(error) => {
                        let _ = events.unbounded_send(AgentEvent::Failed(describe(&error)));
                    }
                }
            }
            () = &mut closed => return Ok(Ended::Closed),
        }
    }
}

/// The event for a session update; thoughts, reviews of Levare's calls
/// and the rest are left out.
fn event_for(update: SessionUpdate, calls: &mut Calls) -> Option<AgentEvent> {
    let done = |status| matches!(status, ToolCallStatus::Completed | ToolCallStatus::Failed);
    match update {
        SessionUpdate::AgentMessageChunk(chunk) => match chunk.content {
            ContentBlock::Text(text) => Some(AgentEvent::Message(text.text)),
            _ => None,
        },
        SessionUpdate::ToolCall(call) => {
            let id = call.tool_call_id.to_string();
            let title = calls.report(&id, Described::call(&call))?;
            Some(AgentEvent::Tool {
                id,
                title,
                done: done(call.status),
            })
        }
        SessionUpdate::ToolCallUpdate(update) => {
            let id = update.tool_call_id.to_string();
            let title = calls.report(&id, Described::update(&update))?;
            Some(AgentEvent::Tool {
                id,
                title,
                done: update.fields.status.is_some_and(done),
            })
        }
        _ => None,
    }
}
