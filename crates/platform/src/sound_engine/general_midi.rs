use super::{MAX_BLOCK_FRAMES, NoteEvent, SoundEngine};
use crate::{AudioError, error};
use rustysynth::{SoundFont, Synthesizer, SynthesizerSettings};
use std::{
    io::Cursor,
    sync::{Arc, OnceLock},
};

// The bank is parsed once, before any audio callback. All synth buffers and
// voices are allocated by Synthesizer::new, on this same control thread.
pub(crate) fn sound_font() -> Result<Arc<SoundFont>, AudioError> {
    static BANK: OnceLock<Result<Arc<SoundFont>, String>> = OnceLock::new();
    BANK.get_or_init(|| {
        SoundFont::new(&mut Cursor::new(
            include_bytes!("../../../../assets/sounds/GeneralUser-GS.sf2").as_slice(),
        ))
        .map(Arc::new)
        .map_err(|error| error.to_string())
    })
    .as_ref()
    .map(Arc::clone)
    .map_err(error)
}

pub(crate) struct GeneralMidi {
    synth: Synthesizer,
    lower: Synthesizer,
    /// Current General MIDI program per host channel, derived from each attack.
    programs: [Option<u8>; 32],
    left: [f32; MAX_BLOCK_FRAMES],
    right: [f32; MAX_BLOCK_FRAMES],
}
impl GeneralMidi {
    pub(crate) fn new(rate: u32) -> Result<Self, AudioError> {
        let mut settings = SynthesizerSettings::new(rate as i32);
        settings.enable_reverb_and_chorus = false;
        settings.maximum_polyphony = 256;
        let mut synth = Synthesizer::new(&sound_font()?, &settings).map_err(error)?;
        let mut lower = Synthesizer::new(&sound_font()?, &settings).map_err(error)?;
        synth.set_master_volume(0.5);
        lower.set_master_volume(0.5);
        Ok(Self {
            synth,
            lower,
            programs: [None; 32],
            left: [0.; MAX_BLOCK_FRAMES],
            right: [0.; MAX_BLOCK_FRAMES],
        })
    }
    fn channel(&mut self, channel: u8) -> (&mut Synthesizer, i32) {
        (
            if channel >= 16 {
                &mut self.lower
            } else {
                &mut self.synth
            },
            i32::from(channel % 16),
        )
    }
}
impl SoundEngine for GeneralMidi {
    fn midi(&mut self, channel: u8, status: u8, a: u8, b: u8) {
        if status == 0xc0 {
            self.programs[usize::from(channel)] = Some(a);
        }
        let (synth, c) = self.channel(channel);
        synth.process_midi_message(c, i32::from(status), i32::from(a), i32::from(b));
    }
    /// GM has no articulation recordings: the plan's level and length carry them.
    /// A program change only affects later attacks, so held notes keep their timbre.
    fn note_on(&mut self, channel: u8, note: NoteEvent) -> bool {
        let program = note.technique.program(note.instrument);
        if self.programs[usize::from(channel)] != Some(program) {
            self.midi(channel, 0xc0, program, 0);
        }
        let (synth, c) = self.channel(channel);
        synth.note_on(c, i32::from(note.key), i32::from(note.level));
        true
    }
    fn note_off(&mut self, channel: u8, key: u8) {
        let (synth, c) = self.channel(channel);
        synth.note_off(c, i32::from(key));
    }
    fn silence(&mut self, channel: Option<u8>, immediate: bool) {
        if let Some(channel) = channel {
            let (synth, c) = self.channel(channel);
            synth.note_off_all_channel(c, immediate);
        } else {
            self.synth.note_off_all(immediate);
            self.lower.note_off_all(immediate);
        }
    }
    fn render(&mut self, left: &mut [f32], right: &mut [f32]) {
        let n = left.len();
        for synth in [&mut self.synth, &mut self.lower] {
            synth.render(&mut self.left[..n], &mut self.right[..n]);
            for i in 0..n {
                left[i] += self.left[i];
                right[i] += self.right[i];
            }
        }
    }
}
