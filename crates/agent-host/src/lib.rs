//! Serves Levare's agent tools over MCP and runs the agent's session, on a
//! tokio runtime of its own. No dependency on the UI: tool calls leave
//! through a channel for the UI thread to run.
mod mcp;
mod runtime;
mod schema;

pub use mcp::{McpServer, ToolInvocation};
pub use runtime::Runtime;
