//! Serves Levare's agent tools over MCP and runs the agent's session, on a
//! tokio runtime of its own. No dependency on the UI: tool calls leave
//! through a channel for the UI thread to run.
mod agents;
#[cfg(test)]
mod agents_tests;
mod calls;
#[cfg(test)]
mod calls_tests;
#[cfg(any(windows, test))]
mod job;
#[cfg(all(test, windows))]
mod job_tests;
mod mcp;
mod runtime;
mod schema;
mod session;
#[cfg(test)]
mod session_tests;

pub use agents::{
    AgentKind, LaunchCommand, PathSource, ResolvedTools, explain_error, launch_command,
    parse_probe, probe, resolve_tools,
};
pub use mcp::{McpServer, ToolInvocation};
pub use runtime::Runtime;
pub use session::{AgentEvent, SessionHandle, start_session};
