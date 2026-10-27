//! Which of the agent's tool calls run Levare's own tools, told apart in the
//! shapes each adapter reports them in. The user asked for the edits, so
//! those calls need no permission card; when in doubt, the card shows.
use std::collections::{HashMap, HashSet};

use agent_client_protocol::schema::v1::{
    PermissionOption, PermissionOptionKind, RequestPermissionRequest, ToolCall, ToolCallUpdate,
};
use serde_json::{Map, Value};

/// The name Levare's MCP server goes by in the session.
pub(crate) const SERVER: &str = "levare";

/// What a tool call says about itself, in a `tool_call`, a
/// `tool_call_update` or a permission request.
#[derive(Clone, Copy, Default)]
pub(crate) struct Described<'a> {
    pub title: Option<&'a str>,
    pub raw_input: Option<&'a Value>,
    pub meta: Option<&'a Map<String, Value>>,
}

impl<'a> Described<'a> {
    pub fn call(call: &'a ToolCall) -> Self {
        Self {
            title: Some(&call.title),
            raw_input: call.raw_input.as_ref(),
            meta: call.meta.as_ref(),
        }
    }

    pub fn update(update: &'a ToolCallUpdate) -> Self {
        Self {
            title: update.fields.title.as_deref(),
            raw_input: update.fields.raw_input.as_ref(),
            meta: update.meta.as_ref(),
        }
    }
}

fn is_tool(name: &str) -> bool {
    agent_tools::tool_descriptions()
        .iter()
        .any(|(tool, _)| *tool == name)
}

/// Whether the call runs one of Levare's tools, told only from structured
/// fields the adapter fills itself, never from a title, which may be a shell
/// command, or from a tool's own arguments:
/// - Claude Code: `_meta.claudeCode.toolName` is exactly
///   `mcp__levare__<tool>` (claude-agent-acp `src/tool-calls/renderer.ts`
///   `toolUseMeta`; `src/acp-agent.ts` puts it on permission requests).
/// - Codex: a `tool_call` of an MCP call, flagged `_meta.is_mcp_tool_call`
///   (codex-acp `src/tool-calls/AcpToolCallRenderer.ts` `mcpMeta`, set only
///   by `McpToolReporter.started`), with `rawInput.server` `levare` and
///   `rawInput.tool` one of the tools, and its title `mcp.levare.<tool>`
///   (`McpToolReporter.ts`). The tool's arguments are under
///   `rawInput.arguments`, so they cannot pose as the server.
/// - Gemini CLI sends no such field, so its calls never count.
pub(crate) fn is_levare_tool(call: Described) -> bool {
    let meta = |key: &str| call.meta.and_then(|meta| meta.get(key));
    let claude = meta("claudeCode")
        .and_then(|claude| claude.get("toolName"))
        .and_then(Value::as_str)
        .and_then(|name| name.strip_prefix("mcp__levare__"))
        .is_some_and(is_tool);
    let input = |key: &str| {
        call.raw_input
            .and_then(|input| input.get(key))
            .and_then(Value::as_str)
    };
    let codex = meta("is_mcp_tool_call") == Some(&Value::Bool(true))
        && input("server") == Some(SERVER)
        && input("tool").is_some_and(|tool| {
            is_tool(tool) && call.title == Some(format!("mcp.{SERVER}.{tool}").as_str())
        });
    claude || codex
}

/// Whether the call is Codex's automatic review of a Levare tool call
/// («Guardian Review»), which says nothing the Levare call does not.
pub(crate) fn reviews_levare_tool(call: Described) -> bool {
    let action = call
        .raw_input
        .and_then(|input| input.get("action"))
        .and_then(Value::as_object);
    action.is_some_and(|action| {
        action.get("type").and_then(Value::as_str) == Some("mcpToolCall")
            && action.get("server").and_then(Value::as_str) == Some(SERVER)
    })
}

/// The tool calls of a session so far: updates and permission requests may
/// leave out what the call was opened with.
#[derive(Default)]
pub(crate) struct Calls {
    titles: HashMap<String, String>,
    levare: HashSet<String>,
    /// Reviews of Levare calls, not shown.
    hidden: HashSet<String>,
}

impl Calls {
    /// Notes what a report of call `id` says. Returns the title to show, or
    /// `None` if the call is not shown.
    pub fn report(&mut self, id: &str, call: Described) -> Option<String> {
        if let Some(title) = call.title {
            self.titles.insert(id.to_string(), title.to_string());
        }
        if is_levare_tool(call) {
            self.levare.insert(id.to_string());
        }
        if reviews_levare_tool(call) {
            self.hidden.insert(id.to_string());
        }
        if self.hidden.contains(id) {
            return None;
        }
        Some(self.title(id).unwrap_or_default().to_string())
    }

    pub fn title(&self, id: &str) -> Option<&str> {
        self.titles.get(id).map(String::as_str)
    }

    /// Whether call `id`, as described here or when it was reported, runs
    /// one of Levare's tools.
    pub fn is_levare(&self, id: &str, call: Described) -> bool {
        self.levare.contains(id) || is_levare_tool(call)
    }
}

/// The option that lets a Levare tool run, if the request is for one:
/// «allow once», or else «allow always», which the adapters keep for that
/// tool or server only.
pub(crate) fn auto_answer(calls: &Calls, request: &RequestPermissionRequest) -> Option<String> {
    let call = &request.tool_call;
    if !calls.is_levare(&call.tool_call_id.to_string(), Described::update(call)) {
        return None;
    }
    let with_kind = |kind: PermissionOptionKind| {
        request
            .options
            .iter()
            .find(|option: &&PermissionOption| option.kind == kind)
            .map(|option| option.option_id.to_string())
    };
    with_kind(PermissionOptionKind::AllowOnce)
        .or_else(|| with_kind(PermissionOptionKind::AllowAlways))
}
