//! Samples in the shapes the adapters send, from their sources:
//! claude-agent-acp 0.94.0 (`src/tool-calls/renderer.ts` `toolCall` and
//! `permissionToolCall`, `src/acp-agent.ts` `_meta.claudeCode`,
//! `src/permissions/options/shared.ts`), codex-acp 2.2.2
//! (`src/tool-calls/reporters/McpToolReporter.ts`, `GuardianReporter.ts`,
//! `ElicitationReporter.ts`, `src/permissions/mcp.ts`) and Gemini CLI
//! (`packages/cli/src/acp/acpSession.ts`, `acpUtils.ts`, and the title of
//! `DiscoveredMCPToolInvocation` in the core bundle).
use agent_client_protocol::schema::v1::{RequestPermissionRequest, ToolCall, ToolCallUpdate};
use serde_json::{Value, json};

use crate::calls::{Calls, Described, auto_answer};

fn permission(tool_call: Value, options: Value) -> RequestPermissionRequest {
    serde_json::from_value(json!({
        "sessionId": "s",
        "toolCall": tool_call,
        "options": options,
    }))
    .unwrap()
}

fn report(calls: &mut Calls, call: Value) -> Option<String> {
    let call: ToolCall = serde_json::from_value(call).unwrap();
    calls.report(&call.tool_call_id.to_string(), Described::call(&call))
}

fn claude_options() -> Value {
    json!([
        {"optionId": "allow-with-updates", "name": "Yes, and don't ask again for levare - resumen", "kind": "allow_always"},
        {"optionId": "allow-once", "name": "Yes", "kind": "allow_once"},
        {"optionId": "reject", "name": "No", "kind": "reject_once"},
    ])
}

#[test]
fn claude_code_levare_tool_is_allowed_once() {
    let calls = Calls::default();
    let request = permission(
        json!({
            "toolCallId": "toolu_01",
            "name": "mcp__levare__resumen",
            "status": "pending",
            "rawInput": {},
            "title": "mcp__levare__resumen",
            "kind": "other",
            "content": [],
            "_meta": {"claudeCode": {
                "toolName": "mcp__levare__resumen",
                "mcpServer": {"name": "levare", "source": "dynamic"},
            }},
        }),
        claude_options(),
    );
    assert_eq!(auto_answer(&calls, &request).as_deref(), Some("allow-once"));

    // Only the server in `_meta` says it.
    let request = permission(
        json!({
            "toolCallId": "toolu_02",
            "status": "pending",
            "title": "Usar herramienta",
            "_meta": {"claudeCode": {"mcpServer": {"name": "levare", "source": "dynamic"}}},
        }),
        claude_options(),
    );
    assert_eq!(auto_answer(&calls, &request).as_deref(), Some("allow-once"));
}

#[test]
fn claude_code_other_tools_ask_the_user() {
    let calls = Calls::default();
    for (title, tool) in [
        ("ls -la", "Bash"),
        ("Write /tmp/notas.txt", "Write"),
        ("mcp__github__leer", "mcp__github__leer"),
        ("mcp__levare__borrar", "mcp__levare__borrar"),
    ] {
        let request = permission(
            json!({
                "toolCallId": "toolu_03",
                "name": tool,
                "status": "pending",
                "rawInput": {"command": "ls -la"},
                "title": title,
                "kind": "execute",
                "_meta": {"claudeCode": {"toolName": tool}},
            }),
            claude_options(),
        );
        assert_eq!(auto_answer(&calls, &request), None, "{title}");
    }
}

#[test]
fn codex_approval_of_a_reported_levare_call_is_allowed() {
    let mut calls = Calls::default();
    let shown = report(
        &mut calls,
        json!({
            "toolCallId": "exec-9ade0a68",
            "title": "mcp.levare.ejecutar",
            "kind": "execute",
            "status": "pending",
            "rawInput": {"server": "levare", "tool": "ejecutar", "arguments": {"comando": "expresion.staccato"}},
        }),
    );
    assert_eq!(shown.as_deref(), Some("mcp.levare.ejecutar"));
    let options = json!([
        {"optionId": "allow_once", "name": "Allow", "kind": "allow_once"},
        {"optionId": "allow_session", "name": "Allow for this session", "kind": "allow_always"},
        {"optionId": "cancel", "name": "Cancel", "kind": "reject_once"},
    ]);
    // The approval names the call by its id only.
    let correlated = permission(
        json!({"toolCallId": "exec-9ade0a68", "kind": "execute", "status": "pending"}),
        options.clone(),
    );
    assert_eq!(
        auto_answer(&calls, &correlated).as_deref(),
        Some("allow_once")
    );
    let unknown = permission(
        json!({"toolCallId": "exec-otra", "kind": "execute", "status": "pending"}),
        options.clone(),
    );
    assert_eq!(auto_answer(&calls, &unknown), None);
    // One it could not match to a call carries the server in its input.
    let standalone = |server: &str| {
        permission(
            json!({
                "toolCallId": format!("elicitation:s:{server}:1"),
                "kind": "execute",
                "status": "pending",
                "title": "MCP tool call approval",
                "rawInput": {"serverName": server, "description": "Allow levare to run tool \"leer\"?", "schema": {}},
            }),
            options.clone(),
        )
    };
    assert_eq!(
        auto_answer(&calls, &standalone("levare")).as_deref(),
        Some("allow_once")
    );
    assert_eq!(auto_answer(&calls, &standalone("github")), None);
}

#[test]
fn codex_review_of_a_levare_call_is_not_shown() {
    let mut calls = Calls::default();
    let review = |status: &str| {
        json!({
            "toolCallId": "guardian_assessment:0f0a6dbe",
            "title": "Guardian Review",
            "kind": "think",
            "status": status,
            "rawInput": {
                "threadId": "s",
                "turnId": "t",
                "reviewId": "0f0a6dbe",
                "action": {"type": "mcpToolCall", "server": "levare", "toolName": "resumen", "connectorId": null, "connectorName": null, "toolTitle": null},
            },
        })
    };
    assert_eq!(report(&mut calls, review("in_progress")), None);
    // The update that closes it carries no input.
    let update: ToolCallUpdate = serde_json::from_value(json!({
        "toolCallId": "guardian_assessment:0f0a6dbe",
        "status": "completed",
    }))
    .unwrap();
    assert_eq!(
        calls.report("guardian_assessment:0f0a6dbe", Described::update(&update)),
        None
    );
    // The review of a shell command is the user's business.
    let shell = json!({
        "toolCallId": "guardian_assessment:77",
        "title": "Guardian Review",
        "kind": "think",
        "status": "in_progress",
        "rawInput": {"action": {"type": "command", "command": "rm -rf build", "cwd": "/tmp"}},
    });
    assert_eq!(
        report(&mut calls, shell).as_deref(),
        Some("Guardian Review")
    );
}

#[test]
fn gemini_levare_tool_is_allowed_once() {
    let mut calls = Calls::default();
    let options = json!([
        {"optionId": "proceed_always_server", "name": "Allow all server tools for this session", "kind": "allow_always"},
        {"optionId": "proceed_always_tool", "name": "Allow tool for this session", "kind": "allow_always"},
        {"optionId": "proceed_once", "name": "Allow", "kind": "allow_once"},
        {"optionId": "cancel", "name": "Reject", "kind": "reject_once"},
    ]);
    for title in [
        "leer(desde: 1, hasta: 3)",
        "resumen (levare MCP Server)",
        "ejecutar(comando: expresion.staccato)",
    ] {
        let call = json!({
            "toolCallId": "leer-1761",
            "status": "pending",
            "title": title,
            "content": [],
            "locations": [],
            "kind": "other",
        });
        report(&mut calls, call.clone());
        let request = permission(call, options.clone());
        assert_eq!(
            auto_answer(&calls, &request).as_deref(),
            Some("proceed_once"),
            "{title}"
        );
    }
    let shell = permission(
        json!({
            "toolCallId": "run_shell_command-1",
            "status": "pending",
            "title": "rm -rf build",
            "kind": "execute",
        }),
        options,
    );
    assert_eq!(auto_answer(&calls, &shell), None);
}

#[test]
fn levare_tool_without_an_allow_option_asks_the_user() {
    let calls = Calls::default();
    let request = permission(
        json!({"toolCallId": "t", "title": "mcp.levare.leer"}),
        json!([{"optionId": "no", "name": "No", "kind": "reject_once"}]),
    );
    assert_eq!(auto_answer(&calls, &request), None);
    let always = permission(
        json!({"toolCallId": "t", "title": "mcp.levare.leer"}),
        json!([
            {"optionId": "siempre", "name": "Siempre", "kind": "allow_always"},
            {"optionId": "no", "name": "No", "kind": "reject_once"},
        ]),
    );
    assert_eq!(auto_answer(&calls, &always).as_deref(), Some("siempre"));
}
