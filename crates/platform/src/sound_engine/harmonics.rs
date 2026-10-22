//! Built-in additive string-harmonic timbre. These are synthesized tones, not
//! recordings: a strong sounding fundamental, sparse upper partials, slight bow
//! noise and separate bowed/plucked envelopes. Nothing changes their pitch.
//! All tables and voice storage are prepared before the audio callback.
use super::{NoteEvent, SoundEngine};
use score_core::{Instrument, InstrumentFamily, PlayingTechnique};

const VOICES: usize = 512; // Covers the score's 448 held tones plus release tails.
const TABLE: usize = 2048;
const QUIET: f32 = 0.00001;

#[derive(Clone, Copy, Default)]
struct Voice {
    active: bool,
    released: bool,
    channel: u8,
    key: u8,
    phase: f32,
    increment: f32,
    vibrato: f32,
    vibrato_step: f32,
    amplitude: f32,
    envelope: f32,
    attack: f32,
    decay: f32,
    release: f32,
    color: f32,
    color_decay: f32,
    partials: [f32; 3],
    noise: u32,
    noise_low: f32,
    noise_level: f32,
}
#[derive(Clone, Copy)]
struct Channel {
    volume: f32,
    expression: f32,
    pan: f32,
    target: (f32, f32),
    gain: (f32, f32),
}
impl Default for Channel {
    fn default() -> Self {
        let mut value = Self {
            volume: 100. / 127.,
            expression: 1.,
            pan: 64.,
            target: (0., 0.),
            gain: (0., 0.),
        };
        value.update();
        value.gain = value.target;
        value
    }
}
impl Channel {
    fn update(&mut self) {
        // Match MIDI's center at 64 and reach the exact left/right endpoints.
        let angle = if self.pan <= 64. {
            self.pan / 64.
        } else {
            1. + (self.pan - 64.) / 63.
        } * std::f32::consts::FRAC_PI_4;
        // The plan reserves velocity headroom for hairpins. Matching squared
        // velocity/expression curves preserves the effective written dynamic.
        let level = self.volume.powi(2) * self.expression.powi(2);
        self.target = if self.pan == 0. {
            (level, 0.)
        } else if self.pan == 127. {
            (0., level)
        } else {
            (angle.cos() * level, angle.sin() * level)
        };
    }
}

pub(super) struct Harmonics {
    voices: Box<[Voice; VOICES]>,
    used: usize,
    channels: [Channel; score_playback::PLAYBACK_CHANNELS],
    sine: Box<[f32; TABLE + 1]>,
    frequencies: [f32; 128],
    rate: f32,
    smoothing: f32,
    noise_filter: f32,
    serial: u32,
}
impl Harmonics {
    pub(super) fn new(rate: u32) -> Self {
        let rate = rate as f32;
        Self {
            voices: Box::new([Voice::default(); VOICES]),
            used: 0,
            channels: [Channel::default(); score_playback::PLAYBACK_CHANNELS],
            sine: Box::new(std::array::from_fn(|i| {
                (i as f32 * std::f32::consts::TAU / TABLE as f32).sin()
            })),
            frequencies: std::array::from_fn(|key| 440. * 2f32.powf((key as f32 - 69.) / 12.)),
            rate,
            smoothing: 1. - (-1. / (rate * 0.005)).exp(),
            noise_filter: 1. - (-std::f32::consts::TAU * 2000. / rate).exp(),
            serial: 1,
        }
    }
}
fn sine(table: &[f32; TABLE + 1], phase: f32) -> f32 {
    let position = phase.fract() * TABLE as f32;
    let index = position as usize;
    table[index] + (table[index + 1] - table[index]) * (position - index as f32)
}
impl SoundEngine for Harmonics {
    fn midi(&mut self, channel: u8, status: u8, controller: u8, value: u8) {
        let Some(state) = self.channels.get_mut(usize::from(channel)) else {
            return;
        };
        if status != 0xb0 {
            return;
        }
        match controller {
            7 => state.volume = f32::from(value.min(127)) / 127.,
            10 => state.pan = f32::from(value.min(127)),
            11 => state.expression = f32::from(value.min(127)) / 127.,
            _ => return,
        }
        state.update();
        if value == 0 && matches!(controller, 7 | 11)
            || !self.voices[..self.used]
                .iter()
                .any(|v| v.active && v.channel == channel)
        {
            state.gain = state.target;
        }
    }
    fn note_on(&mut self, channel: u8, note: NoteEvent) -> bool {
        if note.harmonic.is_none() || note.instrument.family() != InstrumentFamily::Strings {
            return false;
        }
        if usize::from(channel) >= score_playback::PLAYBACK_CHANNELS || note.key >= 128 {
            return false;
        }
        // A pitch beyond this device's Nyquist limit cannot be synthesized.
        let increment = self.frequencies[usize::from(note.key)] / self.rate;
        if increment >= 0.5 {
            return true;
        }
        let slot = self
            .voices
            .iter()
            .position(|v| !v.active)
            .or_else(|| {
                self.voices
                    .iter()
                    .enumerate()
                    .filter(|(_, v)| v.released)
                    .min_by(|(_, a), (_, b)| a.envelope.total_cmp(&b.envelope))
                    .map(|(i, _)| i)
            })
            .unwrap_or_else(|| {
                self.voices
                    .iter()
                    .enumerate()
                    .min_by(|(_, a), (_, b)| a.envelope.total_cmp(&b.envelope))
                    .unwrap()
                    .0
            });
        let pizz = note.technique == PlayingTechnique::Pizzicato;
        let (body, pluck, speed) = match note.instrument {
            Instrument::Violin => (0.12, 0.48, 5.6),
            Instrument::Viola => (0.15, 0.62, 5.3),
            Instrument::Cello => (0.18, 0.85, 5.0),
            _ => (0.20, 1.1, 4.7),
        };
        self.serial = self.serial.wrapping_add(1);
        let mut partials = [body, body * 0.35, body * 0.12];
        for (index, partial) in partials.iter_mut().enumerate() {
            // Taper each partial before Nyquist, including at low device rates.
            *partial *= ((0.5 - increment * (index + 2) as f32) / 0.05).clamp(0., 1.);
        }
        self.voices[slot] = Voice {
            active: true,
            released: false,
            channel,
            key: note.key,
            phase: 0.,
            increment,
            vibrato: (self.serial % 97) as f32 / 97.,
            vibrato_step: speed / self.rate,
            amplitude: 0.15 * (f32::from(note.level.min(127)) / 127.).powi(2),
            envelope: 0.,
            attack: 1. - (-1. / (self.rate * if pizz { 0.0008 } else { 0.018 })).exp(),
            decay: if pizz {
                (-1. / (self.rate * pluck)).exp()
            } else {
                1.
            },
            release: (-1. / (self.rate * if pizz { 0.065 } else { 0.025 })).exp(),
            color: 1.,
            color_decay: if pizz {
                (-1. / (self.rate * 0.16)).exp()
            } else {
                1.
            },
            partials,
            noise: self.serial.wrapping_mul(747796405).wrapping_add(1),
            noise_low: 0.,
            noise_level: if pizz { 0. } else { 0.006 },
        };
        self.used = self.used.max(slot + 1);
        true
    }
    fn note_off(&mut self, channel: u8, key: u8) {
        for voice in &mut self.voices[..self.used] {
            if voice.active && voice.channel == channel && voice.key == key {
                voice.released = true;
            }
        }
    }
    fn harmonic_off(&mut self, channel: u8, key: u8) {
        self.note_off(channel, key);
    }
    fn silence(&mut self, channel: Option<u8>, immediate: bool) {
        for voice in &mut self.voices[..self.used] {
            if channel.is_none_or(|channel| channel == voice.channel) {
                if immediate {
                    voice.active = false;
                } else {
                    voice.released = true;
                }
            }
        }
        while self.used > 0 && !self.voices[self.used - 1].active {
            self.used -= 1;
        }
    }
    fn render(&mut self, left: &mut [f32], right: &mut [f32]) {
        if self.used == 0 {
            return;
        }
        for (left, right) in left.iter_mut().zip(right) {
            for channel in &mut self.channels {
                channel.gain.0 += (channel.target.0 - channel.gain.0) * self.smoothing;
                channel.gain.1 += (channel.target.1 - channel.gain.1) * self.smoothing;
            }
            for voice in &mut self.voices[..self.used] {
                if !voice.active {
                    continue;
                }
                // Attack and decay are independent: pizzicato dies away even
                // while its written note is held; arco maintains its energy.
                if voice.released {
                    voice.envelope *= voice.release;
                } else {
                    voice.envelope += (1. - voice.envelope) * voice.attack;
                }
                voice.amplitude *= voice.decay;
                if voice.envelope * voice.amplitude < QUIET && (voice.released || voice.decay < 1.)
                {
                    // Do not discard a newly attacked voice at its initial zero.
                    if voice.released || voice.amplitude < QUIET {
                        voice.active = false;
                        continue;
                    }
                }
                let s = sine(&self.sine, voice.phase);
                let c = sine(&self.sine, (voice.phase + 0.25).fract());
                let s2 = 2. * s * c;
                let s3 = (3. - 4. * s * s) * s;
                let s4 = 2. * s2 * (1. - 2. * s * s);
                voice.noise ^= voice.noise << 13;
                voice.noise ^= voice.noise >> 17;
                voice.noise ^= voice.noise << 5;
                let noise = voice.noise as f32 / u32::MAX as f32 * 2. - 1.;
                voice.noise_low += self.noise_filter * (noise - voice.noise_low);
                let value = (s
                    + voice.color
                        * (voice.partials[0] * s2
                            + voice.partials[1] * s3
                            + voice.partials[2] * s4)
                    + voice.noise_level * (noise - voice.noise_low))
                    * voice.envelope
                    * voice.amplitude;
                let gain = self.channels[usize::from(voice.channel)].gain;
                *left += value * gain.0;
                *right += value * gain.1;
                voice.color *= voice.color_decay;
                let vibrato = sine(&self.sine, voice.vibrato) * 0.0008 * voice.noise_level.signum();
                voice.phase = (voice.phase + voice.increment * (1. + vibrato)).fract();
                voice.vibrato = (voice.vibrato + voice.vibrato_step).fract();
            }
        }
        while self.used > 0 && !self.voices[self.used - 1].active {
            self.used -= 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use score_core::Harmonic;
    fn note(technique: PlayingTechnique) -> NoteEvent {
        NoteEvent {
            key: 81,
            level: 88,
            instrument: Instrument::Violin,
            technique,
            harmonic: Some(Harmonic::Natural),
            articulation: None,
            legato: false,
            tremolo: None,
        }
    }
    fn render(engine: &mut impl SoundEngine, frames: usize) -> Vec<f32> {
        let mut result = vec![0.; frames];
        let mut right = [0.; 64];
        assert_no_alloc::assert_no_alloc(|| {
            for block in result.chunks_mut(64) {
                right.fill(0.);
                engine.render(block, &mut right[..block.len()]);
            }
        });
        result
    }
    fn energy(values: &[f32]) -> f64 {
        values.iter().map(|v| f64::from(*v).powi(2)).sum::<f64>() / values.len() as f64
    }
    #[test]
    fn bow_sustains_while_pizzicato_decays_at_the_same_pitch() {
        let mut bowed = Harmonics::new(48_000);
        let mut plucked = Harmonics::new(48_000);
        assert_no_alloc::assert_no_alloc(|| {
            assert!(bowed.note_on(0, note(PlayingTechnique::Arco)));
            assert!(plucked.note_on(0, note(PlayingTechnique::Pizzicato)));
        });
        let bow = render(&mut bowed, 72_000);
        let pizz = render(&mut plucked, 72_000);
        let early = 4800..9600;
        let late = 57_600..62_400;
        assert!(energy(&bow[late.clone()]) > energy(&bow[early.clone()]) * 0.9);
        assert!(energy(&pizz[late]) < energy(&pizz[early]) * 0.03);
        assert!(
            bow.iter()
                .chain(&pizz)
                .all(|v| v.is_finite() && v.abs() < 0.2)
        );
        // Harmonic timbre has a dominant sounding fundamental (A5 = 880 Hz).
        let window = &bow[4800..19200];
        let projection = |frequency: f64| {
            let (mut re, mut im) = (0., 0.);
            for (i, value) in window.iter().enumerate() {
                let phase = std::f64::consts::TAU * frequency * i as f64 / 48_000.;
                re += f64::from(*value) * phase.cos();
                im += f64::from(*value) * phase.sin();
            }
            (re * re + im * im) * 2. / (window.len() as f64).powi(2)
        };
        assert!(projection(880.) / energy(window) > 0.85);
        assert!(
            projection(440.) / energy(window) < 0.0001,
            "the stopped/subharmonic pitch must not sound"
        );
        assert!(projection(1760.) / projection(880.) < 0.03);
    }
    #[test]
    fn voices_release_independently_and_channel_mute_pan_expression_are_live() {
        let mut engine = Harmonics::new(48_000);
        engine.midi(0, 0xb0, 7, 0);
        engine.note_on(0, note(PlayingTechnique::Arco));
        assert_eq!(energy(&render(&mut engine, 4800)), 0.);
        engine.midi(0, 0xb0, 7, 100);
        engine.midi(0, 0xb0, 10, 0);
        render(&mut engine, 4800);
        let mut left = [0.; 64];
        let mut right = left;
        engine.render(&mut left, &mut right);
        assert!(energy(&left) > 0.0001 && energy(&right) < 1e-12);
        let loud = energy(&render(&mut engine, 4800));
        engine.midi(0, 0xb0, 11, 32);
        render(&mut engine, 4800);
        assert!(energy(&render(&mut engine, 4800)) < loud * 0.08);
        engine.note_on(
            1,
            NoteEvent {
                key: 84,
                ..note(PlayingTechnique::Arco)
            },
        );
        assert_no_alloc::assert_no_alloc(|| engine.silence(Some(0), true));
        assert!(engine.voices.iter().any(|v| v.active && v.channel == 1));
        assert!(!engine.voices.iter().any(|v| v.active && v.channel == 0));
        engine.note_off(1, 84);
        render(&mut engine, 24_000);
        assert!(!engine.voices.iter().any(|v| v.active));
    }
    #[test]
    fn bounded_polyphony_all_rates_and_silence_stay_finite_without_allocations() {
        for rate in [16_000, 44_100, 48_000, 192_000] {
            let mut engine = Harmonics::new(rate);
            assert_no_alloc::assert_no_alloc(|| {
                for index in 0..VOICES + 10 {
                    let note = NoteEvent {
                        key: (index % 128) as u8,
                        ..note(PlayingTechnique::Arco)
                    };
                    assert!(engine.note_on((index % 32) as u8, note));
                }
                let mut left = [0.; 64];
                let mut right = left;
                engine.render(&mut left, &mut right);
                assert!(left.iter().chain(&right).all(|v| v.is_finite()));
                engine.silence(None, true);
                left.fill(0.);
                right.fill(0.);
                engine.render(&mut left, &mut right);
                assert!(left.iter().chain(&right).all(|v| *v == 0.));
            });
        }
    }
}

#[cfg(test)]
mod calibration_tests {
    use super::*;
    #[test]
    fn direct_velocity_and_hairpin_expression_preserve_the_same_effective_dynamic() {
        let energy = |velocity: u8, expression: u8, volume: u8| {
            let mut engine = Harmonics::new(48_000);
            engine.midi(0, 0xb0, 11, expression);
            engine.midi(0, 0xb0, 7, volume);
            engine.note_on(
                0,
                NoteEvent {
                    key: 81,
                    level: velocity,
                    instrument: Instrument::Violin,
                    technique: PlayingTechnique::Arco,
                    harmonic: Some(score_core::Harmonic::Natural),
                    articulation: None,
                    legato: false,
                    tremolo: None,
                },
            );
            let mut sum = 0.;
            for block in 0..150 {
                let mut left = [0.; 64];
                let mut right = left;
                engine.render(&mut left, &mut right);
                if block > 75 {
                    sum += left.iter().map(|v| v * v).sum::<f32>();
                }
            }
            sum
        };
        let plain = energy(56, 127, 100);
        let headroom = energy(104, 68, 100);
        assert!(
            (0.94..1.06).contains(&(headroom / plain)),
            "equal dynamic ratio {}",
            headroom / plain
        );
        let half_volume = energy(56, 127, 50) / plain;
        assert!(
            (0.060..0.066).contains(&half_volume),
            "MIDI mixer energy ratio {half_volume}"
        );
    }
}
