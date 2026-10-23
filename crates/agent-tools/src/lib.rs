//! The agent's tools over the open score: read, select, run registry commands
//! and write whole bars, through the same editor the user works with. No
//! dependency on the UI or on the transport that serves the tools.
mod catalog;
#[cfg(test)]
mod catalog_tests;
mod instructions;
mod tools;
#[cfg(test)]
mod tools_tests;

pub use catalog::{EXCLUDED, allowed_commands};
pub use instructions::{INSTRUCTIONS, READ_ONLY_TOOLS, tool_descriptions};
pub use tools::{
    AjustarArgs, ComandosArgs, ConnectionClass, EjecutarArgs, EscribirArgs, LeerArgs, Position,
    SeleccionarArgs, TextClass, ToolCall, dispatch,
};

use score_edit::Editor;
use score_keymap::CommandId;

/// What the tools need from the app: its editor and its command dispatcher.
pub trait Host {
    fn editor(&self) -> &Editor;
    fn editor_mut(&mut self) -> &mut Editor;
    /// Runs a registry command as a shortcut would; `Err` carries a Spanish
    /// message for the agent.
    fn run(&mut self, id: CommandId) -> Result<(), String>;
}
