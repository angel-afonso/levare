use crate::{AudioError, error};
use cpal::{FromSample, Sample};
use rtrb::{Consumer, Producer, RingBuffer};
use rustysynth::{SoundFont, Synthesizer, SynthesizerSettings};
use score_playback::{Message, PlaybackPlan};
use std::{
    io::Cursor,
    sync::{
        Arc, OnceLock,
        atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering},
    },
};

const QUEUE_CAPACITY: usize = 64;
const BLOCK: usize = 64;

struct Shared {
    epoch: AtomicU64,
    started: AtomicU64,
    finished: AtomicU64,
    frame: AtomicU64,
    volume: AtomicU32,
    failed: Arc<AtomicBool>,
    paused: AtomicBool,
}

// Inline bounded payloads keep ownership and allocation out of the callback.
#[allow(clippy::large_enum_variant)]
enum Command {
    Play {
        epoch: u64,
        plan: Arc<PlaybackPlan>,
        start_frame: u64,
        held_notes: [Option<score_playback::HeldNote>; score_core::MAX_SOUNDING_NOTES],
        expressions: [u8; score_playback::PLAYBACK_CHANNELS],
    },
    Preview {
        epoch: u64,
        keys: [Option<u8>; score_core::MAX_CHORD_NOTES],
        program: u8,
    },
}

struct Active {
    epoch: u64,
    plan: Arc<PlaybackPlan>,
    start_frame: u64,
}

/// All allocation, plan preparation and final plan destruction happen here,
/// on the control thread. Retained owners outlive queued and active references.
pub struct Control {
    commands: Producer<Command>,
    shared: Arc<Shared>,
    retained: Vec<Arc<PlaybackPlan>>,
    active: Option<Active>,
    sample_rate: u32,
}

#[derive(Clone, Copy, Debug)]
pub struct PlaybackPosition {
    pub frame: u64,
    pub element: Option<score_core::ElementId>,
    pub elements: [Option<score_core::ElementId>; score_core::MAX_STAVES],
}

impl Control {
    pub fn play(&mut self, plan: Arc<PlaybackPlan>) -> Result<(), AudioError> {
        self.start(plan, 0, false)
    }

    pub fn play_from(
        &mut self,
        plan: Arc<PlaybackPlan>,
        element: score_core::ElementId,
    ) -> Result<(), AudioError> {
        let frame = plan
            .frame_for_element(element)
            .ok_or_else(|| error("La selección no existe en la partitura"))?;
        self.start(plan, frame, false)
    }

    /// Seek to musical time, preserving pause even before the next callback.
    pub fn play_from_frame(
        &mut self,
        plan: Arc<PlaybackPlan>,
        frame: u64,
    ) -> Result<(), AudioError> {
        if frame >= plan.duration_frames {
            return Err(error("La posición está fuera de la partitura"));
        }
        let paused = self.is_paused();
        self.start(plan, frame, paused)
    }

    fn start(
        &mut self,
        plan: Arc<PlaybackPlan>,
        start_frame: u64,
        paused: bool,
    ) -> Result<(), AudioError> {
        if self.failed() {
            return Err(error("La salida de audio se desconectó"));
        }
        if plan.sample_rate != self.sample_rate {
            return Err(error("La reproducción usa otra frecuencia de audio"));
        }
        if plan.duration_frames == 0 {
            return Err(error("La partitura está vacía"));
        }
        // A paused seek must never clear the pause flag, even between generation
        // changes and publishing Play: the callback may run between any stores.
        self.shared.epoch.fetch_add(1, Ordering::AcqRel);
        self.shared.paused.store(paused, Ordering::Release);
        self.active = None;
        self.reclaim();
        let epoch = self.shared.epoch.load(Ordering::Acquire);
        // Replaying the same Arc must not create multiple control-side owners
        // that keep each other's strong_count above one after playback ends.
        if !self
            .retained
            .iter()
            .any(|retained| Arc::ptr_eq(retained, &plan))
        {
            self.retained.push(plan.clone());
        }
        self.commands
            .push(Command::Play {
                epoch,
                held_notes: plan.held_notes_at(start_frame),
                expressions: plan.expressions_at(start_frame),
                plan: plan.clone(),
                start_frame,
            })
            .map_err(|_| error("La cola de audio está ocupada; inténtalo de nuevo"))?;
        self.active = Some(Active {
            epoch,
            plan,
            start_frame,
        });
        Ok(())
    }

    pub fn preview(&mut self, key: u8) -> Result<(), AudioError> {
        self.preview_with_program(key, 0)
    }
    pub fn preview_with_program(&mut self, key: u8, program: u8) -> Result<(), AudioError> {
        let mut keys = [None; score_core::MAX_CHORD_NOTES];
        keys[0] = Some(key);
        self.preview_chord(keys, program)
    }
    pub fn preview_chord(
        &mut self,
        keys: [Option<u8>; score_core::MAX_CHORD_NOTES],
        program: u8,
    ) -> Result<(), AudioError> {
        if program > 127
            || keys.iter().flatten().any(|key| *key > 127)
            || keys.iter().all(Option::is_none)
        {
            return Err(error("Notas MIDI inválidas"));
        }
        let mut seen = [false; 128];
        for key in keys.into_iter().flatten() {
            if seen[usize::from(key)] {
                return Err(error("Notas MIDI duplicadas"));
            }
            seen[usize::from(key)] = true;
        }
        if self.failed() {
            return Err(error("La salida de audio se desconectó"));
        }
        self.commands
            .push(Command::Preview {
                epoch: self.shared.epoch.load(Ordering::Acquire),
                keys,
                program,
            })
            .map_err(|_| error("La cola de audio está ocupada"))
    }

    /// Stop is an atomic generation change, so even a saturated queue cannot
    /// delay it or make an old queued start sound after the stop.
    pub fn stop(&mut self) {
        self.shared.epoch.fetch_add(1, Ordering::AcqRel);
        self.shared.paused.store(false, Ordering::Release);
        self.active = None;
    }

    fn has_active(&self) -> bool {
        !self.failed()
            && self
                .active
                .as_ref()
                .is_some_and(|active| self.shared.finished.load(Ordering::Acquire) != active.epoch)
    }

    pub fn is_playing(&self) -> bool {
        self.has_active() && !self.shared.paused.load(Ordering::Acquire)
    }
    pub fn is_paused(&self) -> bool {
        self.has_active() && self.shared.paused.load(Ordering::Acquire)
    }

    pub fn pause(&mut self) {
        if self.has_active() {
            self.shared.paused.store(true, Ordering::Release);
        }
    }

    pub fn resume(&mut self) {
        if self.has_active() {
            self.shared.paused.store(false, Ordering::Release);
        }
    }

    pub fn position(&mut self) -> Option<PlaybackPosition> {
        if !self.has_active() {
            self.active = None;
        }
        self.reclaim();
        let active = self.active.as_ref()?;
        let frame = if self.shared.started.load(Ordering::Acquire) == active.epoch {
            self.shared.frame.load(Ordering::Acquire)
        } else {
            active.start_frame
        };
        Some(PlaybackPosition {
            frame,
            element: active.plan.element_at(frame),
            elements: active.plan.elements_at(frame),
        })
    }
    pub fn failed(&self) -> bool {
        self.shared.failed.load(Ordering::Acquire)
    }
    pub fn set_volume(&mut self, percent: u32) {
        self.shared
            .volume
            .store(percent.min(100), Ordering::Relaxed);
    }
    fn reclaim(&mut self) {
        self.retained.retain(|plan| Arc::strong_count(plan) > 1);
    }
}

// The bank is parsed once, before any audio callback. All synth buffers and
// voices are allocated by Synthesizer::new, on this same control thread.
fn sound_font() -> Result<Arc<SoundFont>, AudioError> {
    static BANK: OnceLock<Result<Arc<SoundFont>, String>> = OnceLock::new();
    BANK.get_or_init(|| {
        SoundFont::new(&mut Cursor::new(
            include_bytes!("../../../assets/sounds/GeneralUser-GS.sf2").as_slice(),
        ))
        .map(Arc::new)
        .map_err(|error| error.to_string())
    })
    .as_ref()
    .map(Arc::clone)
    .map_err(error)
}

pub struct Renderer {
    commands: Consumer<Command>,
    shared: Arc<Shared>,
    synth: Synthesizer,
    lower_synth: Synthesizer,
    active: Option<Active>,
    epoch: u64,
    frame: u64,
    next_message: usize,
    clock: u64,
    preview_off: Option<u64>,
    sample_rate: u32,
    left: [f32; BLOCK],
    right: [f32; BLOCK],
    lower_left: [f32; BLOCK],
    lower_right: [f32; BLOCK],
}

impl Renderer {
    /// Also usable offline, to exercise the exact callback without audio hardware.
    pub fn new(sample_rate: u32) -> Result<(Control, Self), AudioError> {
        if !(16_000..=192_000).contains(&sample_rate) {
            return Err(error("Frecuencia de audio no compatible"));
        }
        let mut settings = SynthesizerSettings::new(sample_rate as i32);
        settings.enable_reverb_and_chorus = false;
        // Reserve RustySynth's maximum for chords, sample layers and release tails.
        settings.maximum_polyphony = 256;
        let mut synth = Synthesizer::new(&sound_font()?, &settings).map_err(error)?;
        synth.set_master_volume(0.5);
        let mut lower_synth = Synthesizer::new(&sound_font()?, &settings).map_err(error)?;
        lower_synth.set_master_volume(0.5);
        let (producer, consumer) = RingBuffer::new(QUEUE_CAPACITY);
        let shared = Arc::new(Shared {
            epoch: AtomicU64::new(1),
            started: AtomicU64::new(0),
            finished: AtomicU64::new(0),
            frame: AtomicU64::new(0),
            volume: AtomicU32::new(70),
            failed: Arc::new(AtomicBool::new(false)),
            paused: AtomicBool::new(false),
        });
        let control = Control {
            commands: producer,
            shared: shared.clone(),
            retained: Vec::new(),
            active: None,
            sample_rate,
        };
        let renderer = Self {
            commands: consumer,
            shared,
            synth,
            lower_synth,
            lower_left: [0.; BLOCK],
            lower_right: [0.; BLOCK],
            active: None,
            epoch: 1,
            frame: 0,
            next_message: 0,
            clock: 0,
            preview_off: None,
            sample_rate,
            left: [0.; BLOCK],
            right: [0.; BLOCK],
        };
        Ok((control, renderer))
    }

    pub(crate) fn failure_flag(&self) -> Arc<AtomicBool> {
        self.shared.failed.clone()
    }

    fn synth_for(&mut self, channel: u8) -> (&mut Synthesizer, i32) {
        (
            if channel >= 16 {
                &mut self.lower_synth
            } else {
                &mut self.synth
            },
            i32::from(channel % 16),
        )
    }
    fn check_stop(&mut self) {
        let epoch = self.shared.epoch.load(Ordering::Acquire);
        if epoch != self.epoch {
            self.synth.note_off_all(true);
            self.lower_synth.note_off_all(true);
            self.active = None;
            self.preview_off = None;
            self.frame = 0;
            self.next_message = 0;
            self.epoch = epoch;
        }
    }

    fn commands(&mut self) {
        self.check_stop();
        // A fixed bound holds even if the producer keeps writing concurrently.
        for _ in 0..QUEUE_CAPACITY {
            let Ok(command) = self.commands.pop() else {
                break;
            };
            // Adopt Stop before testing the command. Never adopt a newer epoch
            // between accepting an old command and installing it, or that old
            // start could become indistinguishable from the new stopped state.
            self.check_stop();
            match command {
                Command::Play {
                    epoch,
                    plan,
                    start_frame,
                    held_notes,
                    expressions,
                } if epoch == self.epoch => {
                    self.synth.note_off_all(true);
                    self.lower_synth.note_off_all(true);
                    for (channel, value) in expressions.into_iter().enumerate() {
                        let (synth, channel) = self.synth_for(channel as u8);
                        synth.process_midi_message(channel, 0xb0, 11, i32::from(value));
                    }
                    for track in &plan.tracks {
                        let (synth, channel) = self.synth_for(track.channel);
                        synth.process_midi_message(channel, 0xc0, i32::from(track.program), 0);
                    }
                    // Chase both the original attack and current expression level.
                    for note in held_notes.into_iter().flatten() {
                        let (synth, channel) = self.synth_for(note.channel);
                        synth.process_midi_message(channel, 0xc0, i32::from(note.program), 0);
                        synth.note_on(channel, i32::from(note.key), i32::from(note.velocity));
                    }
                    self.next_message = plan
                        .messages
                        .partition_point(|message| message.frame < start_frame);
                    self.active = Some(Active {
                        epoch,
                        plan,
                        start_frame,
                    });
                    self.frame = start_frame;
                    self.preview_off = None;
                    self.shared.frame.store(start_frame, Ordering::Release);
                    self.shared.started.store(epoch, Ordering::Release);
                }
                Command::Preview {
                    epoch,
                    keys,
                    program,
                } if epoch == self.epoch => {
                    self.synth.note_off_all_channel(15, true);
                    self.synth
                        .process_midi_message(15, 0xc0, i32::from(program), 0);
                    for key in keys.into_iter().flatten() {
                        self.synth.note_on(15, i32::from(key), 88);
                    }
                    self.preview_off = Some(self.clock + u64::from(self.sample_rate) / 4);
                }
                _ => {} // Old commands are invalidated by Stop; owners stay in Control.
            }
        }
    }

    fn due_messages(&mut self) {
        while let Some(message) = self
            .active
            .as_ref()
            .and_then(|active| active.plan.messages.get(self.next_message))
            .copied()
        {
            if message.frame > self.frame {
                break;
            }
            let (synth, channel) = self.synth_for(message.channel);
            match message.message {
                Message::Expression(value) => {
                    synth.process_midi_message(channel, 0xb0, 11, i32::from(value))
                }
                Message::ProgramChange(program) => {
                    synth.process_midi_message(channel, 0xc0, i32::from(program), 0)
                }
                Message::NoteOn { key, velocity } => {
                    synth.note_on(channel, i32::from(key), i32::from(velocity))
                }
                Message::NoteOff(key) => synth.note_off(channel, i32::from(key)),
            }
            self.next_message += 1;
        }
        if self
            .active
            .as_ref()
            .is_some_and(|active| self.frame >= active.plan.duration_frames)
        {
            // Control retains a strong owner. Dropping this reference cannot
            // destroy a plan or free its heap storage on the callback thread.
            self.active = None;
            self.shared.finished.store(self.epoch, Ordering::Release);
        }
        if self.preview_off.is_some_and(|off| self.clock >= off) {
            self.synth.note_off_all_channel(15, false);
            self.preview_off = None;
        }
    }

    /// No locks, allocation, disk I/O, formatting or UI calls. Output format
    /// conversion and stereo/mono routing use preallocated stack buffers.
    pub fn render<T: Sample + FromSample<f32>>(&mut self, output: &mut [T], channels: usize) {
        if channels == 0 {
            return;
        }
        self.commands();
        let total_frames = output.len() / channels;
        let mut written = 0;
        while written < total_frames {
            self.check_stop();
            if self.active.is_some() && self.shared.paused.load(Ordering::Acquire) {
                // Freeze the sample clock AND the synthesizer's envelopes,
                // oscillator phase and cached block. Resume continues this
                // very note; no new attack or callback allocations are needed.
                let count = (total_frames - written).min(BLOCK);
                for sample in &mut output[written * channels..(written + count) * channels] {
                    *sample = T::from_sample(0.);
                }
                self.clock += count as u64;
                written += count;
                continue;
            }
            self.due_messages();
            let mut count = (total_frames - written).min(BLOCK);
            if let Some(active) = &self.active {
                let next = active
                    .plan
                    .messages
                    .get(self.next_message)
                    .map(|message| message.frame)
                    .unwrap_or(active.plan.duration_frames);
                count = count.min((next - self.frame) as usize);
            }
            if let Some(off) = self.preview_off {
                count = count.min((off - self.clock) as usize);
            }
            // due_messages consumed all boundaries at the current frame.
            debug_assert!(count > 0);
            self.synth
                .render(&mut self.left[..count], &mut self.right[..count]);
            self.lower_synth.render(
                &mut self.lower_left[..count],
                &mut self.lower_right[..count],
            );
            for i in 0..count {
                self.left[i] += self.lower_left[i];
                self.right[i] += self.lower_right[i];
            }
            let volume = self.shared.volume.load(Ordering::Relaxed) as f32 / 100.;
            for index in 0..count {
                let frame =
                    &mut output[(written + index) * channels..(written + index + 1) * channels];
                if channels == 1 {
                    frame[0] = T::from_sample(
                        ((self.left[index] + self.right[index]) * 0.5 * volume).clamp(-1., 1.),
                    );
                } else {
                    frame[0] = T::from_sample((self.left[index] * volume).clamp(-1., 1.));
                    frame[1] = T::from_sample((self.right[index] * volume).clamp(-1., 1.));
                    for channel in &mut frame[2..] {
                        *channel = T::from_sample(0.);
                    }
                }
            }
            written += count;
            self.clock += count as u64;
            if self.active.is_some() {
                self.frame += count as u64;
            }
        }
        if !self.shared.paused.load(Ordering::Acquire) {
            self.due_messages();
        }
        self.shared.frame.store(self.frame, Ordering::Release);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use assert_no_alloc::assert_no_alloc;
    use score_core::{Duration, Event, Pitch, Score, Step};
    use score_playback::Tempo;

    #[test]
    fn chord_audition_and_selected_chord_pause_resume_without_callback_allocations() {
        let pitches = score_core::Pitches::new(&[
            Pitch::new(Step::C, 4)
                .unwrap()
                .with_accidental(Some(score_core::Accidental::Sharp)),
            Pitch::new(Step::E, 4).unwrap(),
            Pitch::new(Step::G, 4).unwrap(),
        ])
        .unwrap();
        let mut score = Score::default();
        let selected = score
            .insert(
                0,
                0,
                Event {
                    notation: Default::default(),
                    pitches,
                    duration: Duration::Whole,
                },
            )
            .unwrap();
        let plan = Arc::new(PlaybackPlan::new(&score, Tempo::default(), 48_000).unwrap());
        let (mut control, mut renderer) = Renderer::new(48_000).unwrap();
        let (mut reference_control, mut reference) = Renderer::new(48_000).unwrap();
        let mut actual = [0f32; 4096];
        let mut expected = [0f32; 4096];
        let mut keys = [None; score_core::MAX_CHORD_NOTES];
        for (slot, pitch) in keys.iter_mut().zip(pitches.iter()) {
            *slot = Some(score_playback::midi_key(pitch));
        }
        control.preview_chord(keys, 0).unwrap();
        assert_no_alloc(|| renderer.render(&mut actual, 2));
        assert!(actual.iter().map(|sample| sample * sample).sum::<f32>() > 0.01);
        // Fresh renderers eliminate audition release tails from the comparison.
        let (mut control2, mut renderer2) = Renderer::new(48_000).unwrap();
        control2.play_from(plan.clone(), selected).unwrap();
        reference_control.play_from(plan, selected).unwrap();
        assert_no_alloc(|| {
            renderer2.render(&mut actual, 2);
            reference.render(&mut expected, 2);
            control2.pause();
            renderer2.render(&mut actual, 2);
            assert!(actual.iter().all(|sample| *sample == 0.));
            control2.resume();
            renderer2.render(&mut actual, 2);
            reference.render(&mut expected, 2);
        });
        assert_eq!(actual, expected);
        assert!(actual.iter().any(|sample| sample.abs() > 0.001));
    }

    fn plan() -> Arc<PlaybackPlan> {
        let mut score = Score::default();
        score
            .insert(
                0,
                0,
                Event {
                    notation: Default::default(),
                    pitches: score_core::Pitches::single(Pitch::new(Step::A, 4)),
                    duration: Duration::Quarter,
                },
            )
            .unwrap();
        score
            .insert(
                0,
                1,
                Event {
                    notation: Default::default(),
                    pitches: score_core::Pitches::single(Pitch::new(Step::A, 4)),
                    duration: Duration::Quarter,
                },
            )
            .unwrap();
        Arc::new(PlaybackPlan::new(&score, Tempo::default(), 48_000).unwrap())
    }

    #[test]
    fn tied_continuation_chases_its_original_dynamic_and_pause_keeps_exact_audio() {
        use score_core::{Connection, ConnectionKind, Dynamic, Notation};
        let mut score = Score::default();
        let event = Event {
            pitches: score_core::Pitches::single(Pitch::new(Step::A, 4)),
            duration: Duration::Quarter,
            notation: Notation::default(),
        };
        let a = score.insert(0, 0, event).unwrap();
        let b = score.insert(0, 1, event).unwrap();
        score.set_dynamic(a, Some(Dynamic::Pp)).unwrap();
        score.set_dynamic(b, Some(Dynamic::Fff)).unwrap();
        score
            .add_connection(Connection {
                start: a,
                end: b,
                kind: ConnectionKind::Tie,
            })
            .unwrap();
        let plan = Arc::new(PlaybackPlan::new(&score, Tempo::default(), 48_000).unwrap());
        let (mut control, mut renderer) = Renderer::new(48_000).unwrap();
        let (mut reference_control, mut reference) = Renderer::new(48_000).unwrap();
        control.play_from(plan.clone(), b).unwrap();
        reference_control.play_from(plan, a).unwrap();
        let mut actual = [0f32; 4096];
        let mut expected = [0f32; 4096];
        assert_no_alloc(|| {
            renderer.render(&mut actual, 2);
            reference.render(&mut expected, 2);
            assert_eq!(actual, expected);
            assert!(actual.iter().any(|v| v.abs() > 0.0001));
            control.pause();
            renderer.render(&mut actual, 2);
            assert!(actual.iter().all(|v| *v == 0.));
            control.resume();
            renderer.render(&mut actual, 2);
            reference.render(&mut expected, 2);
            assert_eq!(actual, expected);
        });
    }

    #[test]
    fn ensemble_selection_chases_held_notes_and_pause_resumes_exact_audio() {
        let mut score = Score::default();
        let piano = score
            .insert(
                0,
                0,
                Event {
                    notation: Default::default(),
                    pitches: score_core::Pitches::single(Pitch::new(Step::A, 4)),
                    duration: Duration::Whole,
                },
            )
            .unwrap();
        let left = score
            .insert_on(
                score.parts()[0].id(),
                score_core::StaffId::Lower,
                0,
                0,
                Event {
                    notation: Default::default(),
                    pitches: score_core::Pitches::single(Pitch::new(Step::G, 3)),
                    duration: Duration::Whole,
                },
            )
            .unwrap();
        let flute = score.add_part(score_core::Instrument::Flute).unwrap();
        score
            .insert_in(
                flute,
                0,
                0,
                Event {
                    notation: Default::default(),
                    pitches: score_core::Pitches::single(None),
                    duration: Duration::Quarter,
                },
            )
            .unwrap();
        let rest = score
            .insert_in(
                flute,
                0,
                1,
                Event {
                    notation: Default::default(),
                    pitches: score_core::Pitches::single(None),
                    duration: Duration::Quarter,
                },
            )
            .unwrap();
        let plan = Arc::new(PlaybackPlan::new(&score, Tempo::default(), 48_000).unwrap());
        let (mut control, mut renderer) = Renderer::new(48_000).unwrap();
        let (mut reference_control, mut reference) = Renderer::new(48_000).unwrap();
        control.play_from(plan.clone(), rest).unwrap();
        reference_control.play_from(plan, rest).unwrap();
        let mut warmup = [0f32; 512];
        let mut expected = [0f32; 4096];
        let mut actual = [0f32; 4096];
        assert_no_alloc(|| {
            renderer.render(&mut warmup, 2);
            reference.render(&mut warmup, 2);
            control.pause();
            renderer.render(&mut actual, 2);
            assert!(actual.iter().all(|sample| *sample == 0.));
            control.resume();
            renderer.render(&mut actual, 2);
            reference.render(&mut expected, 2);
        });
        assert_eq!(actual, expected);
        assert!(
            actual.iter().map(|sample| sample * sample).sum::<f32>() > 0.01,
            "The held piano must sound even though the selected flute event is a rest"
        );
        let position = control.position().unwrap();
        assert_eq!(
            position.elements.into_iter().flatten().collect::<Vec<_>>(),
            vec![piano, left, rest]
        );
        control.stop();
        assert_no_alloc(|| renderer.render(&mut actual, 2));
    }

    #[test]
    fn adding_a_later_hairpin_preserves_the_existing_volume_and_timbre() {
        use score_core::{Connection, ConnectionKind, Dynamic, Pitches};
        let mut s = Score::default();
        let event = Event {
            pitches: Pitches::single(Pitch::new(Step::A, 4)),
            duration: Duration::Whole,
            notation: Default::default(),
        };
        let first = s.insert(0, 0, event).unwrap();
        let a = s.insert(1, 0, event).unwrap();
        let b = s.insert(2, 0, event).unwrap();
        s.set_dynamic(first, Some(Dynamic::P)).unwrap();
        s.set_dynamic(b, Some(Dynamic::F)).unwrap();
        let plain = Arc::new(PlaybackPlan::new(&s, Tempo::default(), 48_000).unwrap());
        s.add_connection(Connection {
            start: a,
            end: b,
            kind: ConnectionKind::Crescendo,
        })
        .unwrap();
        let ramp = Arc::new(PlaybackPlan::new(&s, Tempo::default(), 48_000).unwrap());
        assert!(ramp.messages.iter().any(|m| m.frame == 0
            && m.message
                == Message::NoteOn {
                    key: 69,
                    velocity: 56
                }));
        assert_eq!(ramp.expressions_at(0)[0], 127);
        let (mut a_control, mut a_renderer) = Renderer::new(48_000).unwrap();
        let (mut b_control, mut b_renderer) = Renderer::new(48_000).unwrap();
        a_control.play(plain.clone()).unwrap();
        b_control.play(ramp.clone()).unwrap();
        let mut actual = [0f32; 4096];
        let mut expected = [0f32; 4096];
        assert_no_alloc(|| {
            for _ in 0..24 {
                a_renderer.render(&mut expected, 2);
                b_renderer.render(&mut actual, 2);
                assert_eq!(actual, expected);
            }
        });
        // The explicit level reached at the end also matches ordinary playback.
        let (mut a_control, mut a_renderer) = Renderer::new(48_000).unwrap();
        let (mut b_control, mut b_renderer) = Renderer::new(48_000).unwrap();
        a_control.play_from(plain, b).unwrap();
        b_control.play_from(ramp, b).unwrap();
        assert_no_alloc(|| {
            a_renderer.render(&mut expected, 2);
            b_renderer.render(&mut actual, 2);
            assert_eq!(actual, expected);
        });
    }

    #[test]
    fn sustained_hairpins_change_audio_gain_and_seek_pause_resume_without_allocating() {
        use score_core::{Connection, ConnectionKind, Dynamic, Instrument, Pitches};
        let mut score = Score::default();
        let flute = score.add_part(Instrument::Flute).unwrap();
        let chord = Event {
            pitches: Pitches::new(&[
                Pitch::new(Step::A, 4).unwrap(),
                Pitch::new(Step::C, 5).unwrap(),
            ])
            .unwrap(),
            duration: Duration::Whole,
            notation: Default::default(),
        };
        let a = score.insert_in(flute, 0, 0, chord).unwrap();
        let b = score.insert_in(flute, 1, 0, chord).unwrap();
        score.set_dynamic(a, Some(Dynamic::P)).unwrap();
        score.set_dynamic(b, Some(Dynamic::F)).unwrap();
        score
            .add_connection(Connection {
                start: a,
                end: b,
                kind: ConnectionKind::Tie,
            })
            .unwrap();
        score
            .add_connection(Connection {
                start: a,
                end: b,
                kind: ConnectionKind::Crescendo,
            })
            .unwrap();
        let mut flat = score.clone();
        flat.set_dynamic(b, Some(Dynamic::P)).unwrap();
        let ramp = Arc::new(PlaybackPlan::new(&score, Tempo::default(), 48_000).unwrap());
        let flat_plan = Arc::new(PlaybackPlan::new(&flat, Tempo::default(), 48_000).unwrap());
        let (mut control, mut renderer) = Renderer::new(48_000).unwrap();
        let (mut flat_control, mut flat_renderer) = Renderer::new(48_000).unwrap();
        control.play(ramp.clone()).unwrap();
        flat_control.play(flat_plan).unwrap();
        let mut actual = [0f32; 2048];
        let mut expected = [0f32; 2048];
        let mut late_ramp = 0f64;
        let mut late_flat = 0f64;
        assert_no_alloc(|| {
            for block in 0..90 {
                renderer.render(&mut actual, 2);
                flat_renderer.render(&mut expected, 2);
                if block > 70 {
                    late_ramp += actual.iter().map(|s| f64::from(*s).powi(2)).sum::<f64>();
                    late_flat += expected.iter().map(|s| f64::from(*s).powi(2)).sum::<f64>();
                }
            }
            assert!(late_flat > 0.001);
            assert!(
                late_ramp > late_flat * 4.,
                "A tied chord must grow louder without another note-on"
            );
            control.pause();
            renderer.render(&mut actual, 2);
            assert!(actual.iter().all(|s| *s == 0.));
        });
        // Starting at a tied continuation chases both the held chord and final expression.
        flat.set_dynamic(a, Some(Dynamic::F)).unwrap();
        flat.set_dynamic(b, Some(Dynamic::F)).unwrap();
        let loud = Arc::new(PlaybackPlan::new(&flat, Tempo::default(), 48_000).unwrap());
        let (mut seek_control, mut seek_renderer) = Renderer::new(48_000).unwrap();
        let (mut ref_control, mut reference) = Renderer::new(48_000).unwrap();
        seek_control.play_from(ramp.clone(), b).unwrap();
        ref_control.play(loud).unwrap();
        assert_no_alloc(|| {
            seek_renderer.render(&mut actual, 2);
            reference.render(&mut expected, 2);
            assert_eq!(actual, expected);
            seek_control.pause();
            seek_renderer.render(&mut actual, 2);
            assert!(actual.iter().all(|s| *s == 0.));
            seek_control.resume();
            seek_renderer.render(&mut actual, 2);
            reference.render(&mut expected, 2);
            assert_eq!(actual, expected);
        });
        // A diminuendo acts on the same held voices in the opposite direction.
        let crescendo = *score
            .connections()
            .iter()
            .find(|c| c.kind.is_hairpin())
            .unwrap();
        score.remove_connection(crescendo);
        score
            .add_connection(Connection {
                kind: ConnectionKind::Diminuendo,
                ..crescendo
            })
            .unwrap();
        score.set_dynamic(a, Some(Dynamic::F)).unwrap();
        score.set_dynamic(b, Some(Dynamic::P)).unwrap();
        let dim = Arc::new(PlaybackPlan::new(&score, Tempo::default(), 48_000).unwrap());
        let (mut dim_control, mut dim_renderer) = Renderer::new(48_000).unwrap();
        dim_control.play(dim).unwrap();
        let mut constant = score.clone();
        constant.set_dynamic(b, Some(Dynamic::F)).unwrap();
        let (mut constant_control, mut constant_renderer) = Renderer::new(48_000).unwrap();
        constant_control
            .play(Arc::new(
                PlaybackPlan::new(&constant, Tempo::default(), 48_000).unwrap(),
            ))
            .unwrap();
        let mut dim_energy = 0f64;
        let mut constant_energy = 0f64;
        assert_no_alloc(|| {
            for block in 0..90 {
                dim_renderer.render(&mut actual, 2);
                constant_renderer.render(&mut expected, 2);
                if block > 70 {
                    dim_energy += actual.iter().map(|s| f64::from(*s).powi(2)).sum::<f64>();
                    constant_energy += expected.iter().map(|s| f64::from(*s).powi(2)).sum::<f64>();
                }
            }
        });
        assert!(constant_energy > dim_energy * 4.);
    }

    #[test]
    fn pizzicato_uses_real_bank_sound_seeks_and_resumes_without_callback_allocations() {
        use score_core::{Instrument, PlayingTechnique as T};
        let bank = sound_font().unwrap();
        assert!(
            bank.get_presets()
                .iter()
                .any(|p| p.get_bank_number() == 0 && p.get_patch_number() == 45)
        );
        let mut score = Score::default();
        let part = score.add_part(Instrument::Violin).unwrap();
        let mut ids = Vec::new();
        for i in 0..3 {
            ids.push(
                score
                    .insert_in(
                        part,
                        0,
                        i,
                        Event {
                            pitches: score_core::Pitches::single(Pitch::new(Step::A, 4)),
                            duration: Duration::Quarter,
                            notation: Default::default(),
                        },
                    )
                    .unwrap(),
            );
        }
        score.set_technique(ids[0], Some(T::Pizzicato)).unwrap();
        score.set_technique(ids[2], Some(T::Arco)).unwrap();
        let plan = Arc::new(PlaybackPlan::new(&score, Tempo::default(), 48_000).unwrap());
        let render_start = |id| {
            let (mut control, mut renderer) = Renderer::new(48_000).unwrap();
            control.play_from(plan.clone(), id).unwrap();
            let mut output = [0f32; 4096];
            assert_no_alloc(|| renderer.render(&mut output, 2));
            assert!(output.iter().map(|v| v * v).sum::<f32>() > 0.001);
            (control, renderer, output)
        };
        let (_, _, pizz) = render_start(ids[0]);
        let (mut control, mut renderer, inherited) = render_start(ids[1]);
        assert_eq!(
            pizz, inherited,
            "Starting after the pizzicato mark must chase its sound"
        );
        let (_, _, arco) = render_start(ids[2]);
        assert_ne!(pizz, arco, "Arco must restore a different, bowed timbre");
        let (_, mut reference, _) = render_start(ids[1]);
        let mut actual = [0f32; 4096];
        let mut expected = [0f32; 4096];
        assert_no_alloc(|| {
            control.pause();
            renderer.render(&mut actual, 2);
            assert!(actual.iter().all(|v| *v == 0.));
            control.resume();
            renderer.render(&mut actual, 2);
            reference.render(&mut expected, 2);
            assert_eq!(actual, expected);
        });
    }

    #[test]
    fn every_catalog_instrument_has_a_bank_preset_and_a_distinct_audition() {
        let bank = sound_font().unwrap();
        let mut previous = [0f32; 4096];
        for instrument in score_core::Instrument::ALL {
            assert!(
                bank.get_presets()
                    .iter()
                    .any(|preset| preset.get_bank_number() == 0
                        && preset.get_patch_number() == i32::from(instrument.program()))
            );
            let (mut control, mut renderer) = Renderer::new(48_000).unwrap();
            control
                .preview_with_program(69, instrument.program())
                .unwrap();
            let mut output = [0f32; 4096];
            assert_no_alloc(|| renderer.render(&mut output, 2));
            assert!(
                output.iter().map(|sample| sample * sample).sum::<f32>() > 0.001,
                "{} must be audible",
                instrument.name()
            );
            assert_ne!(
                output,
                previous,
                "{} must change the timbre",
                instrument.name()
            );
            previous = output;
        }
    }

    #[test]
    fn real_piano_renders_and_finishes_without_callback_allocations() {
        let (mut control, mut renderer) = Renderer::new(48_000).unwrap();
        control.play(plan()).unwrap();
        let mut output = [0f32; 1024];
        let mut energy = 0f64;
        assert_no_alloc(|| {
            for _ in 0..120 {
                renderer.render(&mut output, 2);
                for sample in output {
                    assert!(sample.is_finite() && sample.abs() <= 1.);
                    energy += f64::from(sample * sample);
                }
            }
        });
        assert!(
            energy > 1.,
            "The bundled piano must produce audible samples"
        );
        assert!(!control.is_playing());
        assert!(control.position().is_none());
        assert!(control.retained.is_empty()); // Final destruction ran on control thread.
    }

    #[test]
    fn stop_invalidates_a_full_queue_and_superseded_plans_are_freed_off_callback() {
        let (mut control, mut renderer) = Renderer::new(48_000).unwrap();
        control.play(plan()).unwrap();
        control.play(plan()).unwrap();
        for _ in 0..QUEUE_CAPACITY - 2 {
            control.preview(69).unwrap();
        }
        assert!(control.preview(69).is_err());
        control.stop();
        let mut output = [0f32; 512];
        assert_no_alloc(|| renderer.render(&mut output, 2));
        assert!(output.iter().all(|sample| *sample == 0.));
        control.position();
        assert!(control.retained.is_empty());
        control.preview(69).unwrap();
        assert_no_alloc(|| renderer.render(&mut output, 2));
        control.stop();
        assert_no_alloc(|| {
            renderer.render(&mut output, 2);
            renderer.render(&mut output, 2);
        });
        assert!(output.iter().all(|sample| sample.abs() < 1e-7));
    }

    #[test]
    fn concurrent_starts_and_previews_cannot_survive_the_final_stop() {
        let (mut control, mut renderer) = Renderer::new(48_000).unwrap();
        let plan = plan();
        let done = Arc::new(AtomicBool::new(false));
        let producer_done = done.clone();
        let producer = std::thread::spawn(move || {
            for _ in 0..1000 {
                let _ = control.play(plan.clone());
                let _ = control.preview(69);
                control.pause();
                std::thread::yield_now();
                control.resume();
                control.stop();
            }
            producer_done.store(true, Ordering::Release);
            control
        });
        let mut output = [0f32; 256];
        while !done.load(Ordering::Acquire) {
            assert_no_alloc(|| renderer.render(&mut output, 2));
            std::thread::yield_now();
        }
        let mut control = producer.join().unwrap();
        assert_no_alloc(|| {
            renderer.render(&mut output, 2);
            renderer.render(&mut output, 2);
        });
        assert!(renderer.active.is_none());
        assert!(renderer.preview_off.is_none());
        assert!(output.iter().all(|sample| sample.abs() < 1e-7));
        assert!(!control.is_playing());
        control.position();
        assert!(control.retained.is_empty());
    }

    #[test]
    fn pause_freezes_time_and_resumes_the_exact_waveform_without_allocations() {
        let (mut reference_control, mut reference) = Renderer::new(48_000).unwrap();
        let (mut control, mut renderer) = Renderer::new(48_000).unwrap();
        reference_control.play(plan()).unwrap();
        control.play(plan()).unwrap();
        let mut expected = [0f32; 1024];
        let mut actual = [0f32; 1024];
        assert_no_alloc(|| {
            reference.render(&mut expected, 2);
            renderer.render(&mut actual, 2);
        });
        assert_eq!(actual, expected);
        let frame = control.position().unwrap().frame;
        control.pause();
        assert!(control.is_paused());
        assert!(!control.is_playing());
        assert_no_alloc(|| {
            for _ in 0..20 {
                renderer.render(&mut actual, 2);
            }
        });
        assert!(actual.iter().all(|sample| *sample == 0.));
        assert_eq!(control.position().unwrap().frame, frame);
        control.resume();
        assert_no_alloc(|| {
            reference.render(&mut expected, 2);
            renderer.render(&mut actual, 2);
        });
        assert_eq!(
            actual, expected,
            "Resume must preserve sample phase and envelope, not restart the note"
        );
        assert!(control.is_playing());
        assert_eq!(control.position().unwrap().frame, frame + 512);
    }

    #[test]
    fn selection_starts_at_its_onset_and_queued_pause_and_stop_preserve_invariants() {
        let (mut control, mut renderer) = Renderer::new(48_000).unwrap();
        let plan = plan();
        let selected = plan.spans[1].element;
        control.play_from(plan, selected).unwrap();
        assert_eq!(control.position().unwrap().frame, 24_000);
        control.pause(); // Pause before the first audio callback has consumed Play.
        let mut output = [0f32; 1024];
        assert_no_alloc(|| renderer.render(&mut output, 2));
        assert!(output.iter().all(|sample| *sample == 0.));
        assert_eq!(renderer.frame, 24_000);
        assert_eq!(control.position().unwrap().element, Some(selected));
        control.resume();
        assert_no_alloc(|| renderer.render(&mut output, 2));
        assert!(output.iter().any(|sample| sample.abs() > 0.001));
        assert_eq!(renderer.frame, 24_512);
        control.pause();
        control.stop();
        control.resume(); // Stop invalidates the suspended session permanently.
        assert_no_alloc(|| {
            renderer.render(&mut output, 2);
            renderer.render(&mut output, 2);
        });
        assert!(!control.is_paused());
        assert!(!control.is_playing());
        assert!(renderer.active.is_none());
        assert!(control.position().is_none());
        assert!(output.iter().all(|sample| sample.abs() < 1e-7));
    }

    #[test]
    fn frame_seek_chases_held_notes_replaces_pending_starts_and_preserves_pause() {
        let (mut control, mut renderer) = Renderer::new(48_000).unwrap();
        let plan = plan();
        control.play(plan.clone()).unwrap();
        control.play_from_frame(plan.clone(), 12_000).unwrap();
        control.pause();
        let mut output = [0f32; 2048];
        assert_no_alloc(|| renderer.render(&mut output, 2));
        assert_eq!(control.position().unwrap().frame, 12_000);
        assert!(output.iter().all(|sample| *sample == 0.));
        assert!(control.is_paused());
        // No separate pause() is needed after a seek. A callback immediately
        // after publishing it must already see a paused session.
        control.play_from_frame(plan.clone(), 18_000).unwrap();
        assert!(control.is_paused());
        assert_no_alloc(|| renderer.render(&mut output, 2));
        assert_eq!(control.position().unwrap().frame, 18_000);
        assert!(output.iter().all(|sample| *sample == 0.));
        control.resume();
        assert_no_alloc(|| renderer.render(&mut output, 2));
        assert_eq!(control.position().unwrap().frame, 19_024);
        assert!(output.iter().any(|sample| sample.abs() > 0.001));
        // Seeking while playing immediately replaces the earlier position.
        control.play_from_frame(plan.clone(), 30_000).unwrap();
        assert_eq!(control.position().unwrap().frame, 30_000);
        assert!(control.is_playing());
        assert_no_alloc(|| renderer.render(&mut output, 2));
        assert_eq!(control.position().unwrap().frame, 31_024);
        // Invalid endpoints leave the current session untouched.
        assert!(
            control
                .play_from_frame(plan.clone(), plan.duration_frames)
                .is_err()
        );
        assert_eq!(control.position().unwrap().frame, 31_024);
    }

    #[test]
    fn frame_seek_into_implicit_rest_keeps_exact_position_and_silence() {
        let (mut control, mut renderer) = Renderer::new(48_000).unwrap();
        let mut extended = Arc::try_unwrap(plan()).unwrap();
        extended.duration_frames = 192_000;
        control
            .play_from_frame(Arc::new(extended), 120_000)
            .unwrap();
        assert_eq!(control.position().unwrap().element, None);
        let mut output = [0f32; 1024];
        assert_no_alloc(|| renderer.render(&mut output, 2));
        assert_eq!(control.position().unwrap().frame, 120_512);
        assert!(output.iter().all(|sample| *sample == 0.));
        assert!(control.is_playing());
    }

    #[test]
    fn selected_rest_keeps_silence_and_pause_does_not_skip_its_remaining_duration() {
        let mut score = Score::default();
        let rest = score
            .insert(
                0,
                0,
                Event {
                    notation: Default::default(),
                    pitches: score_core::Pitches::single(None),
                    duration: Duration::Quarter,
                },
            )
            .unwrap();
        let note = score
            .insert(
                0,
                1,
                Event {
                    notation: Default::default(),
                    pitches: score_core::Pitches::single(Pitch::new(Step::C, 4)),
                    duration: Duration::Quarter,
                },
            )
            .unwrap();
        let plan = Arc::new(PlaybackPlan::new(&score, Tempo::default(), 48_000).unwrap());
        let (mut control, mut renderer) = Renderer::new(48_000).unwrap();
        control.play_from(plan, rest).unwrap();
        let mut output = [0f32; 1024];
        assert_no_alloc(|| renderer.render(&mut output, 2));
        assert!(output.iter().all(|sample| *sample == 0.));
        control.pause();
        assert_no_alloc(|| {
            for _ in 0..100 {
                renderer.render(&mut output, 2);
            }
        });
        assert_eq!(control.position().unwrap().frame, 512);
        control.resume();
        assert_no_alloc(|| {
            for _ in 0..46 {
                renderer.render(&mut output, 2);
            }
        });
        assert_eq!(control.position().unwrap().element, Some(note));
        assert_no_alloc(|| {
            for _ in 0..48 {
                renderer.render(&mut output, 2);
            }
        });
        assert!(!control.is_playing());
    }

    #[test]
    fn preview_releases_automatically_and_output_supports_mono_and_integer_formats() {
        let (mut control, mut renderer) = Renderer::new(48_000).unwrap();
        control.preview(60).unwrap();
        let mut mono = [0i16; 512];
        assert_no_alloc(|| renderer.render(&mut mono, 1));
        assert!(mono.iter().any(|sample| *sample != 0));
        let mut surround = [0f32; 1536];
        assert_no_alloc(|| {
            for _ in 0..200 {
                renderer.render(&mut surround, 6);
            }
        });
        assert!(renderer.preview_off.is_none());
        assert!(
            surround
                .as_chunks::<6>()
                .0
                .iter()
                .all(|frame| frame[2..].iter().all(|sample| *sample == 0.))
        );
    }
}
