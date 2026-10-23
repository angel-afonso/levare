use std::path::PathBuf;
use std::time::Duration;

use agent_client_protocol::schema::v1::{
    AgentCapabilities, CancelNotification, ContentBlock, ContentChunk, InitializeRequest,
    InitializeResponse, McpCapabilities, McpServer as AcpMcpServer, NewSessionRequest,
    NewSessionResponse, PermissionOption, PermissionOptionKind, PromptRequest, PromptResponse,
    RequestPermissionOutcome, RequestPermissionRequest, SessionNotification, SessionUpdate,
    StopReason, ToolCall, ToolCallStatus, ToolCallUpdate, ToolCallUpdateFields,
};
use agent_client_protocol::{self as acp, Agent, Client, ConnectTo, ConnectionTo, Responder};
use futures::StreamExt;
use futures::channel::mpsc;

use super::*;
use crate::session::{BUSY, McpEndpoint, spawn_session};

const SESSION: &str = "sesion-1";

fn update(cx: &ConnectionTo<Client>, update: SessionUpdate) -> Result<(), acp::Error> {
    cx.send_notification(SessionNotification::new(SESSION, update))
}

fn text(text: &str) -> ContentChunk {
    ContentChunk::new(ContentBlock::from(text))
}

/// An in-process ACP agent. A prompt `hola` answers with a thought, a
/// message and a tool call; `permiso` asks for permission first and
/// reports what was chosen. Every `session/new` is sent to `sessions`.
fn fake_agent(
    http: bool,
    sessions: mpsc::UnboundedSender<NewSessionRequest>,
) -> impl ConnectTo<Client> {
    Agent
        .builder()
        .on_receive_request(
            async move |_request: InitializeRequest,
                        responder: Responder<InitializeResponse>,
                        _cx: ConnectionTo<Client>| {
                responder.respond(
                    InitializeResponse::new(acp::schema::ProtocolVersion::V1).agent_capabilities(
                        AgentCapabilities::new()
                            .mcp_capabilities(McpCapabilities::new().http(http)),
                    ),
                )
            },
            acp::on_receive_request!(),
        )
        .on_receive_request(
            async move |request: NewSessionRequest,
                        responder: Responder<NewSessionResponse>,
                        _cx: ConnectionTo<Client>| {
                let _ = sessions.unbounded_send(request);
                responder.respond(NewSessionResponse::new(SESSION))
            },
            acp::on_receive_request!(),
        )
        .on_receive_request(
            async move |request: PromptRequest,
                        responder: Responder<PromptResponse>,
                        cx: ConnectionTo<Client>| {
                let ContentBlock::Text(prompt) = &request.prompt[0] else {
                    unreachable!()
                };
                if prompt.text == "permiso" {
                    // Waiting for the answer must not hold the dispatch loop.
                    cx.spawn({
                        let cx = cx.clone();
                        async move {
                            let tool = ToolCallUpdate::new(
                                "t2",
                                ToolCallUpdateFields::new().title("Escribir compases".to_string()),
                            );
                            let options = vec![
                                PermissionOption::new(
                                    "si",
                                    "Permitir",
                                    PermissionOptionKind::AllowOnce,
                                ),
                                PermissionOption::new(
                                    "no",
                                    "Rechazar",
                                    PermissionOptionKind::RejectOnce,
                                ),
                            ];
                            let answer = cx
                                .send_request(RequestPermissionRequest::new(SESSION, tool, options))
                                .block_task()
                                .await?;
                            let stop = match answer.outcome {
                                RequestPermissionOutcome::Selected(selected) => {
                                    update(
                                        &cx,
                                        SessionUpdate::AgentMessageChunk(text(&format!(
                                            "elegiste {}",
                                            selected.option_id
                                        ))),
                                    )?;
                                    StopReason::EndTurn
                                }
                                _ => StopReason::Cancelled,
                            };
                            responder.respond(PromptResponse::new(stop))
                        }
                    })?;
                    return Ok(());
                }
                update(&cx, SessionUpdate::AgentThoughtChunk(text("pensando")))?;
                update(&cx, SessionUpdate::AgentMessageChunk(text("Hola, ")))?;
                update(&cx, SessionUpdate::AgentMessageChunk(text("músico.")))?;
                update(
                    &cx,
                    SessionUpdate::ToolCall(
                        ToolCall::new("t1", "Leer la partitura").status(ToolCallStatus::InProgress),
                    ),
                )?;
                update(
                    &cx,
                    SessionUpdate::ToolCallUpdate(ToolCallUpdate::new(
                        "t1",
                        ToolCallUpdateFields::new().status(ToolCallStatus::Completed),
                    )),
                )?;
                responder.respond(PromptResponse::new(StopReason::EndTurn))
            },
            acp::on_receive_request!(),
        )
        .on_receive_notification(
            async move |_cancel: CancelNotification, _cx: ConnectionTo<Client>| Ok(()),
            acp::on_receive_notification!(),
        )
}

fn endpoint() -> McpEndpoint {
    McpEndpoint {
        url: "http://127.0.0.1:9/mcp".into(),
        token: "secreto".into(),
    }
}

fn next(rt: &Runtime, events: &mut mpsc::UnboundedReceiver<AgentEvent>) -> Option<AgentEvent> {
    rt.handle()
        .block_on(async { tokio::time::timeout(Duration::from_secs(10), events.next()).await })
        .expect("no event within 10 s")
}

fn cwd() -> PathBuf {
    std::env::temp_dir()
}

#[test]
fn prompt_streams_messages_and_tools_then_ends_the_turn() {
    let rt = Runtime::new().unwrap();
    let (sessions, mut opened) = mpsc::unbounded();
    let (session, mut events) = spawn_session(&rt, fake_agent(true, sessions), cwd(), endpoint());

    session.prompt("hola".into());

    assert_eq!(
        next(&rt, &mut events),
        Some(AgentEvent::Message("Hola, ".into()))
    );
    assert_eq!(
        next(&rt, &mut events),
        Some(AgentEvent::Message("músico.".into()))
    );
    let tool = |done| AgentEvent::Tool {
        id: "t1".into(),
        title: "Leer la partitura".into(),
        done,
    };
    assert_eq!(next(&rt, &mut events), Some(tool(false)));
    // The update carries no title: the call keeps the one it was opened with.
    assert_eq!(next(&rt, &mut events), Some(tool(true)));
    assert_eq!(next(&rt, &mut events), Some(AgentEvent::TurnEnded));

    let request = rt.handle().block_on(opened.next()).unwrap();
    assert_eq!(request.cwd, cwd());
    let [AcpMcpServer::Http(server)] = request.mcp_servers.as_slice() else {
        panic!("{:?}", request.mcp_servers)
    };
    assert_eq!(server.url, "http://127.0.0.1:9/mcp");
    assert_eq!(server.headers[0].name, "Authorization");
    assert_eq!(server.headers[0].value, "Bearer secreto");
}

#[test]
fn permission_round_trip() {
    let rt = Runtime::new().unwrap();
    let (sessions, _opened) = mpsc::unbounded();
    let (session, mut events) = spawn_session(&rt, fake_agent(true, sessions), cwd(), endpoint());

    session.prompt("permiso".into());
    let Some(AgentEvent::Permission {
        request,
        title,
        options,
    }) = next(&rt, &mut events)
    else {
        panic!("expected a permission request")
    };
    assert_eq!(title, "Escribir compases");
    assert_eq!(
        options,
        [
            ("si".to_string(), "Permitir".to_string()),
            ("no".to_string(), "Rechazar".to_string())
        ]
    );

    // A second prompt while the first runs is refused, not queued.
    session.prompt("otra".into());
    assert_eq!(
        next(&rt, &mut events),
        Some(AgentEvent::Failed(BUSY.into()))
    );

    session.answer_permission(request, Some("si".into()));
    assert_eq!(
        next(&rt, &mut events),
        Some(AgentEvent::Message("elegiste si".into()))
    );
    assert_eq!(next(&rt, &mut events), Some(AgentEvent::TurnEnded));
}

#[test]
fn cancel_answers_pending_permission_and_ends_the_turn() {
    let rt = Runtime::new().unwrap();
    let (sessions, _opened) = mpsc::unbounded();
    let (session, mut events) = spawn_session(&rt, fake_agent(true, sessions), cwd(), endpoint());

    session.prompt("permiso".into());
    assert!(matches!(
        next(&rt, &mut events),
        Some(AgentEvent::Permission { .. })
    ));
    session.cancel();
    assert_eq!(next(&rt, &mut events), Some(AgentEvent::TurnEnded));

    // The session goes on.
    session.prompt("hola".into());
    assert_eq!(
        next(&rt, &mut events),
        Some(AgentEvent::Message("Hola, ".into()))
    );
}

#[test]
fn agent_without_http_mcp_fails() {
    let rt = Runtime::new().unwrap();
    let (sessions, _opened) = mpsc::unbounded();
    let (_session, mut events) = spawn_session(&rt, fake_agent(false, sessions), cwd(), endpoint());

    let Some(AgentEvent::Failed(raw)) = next(&rt, &mut events) else {
        panic!("expected a failure")
    };
    assert_eq!(
        explain_error(AgentKind::Claude, &raw),
        "Este agente no admite las herramientas de Levare."
    );
    assert_eq!(next(&rt, &mut events), None);
}

#[test]
fn process_exit_fails_with_its_stderr() {
    let rt = Runtime::new().unwrap();
    let (tools, _invocations) = mpsc::unbounded();
    let mcp = rt.handle().block_on(McpServer::start(tools)).unwrap();
    let launch = LaunchCommand {
        program: "/bin/sh".into(),
        args: vec![
            "-c".into(),
            "echo 'npm error code ENOTFOUND' >&2; exit 1".into(),
        ],
        env: vec![("PATH".into(), "/usr/bin:/bin".into())],
    };

    let (_session, mut events) = start_session(&rt, launch, cwd(), &mcp);

    let Some(AgentEvent::Failed(raw)) = next(&rt, &mut events) else {
        panic!("expected a failure")
    };
    assert!(raw.contains("ENOTFOUND"), "{raw}");
    assert_eq!(
        explain_error(AgentKind::Codex, &raw),
        "Sin conexión para descargar el adaptador."
    );
    assert_eq!(next(&rt, &mut events), None);
}

/// A shell agent that opens the session, then exits with `status` when the
/// prompt arrives.
fn exits_on_prompt(status: u8) -> LaunchCommand {
    let reply = |result: &str| {
        format!(
            r#"read line; id=$(printf '%s' "$line" | sed 's/.*"id":\([0-9]*\).*/\1/'); printf '{{"jsonrpc":"2.0","id":%s,"result":{result}}}\n' "$id""#
        )
    };
    let script = [
        reply(r#"{"protocolVersion":1,"agentCapabilities":{"mcpCapabilities":{"http":true}}}"#),
        reply(r#"{"sessionId":"s"}"#),
        format!("read line; echo 'se acabó' >&2; exit {status}"),
    ]
    .join("\n");
    LaunchCommand {
        program: "/bin/sh".into(),
        args: vec!["-c".into(), script],
        env: vec![("PATH".into(), "/usr/bin:/bin".into())],
    }
}

#[test]
fn process_exit_during_a_turn_fails_once() {
    let rt = Runtime::new().unwrap();
    let (tools, _invocations) = mpsc::unbounded();
    let mcp = rt.handle().block_on(McpServer::start(tools)).unwrap();

    let (session, mut events) = start_session(&rt, exits_on_prompt(3), cwd(), &mcp);
    session.prompt("hola".into());

    let Some(AgentEvent::Failed(raw)) = next(&rt, &mut events) else {
        panic!("expected a failure")
    };
    assert!(raw.contains("se acabó"), "{raw}");
    assert_eq!(next(&rt, &mut events), None);
}

#[test]
fn clean_process_exit_also_fails() {
    let rt = Runtime::new().unwrap();
    let (tools, _invocations) = mpsc::unbounded();
    let mcp = rt.handle().block_on(McpServer::start(tools)).unwrap();

    let (session, mut events) = start_session(&rt, exits_on_prompt(0), cwd(), &mcp);
    session.prompt("hola".into());

    assert_eq!(
        next(&rt, &mut events),
        Some(AgentEvent::Failed("El agente se cerró.".into()))
    );
    assert_eq!(next(&rt, &mut events), None);
}

#[test]
fn dropping_the_handle_ends_the_session_and_the_process() {
    let rt = Runtime::new().unwrap();
    let (tools, _invocations) = mpsc::unbounded();
    let mcp = rt.handle().block_on(McpServer::start(tools)).unwrap();
    // Never answers `initialize`; the odd duration tells it apart.
    let launch = LaunchCommand {
        program: "/bin/sleep".into(),
        args: vec!["31.4159".into()],
        env: vec![],
    };
    let running = || {
        std::process::Command::new("pgrep")
            .args(["-f", "sleep 31.4159"])
            .output()
            .unwrap()
            .status
            .success()
    };

    let (session, mut events) = start_session(&rt, launch, cwd(), &mcp);
    let started = std::time::Instant::now();
    while !running() {
        assert!(started.elapsed() < Duration::from_secs(5), "never started");
        std::thread::sleep(Duration::from_millis(20));
    }
    drop(session);

    assert_eq!(next(&rt, &mut events), None);
    let dropped = std::time::Instant::now();
    while running() {
        assert!(dropped.elapsed() < Duration::from_secs(5), "still running");
        std::thread::sleep(Duration::from_millis(20));
    }
}
