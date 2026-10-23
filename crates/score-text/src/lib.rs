//! Compact text notation for reading and writing one voice of one measure.
//! No dependency on the UI.
mod notation;
#[cfg(test)]
mod notation_tests;

pub use notation::{TextError, WrittenEvent, format_voice, parse_voice};
