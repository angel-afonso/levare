//! Compact text notation for reading and writing one voice of one measure.
//! No dependency on the UI.
mod notation;
#[cfg(test)]
mod notation_tests;
mod position;
#[cfg(test)]
mod position_tests;
mod quote;

pub use notation::{TextError, WrittenEvent, format_voice, parse_voice};
pub use position::{bar_index, beat_tick, event_at, resolve_part};
pub use quote::{part_label, quoted};
