//! Shortcut engine: command registry, keystrokes, profiles and the user's
//! changes, resolved into one keymap. Independent of GPUI so it can be tested
//! and reused; `levare` translates the result into toolkit bindings.

pub mod commands;
pub mod docs;
mod gestures;
mod keymap;
mod keys;
mod profile;
mod user;

pub use commands::{Category, CommandId, CommandSpec, Registry, Scope, Status, id};
pub use gestures::LineDrag;
pub use keymap::{Binding, Keymap, Origin, Problem};
pub use keys::{KeyError, KeySequence, Keystroke, Layout, Modifier, Platform, UsLayout};
pub use profile::Profile;
pub use user::{UserError, UserKeymap};
