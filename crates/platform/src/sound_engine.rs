//! Common contract for optional sound banks. The host always supplies General MIDI
//! fallback, mixing, reverb and transport; providers only synthesize dry audio.
mod general_midi;
mod harmonics;
#[cfg(test)]
mod tests;
pub(crate) use general_midi::GeneralMidi;
#[cfg(test)]
pub(crate) use general_midi::sound_font;

use crate::AudioError;
use score_core::{Instrument, Score};
pub use score_playback::NoteEvent;

pub const MAX_BLOCK_FRAMES: usize = 64;

/// A prepared, uniquely owned audio-thread adapter. All methods must be bounded,
/// nonblocking and allocation-free. Preparation/loading/destruction is control-side.
/// Channels are 0..128; channel 15 is reserved for audition, 9 for percussion.
pub trait SoundEngine: Send {
    /// Standard MIDI controllers only (7 volume, 10 pan, 11 expression). Timbre
    /// travels with each attack; the host sends no program changes.
    fn midi(&mut self, channel: u8, status: u8, a: u8, b: u8);
    /// Return false with no side effects if unsupported: the host plays GM instead.
    /// Level, accent and sounding length are already interpreted by the musical
    /// layer. Instrument, technique, articulation and slur are per attack, so
    /// providers choose a recording without tracking General MIDI programs.
    fn note_on(&mut self, channel: u8, note: NoteEvent) -> bool;
    /// May be called for keys this adapter declined; must be harmless.
    fn note_off(&mut self, channel: u8, key: u8);
    /// Release a harmonic attack independently of normal notes at the same key.
    /// Providers accepting `NoteEvent::harmonic` must implement this; providers
    /// without harmonic sounds safely leave it alone.
    fn harmonic_off(&mut self, _channel: u8, _key: u8) {}
    fn silence(&mut self, channel: Option<u8>, immediate: bool);
    /// ADD dry stereo audio to equally sized planes, at most MAX_BLOCK_FRAMES.
    fn render(&mut self, left: &mut [f32], right: &mut [f32]);
    /// Called before offline rendering only, never by the live device callback.
    fn offline(&mut self) {}
}

/// Optional provider preparation is fallible and happens before audio starts.
/// No provider type, file format or native library is exposed to the editor.
pub trait SoundBank: Send + Sync {
    fn id(&self) -> &'static str;
    fn name(&self) -> &'static str;
    fn description(&self) -> &'static str;
    fn prepare(
        &self,
        score: &Score,
        audition: Instrument,
        rate: u32,
    ) -> Result<Box<dyn SoundEngine>, AudioError>;
}

/// Only instrument/routing changes require preparing a new engine.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RoutingSignature {
    parts: Vec<(Instrument, bool)>,
    audition: Instrument,
}
impl RoutingSignature {
    pub fn new(score: &Score, audition: Instrument) -> Self {
        Self {
            parts: score
                .parts()
                .iter()
                .map(|part| {
                    (
                        part.instrument(),
                        part.instrument().is_grand_staff()
                            && score
                                .connections()
                                .iter()
                                .any(|c| c.kind.is_hairpin() && c.start.part == part.id()),
                    )
                })
                .collect(),
            audition,
        }
    }
}

pub(crate) struct Engines {
    fallback: GeneralMidi,
    extra: Option<Box<dyn SoundEngine>>,
}
impl Engines {
    pub(crate) fn new(rate: u32, extra: Option<Box<dyn SoundEngine>>) -> Result<Self, AudioError> {
        Ok(Self {
            fallback: GeneralMidi::new(rate)?,
            extra,
        })
    }
}
impl SoundEngine for Engines {
    fn midi(&mut self, c: u8, s: u8, a: u8, b: u8) {
        self.fallback.midi(c, s, a, b);
        if let Some(extra) = &mut self.extra {
            extra.midi(c, s, a, b);
        }
    }
    fn note_on(&mut self, c: u8, note: NoteEvent) -> bool {
        if !self.extra.as_mut().is_some_and(|e| e.note_on(c, note)) {
            self.fallback.note_on(c, note);
        }
        true
    }
    fn note_off(&mut self, c: u8, key: u8) {
        self.fallback.note_off(c, key);
        if let Some(extra) = &mut self.extra {
            extra.note_off(c, key);
        }
    }
    fn harmonic_off(&mut self, c: u8, key: u8) {
        self.fallback.harmonic_off(c, key);
        if let Some(extra) = &mut self.extra {
            extra.harmonic_off(c, key);
        }
    }
    fn silence(&mut self, channel: Option<u8>, immediate: bool) {
        self.fallback.silence(channel, immediate);
        if let Some(extra) = &mut self.extra {
            extra.silence(channel, immediate);
        }
    }
    fn render(&mut self, left: &mut [f32], right: &mut [f32]) {
        self.fallback.render(left, right);
        if let Some(extra) = &mut self.extra {
            extra.render(left, right);
        }
    }
    fn offline(&mut self) {
        if let Some(extra) = &mut self.extra {
            extra.offline();
        }
    }
}
