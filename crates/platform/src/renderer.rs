use crate::mixer::{ChannelMix, ReverbSettings};
use crate::sound_engine::{Engines, SoundBank, SoundEngine};
use crate::{AudioError, error};
use cpal::{FromSample, Sample};
use rtrb::{Consumer, Producer, RingBuffer};
use score_playback::{Message, PlaybackPlan};
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering},
};

const QUEUE_CAPACITY: usize = 64;
const BLOCK: usize = 64;
/// Changes of pace (start, pause, resume, loop wrap, end) remembered to tell
/// which frame the listener hears. A few per second at most.
const HISTORY: usize = 64;

struct Shared {
    epoch: AtomicU64,
    started: AtomicU64,
    finished: AtomicU64,
    /// The frame the listener hears now: the rendered frame less the
    /// output latency.
    frame: AtomicU64,
    volume: AtomicU32,
    failed: Arc<AtomicBool>,
    paused: AtomicBool,
    mix: [AtomicU32; score_playback::PLAYBACK_CHANNELS],
    reverb: AtomicU32,
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
        audition: score_playback::Audition,
    },
    Loop {
        epoch: u64,
        looping: Option<Loop>,
    },
}

/// A passage played again and again. At `end` the callback releases what
/// sounds and resumes at `start` with the notes and levels sounding there,
/// prepared here so the wrap needs no plan search beyond one bisection.
#[derive(Clone, Copy)]
struct Loop {
    start: u64,
    end: u64,
    held_notes: [Option<score_playback::HeldNote>; score_core::MAX_SOUNDING_NOTES],
    expressions: [u8; score_playback::PLAYBACK_CHANNELS],
}

impl Loop {
    fn new(plan: &PlaybackPlan, start: u64, end: u64) -> Option<Self> {
        let end = end.min(plan.duration_frames);
        (start < end).then(|| Self {
            start,
            end,
            held_notes: plan.held_notes_at(start),
            expressions: plan.expressions_at(start),
        })
    }
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
    pub elements: [Option<score_core::ElementId>; score_core::MAX_STAVES * 2],
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

    /// Play frames `start..end` of the current plan again and again, or
    /// play on with `None`. A new start clears the loop.
    pub fn set_loop(&mut self, region: Option<(u64, u64)>) -> Result<(), AudioError> {
        let Some(active) = &self.active else {
            return Ok(());
        };
        let looping = region.and_then(|(start, end)| Loop::new(&active.plan, start, end));
        self.commands
            .push(Command::Loop {
                epoch: active.epoch,
                looping,
            })
            .map_err(|_| error("La cola de audio está ocupada; inténtalo de nuevo"))
    }

    pub fn preview(&mut self, key: u8) -> Result<(), AudioError> {
        self.preview_instrument(key, score_core::Instrument::Piano)
    }
    pub fn preview_instrument(
        &mut self,
        key: u8,
        instrument: score_core::Instrument,
    ) -> Result<(), AudioError> {
        let mut keys = [None; score_core::MAX_CHORD_NOTES];
        keys[0] = Some(key);
        self.preview_chord(keys, instrument)
    }
    pub fn preview_chord(
        &mut self,
        keys: [Option<u8>; score_core::MAX_CHORD_NOTES],
        instrument: score_core::Instrument,
    ) -> Result<(), AudioError> {
        self.preview_audition(score_playback::Audition::plain(
            keys,
            instrument,
            self.sample_rate,
        ))
    }
    pub fn preview_audition(
        &mut self,
        audition: score_playback::Audition,
    ) -> Result<(), AudioError> {
        let keys = audition.keys;
        if audition.sample_rate != self.sample_rate
            || audition.duration_frames == 0
            || audition.duration_frames > u64::from(self.sample_rate) * 2
            || !(1..=127).contains(&audition.level)
            || keys.iter().flatten().any(|key| *key > 127)
            || keys.iter().all(Option::is_none)
            || (0..keys.len()).any(|index| {
                keys[index].is_some()
                    && (audition.delays[index] >= audition.gate_at(index)
                        || audition.gate_at(index) > u64::from(self.sample_rate) * 2
                        || audition.note_at(index).key != keys[index].unwrap()
                        || !(1..=127).contains(&audition.note_at(index).level))
            })
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
                audition,
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
    /// Live controls are bounded atomic snapshots: they cannot fill the event queue.
    pub fn set_channel_mix(&mut self, channel: u8, mix: ChannelMix) {
        if let Some(value) = self.shared.mix.get(usize::from(channel)) {
            value.store(mix.packed(), Ordering::Relaxed);
        }
    }
    pub fn set_reverb(&mut self, settings: ReverbSettings) {
        self.shared
            .reverb
            .store(settings.packed(), Ordering::Relaxed);
    }
    fn reclaim(&mut self) {
        self.retained.retain(|plan| Arc::strong_count(plan) > 1);
    }
}

/// Where the frame stood at a sample clock, and whether it moved with it.
#[derive(Clone, Copy)]
struct Pace {
    clock: u64,
    frame: u64,
    moving: bool,
}

pub struct Renderer {
    commands: Consumer<Command>,
    shared: Arc<Shared>,
    engines: Engines,
    active: Option<Active>,
    epoch: u64,
    frame: u64,
    sample_rate: u32,
    /// Frames between rendering a sample and the listener hearing it.
    latency: u64,
    /// Ring of the latest changes of pace since the current start.
    history: [Pace; HISTORY],
    history_len: usize,
    history_next: usize,
    /// The sample clock at which the plan ended, until the listener hears it.
    ending: Option<u64>,
    next_message: usize,
    clock: u64,
    preview_off: Option<u64>,
    /// Fixed storage: no pending attack can allocate in the audio callback.
    preview: Option<(score_playback::Audition, u64)>,
    preview_attacked: [bool; score_playback::MAX_AUDITION_NOTES],
    looping: Option<Loop>,
    left: [f32; BLOCK],
    right: [f32; BLOCK],
    mix: [u32; score_playback::PLAYBACK_CHANNELS],
    reverb: crate::reverb::Reverb,
}

impl Renderer {
    /// Also usable offline, to exercise the exact callback without audio hardware.
    pub fn new(sample_rate: u32) -> Result<(Control, Self), AudioError> {
        if !(16_000..=192_000).contains(&sample_rate) {
            return Err(error("Frecuencia de audio no compatible"));
        }
        let engines = Engines::new(sample_rate, None)?;
        let (producer, consumer) = RingBuffer::new(QUEUE_CAPACITY);
        let shared = Arc::new(Shared {
            epoch: AtomicU64::new(1),
            started: AtomicU64::new(0),
            finished: AtomicU64::new(0),
            frame: AtomicU64::new(0),
            volume: AtomicU32::new(70),
            failed: Arc::new(AtomicBool::new(false)),
            paused: AtomicBool::new(false),
            mix: std::array::from_fn(|_| AtomicU32::new(ChannelMix::default().packed())),
            reverb: AtomicU32::new(ReverbSettings::default().packed()),
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
            engines,
            active: None,
            epoch: 1,
            frame: 0,
            sample_rate,
            latency: 0,
            history: [Pace {
                clock: 0,
                frame: 0,
                moving: false,
            }; HISTORY],
            history_len: 0,
            history_next: 0,
            ending: None,
            next_message: 0,
            clock: 0,
            preview_off: None,
            preview: None,
            preview_attacked: [false; score_playback::MAX_AUDITION_NOTES],
            looping: None,
            left: [0.; BLOCK],
            right: [0.; BLOCK],
            mix: [u32::MAX; score_playback::PLAYBACK_CHANNELS],
            reverb: crate::reverb::Reverb::new(sample_rate),
        };
        Ok((control, renderer))
    }

    pub fn with_bank(
        sample_rate: u32,
        bank: &dyn SoundBank,
        score: &score_core::Score,
        audition: score_core::Instrument,
    ) -> Result<(Control, Self), AudioError> {
        let (control, mut renderer) = Self::new(sample_rate)?;
        renderer.engines = Engines::new(
            sample_rate,
            Some(bank.prepare(score, audition, sample_rate)?),
        )?;
        Ok((control, renderer))
    }

    pub(crate) fn failure_flag(&self) -> Arc<AtomicBool> {
        self.shared.failed.clone()
    }
    /// The time from rendering a sample to hearing it, as the device reports
    /// it each callback. The position published to the interface lags the
    /// rendered one by this much. Capped at one second against nonsense.
    pub fn set_output_latency(&mut self, latency: std::time::Duration) {
        let rate = u128::from(self.sample_rate);
        let frames = latency.as_nanos().saturating_mul(rate) / 1_000_000_000;
        self.latency = frames.min(rate) as u64;
    }
    /// For offline exports only: permits native sample streaming to wait for disk.
    /// The live CPAL stream never enables this mode.
    pub fn offline(&mut self) {
        self.engines.offline();
    }
    fn midi(&mut self, channel: u8, status: u8, a: u8, b: u8) {
        self.engines.midi(channel, status, a, b);
    }
    fn note_on(&mut self, channel: u8, note: score_playback::NoteEvent) {
        self.engines.note_on(channel, note);
    }
    fn note_off(&mut self, channel: u8, key: u8) {
        self.engines.note_off(channel, key);
    }

    fn check_stop(&mut self) {
        let epoch = self.shared.epoch.load(Ordering::Acquire);
        if epoch != self.epoch {
            self.reverb.clear();
            self.engines.silence(None, true);
            self.active = None;
            self.preview_off = None;
            self.preview = None;
            self.frame = 0;
            self.next_message = 0;
            self.epoch = epoch;
            self.history_len = 0;
            self.ending = None;
        }
    }

    /// Remember the pace from the current clock on, when it changed.
    fn pace(&mut self, moving: bool) {
        if self.history_len > 0 {
            let last = self.history[(self.history_next + HISTORY - 1) % HISTORY];
            let expected = if last.moving {
                last.frame + (self.clock - last.clock)
            } else {
                last.frame
            };
            if last.moving == moving && expected == self.frame {
                return;
            }
        }
        self.history[self.history_next] = Pace {
            clock: self.clock,
            frame: self.frame,
            moving,
        };
        self.history_next = (self.history_next + 1) % HISTORY;
        self.history_len = (self.history_len + 1).min(HISTORY);
    }

    /// The frame that sounded at sample clock `clock`. Before the start, the
    /// start itself: the previous playback's tail is not shown.
    fn frame_at_clock(&self, clock: u64) -> u64 {
        let mut oldest = self.frame;
        for back in 1..=self.history_len {
            let pace = self.history[(self.history_next + HISTORY - back) % HISTORY];
            if pace.clock <= clock {
                return if pace.moving {
                    pace.frame + (clock - pace.clock)
                } else {
                    pace.frame
                };
            }
            oldest = pace.frame;
        }
        oldest
    }

    fn update_mix(&mut self) {
        for channel in 0..score_playback::PLAYBACK_CHANNELS {
            let packed = self.shared.mix[channel].load(Ordering::Relaxed);
            if self.mix[channel] != packed {
                let mix = ChannelMix::unpack(packed);
                self.midi(channel as u8, 0xb0, 7, mix.volume);
                self.midi(channel as u8, 0xb0, 10, mix.midi_pan());
                self.mix[channel] = packed;
            }
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
                    self.engines.silence(None, true);
                    for (channel, value) in expressions.into_iter().enumerate() {
                        self.midi(channel as u8, 0xb0, 11, value);
                    }
                    // Chase both the original attack and current expression level.
                    for held in held_notes.into_iter().flatten() {
                        self.note_on(held.channel, held.note);
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
                    self.looping = None;
                    self.preview_off = None;
                    self.preview = None;
                    self.history_len = 0;
                    self.ending = None;
                    self.shared.frame.store(start_frame, Ordering::Release);
                    self.shared.started.store(epoch, Ordering::Release);
                }
                Command::Loop { epoch, looping } if epoch == self.epoch => {
                    self.looping = looping;
                }
                Command::Preview { epoch, audition } if epoch == self.epoch => {
                    self.engines.silence(Some(15), true);
                    self.midi(15, 0xb0, 11, 127);
                    self.preview = Some((audition, self.clock));
                    self.preview_attacked.fill(false);
                    self.preview_off = Some(self.clock.saturating_add(audition.end_frame()));
                }
                _ => {} // Old commands are invalidated by Stop; owners stay in Control.
            }
        }
    }

    /// Back to the loop's start once its end is reached, before any message
    /// scheduled at the end itself can sound.
    fn wrap_loop(&mut self) {
        let Some(active) = &self.active else {
            return;
        };
        if self
            .looping
            .as_ref()
            .is_none_or(|looping| self.frame < looping.end)
        {
            return;
        }
        let looping = self.looping.expect("checked loop");
        self.next_message = active
            .plan
            .messages
            .partition_point(|message| message.frame < looping.start);
        self.engines.silence(None, false);
        for (channel, value) in looping.expressions.into_iter().enumerate() {
            self.midi(channel as u8, 0xb0, 11, value);
        }
        for held in looping.held_notes.into_iter().flatten() {
            self.note_on(held.channel, held.note);
        }
        self.frame = looping.start;
    }

    fn due_messages(&mut self) {
        self.wrap_loop();
        while let Some(message) = self
            .active
            .as_ref()
            .and_then(|active| active.plan.messages.get(self.next_message))
            .copied()
        {
            if message.frame > self.frame {
                break;
            }
            match message.message {
                Message::Expression(value) => self.midi(message.channel, 0xb0, 11, value),
                Message::NoteOn(note) => self.note_on(message.channel, note),
                Message::NoteOff(key) => self.note_off(message.channel, key),
                Message::HarmonicOff(key) => self.engines.harmonic_off(message.channel, key),
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
            // The end is announced once it is heard, in `render`.
            self.active = None;
            self.ending = Some(self.clock);
        }
        if let Some((mut audition, start)) = self.preview {
            for index in 0..audition.keys.len() {
                if let Some(key) = audition.keys[index] {
                    if !self.preview_attacked[index]
                        && start.saturating_add(audition.delays[index]) <= self.clock
                    {
                        self.note_on(15, audition.note_at(index));
                        self.preview_attacked[index] = true;
                    }
                    if self.preview_attacked[index]
                        && start.saturating_add(audition.gate_at(index)) <= self.clock
                    {
                        if audition.note_at(index).harmonic.is_some() {
                            self.engines.harmonic_off(15, key);
                        } else {
                            self.note_off(15, key);
                        }
                        audition.keys[index] = None;
                    }
                }
            }
            self.preview = Some((audition, start));
        }
        if self.preview_off.is_some_and(|off| self.clock >= off) {
            self.engines.silence(Some(15), false);
            self.preview_off = None;
            self.preview = None;
        }
    }

    /// No locks, allocation, disk I/O, formatting or UI calls. Output format
    /// conversion and stereo/mono routing use preallocated stack buffers.
    pub fn render<T: Sample + FromSample<f32>>(&mut self, output: &mut [T], channels: usize) {
        if channels == 0 {
            return;
        }
        self.commands();
        self.update_mix();
        let total_frames = output.len() / channels;
        let mut written = 0;
        while written < total_frames {
            self.check_stop();
            if self.active.is_some() && self.shared.paused.load(Ordering::Acquire) {
                // Freeze the sample clock AND the synthesizer's envelopes,
                // oscillator phase and cached block. Resume continues this
                // very note; no new attack or callback allocations are needed.
                self.pace(false);
                let count = (total_frames - written).min(BLOCK);
                for sample in &mut output[written * channels..(written + count) * channels] {
                    *sample = T::from_sample(0.);
                }
                self.clock += count as u64;
                written += count;
                continue;
            }
            self.due_messages();
            self.pace(self.active.is_some());
            let mut count = (total_frames - written).min(BLOCK);
            if let Some(active) = &self.active {
                let next = active
                    .plan
                    .messages
                    .get(self.next_message)
                    .map(|message| message.frame)
                    .unwrap_or(active.plan.duration_frames);
                count = count.min((next - self.frame) as usize);
                if let Some(looping) = &self.looping {
                    count = count.min(looping.end.saturating_sub(self.frame) as usize);
                }
            }
            if let Some(off) = self.preview_off {
                count = count.min((off - self.clock) as usize);
            }
            if let Some((audition, start)) = &self.preview {
                for index in 0..audition.keys.len() {
                    if audition.keys[index].is_some() {
                        let boundary = if self.preview_attacked[index] {
                            audition.gate_at(index)
                        } else {
                            audition.delays[index]
                        };
                        count = count.min((start.saturating_add(boundary) - self.clock) as usize);
                    }
                }
            }
            // due_messages consumed all boundaries at the current frame.
            debug_assert!(count > 0);
            self.left[..count].fill(0.);
            self.right[..count].fill(0.);
            self.engines
                .render(&mut self.left[..count], &mut self.right[..count]);
            self.reverb.process(
                &mut self.left[..count],
                &mut self.right[..count],
                self.shared.reverb.load(Ordering::Relaxed),
            );
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
        let paused = self.shared.paused.load(Ordering::Acquire);
        if !paused {
            self.due_messages();
        }
        self.pace(self.active.is_some() && !paused);
        // Heard by the end of this buffer's period: what was rendered
        // `latency` frames of clock ago.
        let heard = self.clock.saturating_sub(self.latency);
        self.shared
            .frame
            .store(self.frame_at_clock(heard), Ordering::Release);
        if self.ending.is_some_and(|end| end <= heard) {
            self.ending = None;
            self.shared.finished.store(self.epoch, Ordering::Release);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sound_engine::sound_font;
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
        control
            .preview_chord(keys, score_core::Instrument::Piano)
            .unwrap();
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
            && matches!(m.message, Message::NoteOn(n) if (n.key, n.level) == (69, 56))));
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
    fn every_catalog_instrument_has_a_bank_preset_and_an_audible_audition() {
        let bank = sound_font().unwrap();
        for instrument in score_core::Instrument::ALL {
            assert!(
                bank.get_presets()
                    .iter()
                    .any(|preset| preset.get_bank_number()
                        == if instrument.percussion_key().is_some() {
                            128
                        } else {
                            0
                        }
                        && preset.get_patch_number() == i32::from(instrument.program()))
            );
            let (mut control, mut renderer) = Renderer::new(48_000).unwrap();
            control.preview_instrument(69, instrument).unwrap();
            let mut output = [0f32; 4096];
            assert_no_alloc(|| renderer.render(&mut output, 2));
            assert!(
                output.iter().map(|sample| sample * sample).sum::<f32>() > 0.001,
                "{} must be audible",
                instrument.name()
            );
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
    fn a_loop_wraps_exactly_at_its_end_without_allocating_and_can_be_lifted() {
        let (mut control, mut renderer) = Renderer::new(48_000).unwrap();
        let plan = plan();
        // Without playback there is nothing to loop yet.
        control.set_loop(Some((0, 24_000))).unwrap();
        control.play(plan.clone()).unwrap();
        control.set_loop(Some((0, 24_000))).unwrap();
        let mut output = [0f32; 2048];
        assert_no_alloc(|| {
            for _ in 0..30 {
                renderer.render(&mut output, 2);
            }
        });
        // 30 blocks of 1024 frames, wrapped once at the first quarter's end.
        let position = control.position().unwrap();
        assert_eq!(position.frame, 30 * 1024 - 24_000);
        assert_eq!(position.element, plan.element_at(0));
        assert!(output.iter().any(|sample| sample.abs() > 0.001));
        assert!(control.is_playing());

        control.set_loop(None).unwrap();
        assert_no_alloc(|| {
            for _ in 0..42 {
                renderer.render(&mut output, 2);
            }
        });
        assert!(!control.is_playing());
    }

    #[test]
    fn the_position_follows_what_is_heard_through_the_output_latency() {
        const LATENCY: u64 = 4_800; // 100 ms at 48 kHz.
        let (mut control, mut renderer) = Renderer::new(48_000).unwrap();
        let plan = plan();
        let mut output = [0f32; 2048];
        let mut render = |renderer: &mut Renderer, blocks: usize| {
            assert_no_alloc(|| {
                for _ in 0..blocks {
                    renderer.set_output_latency(std::time::Duration::from_millis(100));
                    renderer.render(&mut output, 2);
                }
            })
        };
        control.play_from_frame(plan.clone(), 12_000).unwrap();
        // Nothing of this playback is heard yet: the start, not the tail
        // of what played before.
        render(&mut renderer, 2);
        assert_eq!(control.position().unwrap().frame, 12_000);
        render(&mut renderer, 8);
        assert_eq!(
            control.position().unwrap().frame,
            12_000 + 10 * 1024 - LATENCY
        );

        // Paused, the sound already sent still plays out, then holds.
        control.pause();
        render(&mut renderer, 1);
        let rendered = 12_000 + 10 * 1024;
        assert_eq!(control.position().unwrap().frame, rendered + 1024 - LATENCY);
        render(&mut renderer, 5);
        assert_eq!(control.position().unwrap().frame, rendered);
        control.resume();
        render(&mut renderer, 1);
        assert_eq!(control.position().unwrap().frame, rendered);

        // A loop shows its end until its wrap is heard.
        let (mut control, mut renderer) = Renderer::new(48_000).unwrap();
        control.play(plan.clone()).unwrap();
        control.set_loop(Some((0, 24_000))).unwrap();
        render(&mut renderer, 27);
        assert_eq!(control.position().unwrap().frame, 27 * 1024 - LATENCY);
        render(&mut renderer, 3);
        assert_eq!(
            control.position().unwrap().frame,
            30 * 1024 - LATENCY - 24_000
        );

        // The end is announced once it is heard.
        control.set_loop(None).unwrap();
        let end = plan.duration_frames as usize;
        let mut blocks = 30;
        while blocks * 1024 < 24_000 + end {
            render(&mut renderer, 1);
            blocks += 1;
        }
        assert!(control.is_playing());
        render(&mut renderer, (LATENCY as usize).div_ceil(1024));
        assert!(!control.is_playing());
    }

    #[test]
    fn a_loop_past_the_end_of_the_music_stops_at_its_end() {
        let (mut control, mut renderer) = Renderer::new(48_000).unwrap();
        let plan = plan();
        control.play_from_frame(plan.clone(), 30_000).unwrap();
        control
            .set_loop(Some((24_000, plan.duration_frames * 4)))
            .unwrap();
        let mut output = [0f32; 2048];
        assert_no_alloc(|| {
            for _ in 0..20 {
                renderer.render(&mut output, 2);
            }
        });
        let frame = control.position().unwrap().frame;
        assert!((24_000..plan.duration_frames).contains(&frame), "{frame}");
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
    fn preview_rolls_at_exact_frames_and_cancels_pending_attacks_without_allocations() {
        use score_playback::Audition;
        let (mut control, mut actual) = Renderer::new(48_000).unwrap();
        let (_, mut reference) = Renderer::new(48_000).unwrap();
        let mut preview = Audition::plain(
            [Some(60), Some(64), Some(67), None, None, None, None, None],
            score_core::Instrument::Piano,
            48_000,
        );
        preview.delays[..3].copy_from_slice(&[0, 1000, 2000]);
        preview.duration_frames = 5000;
        preview.gates[..3].copy_from_slice(&[3000, 4000, 5000]);
        preview.notes[0] = Some(score_playback::NoteEvent {
            level: 56,
            ..preview.note(60)
        });
        preview.notes[1] = Some(score_playback::NoteEvent {
            level: 100,
            ..preview.note(64)
        });
        let mut invalid = preview;
        invalid.delays[1] = 5000;
        assert!(control.preview_audition(invalid).is_err());
        control.preview_audition(preview).unwrap();
        let mut a = [0f32; 1000];
        let mut b = [0f32; 1000];
        for block in 0..12 {
            if block < 6 && block % 2 == 0 {
                reference.note_on(15, preview.note_at(block / 2));
            }
            if [6, 8, 10].contains(&block) {
                reference.note_off(15, preview.keys[(block - 6) / 2].unwrap());
            }
            if block == 10 {
                reference.engines.silence(Some(15), false);
            }
            assert_no_alloc(|| {
                actual.render(&mut a, 2);
                reference.render(&mut b, 2);
            });
            assert_eq!(a, b, "block {block}");
        }
        assert!(actual.preview.is_none());
        control.preview_audition(preview).unwrap();
        reference.engines.silence(Some(15), true);
        reference.note_on(15, preview.note_at(0));
        assert_no_alloc(|| {
            actual.render(&mut a, 2);
            reference.render(&mut b, 2);
        });
        assert_eq!(a, b);
        control.stop();
        reference.reverb.clear();
        reference.engines.silence(None, true);
        for _ in 0..12 {
            assert_no_alloc(|| {
                actual.render(&mut a, 2);
                reference.render(&mut b, 2);
            });
            // Match the synth's normal stop tail, without any delayed reattack.
            assert_eq!(a, b);
        }
        assert!(actual.preview.is_none());
        // Replacing a preview also retires its remaining attacks.
        control.preview_audition(preview).unwrap();
        assert_no_alloc(|| actual.render(&mut a, 2));
        control.preview(72).unwrap();
        assert_no_alloc(|| actual.render(&mut a, 2));
        let pending = actual.preview.unwrap().0;
        assert!(
            pending
                .keys
                .iter()
                .enumerate()
                .all(|(index, key)| key.is_none() || actual.preview_attacked[index])
        );
    }

    #[test]
    fn sixteen_note_piano_preview_renders_and_releases_without_callback_allocations() {
        use score_core::{Pitches, StaffId};
        let mut score = Score::default();
        let piano = score.parts()[0].id();
        let mut ids = Vec::new();
        for (staff, octave) in [(StaffId::Upper, 5), (StaffId::Lower, 2)] {
            let pitches = Pitches::new(
                &(0..8)
                    .map(|i| Pitch::new(Step::C, octave).unwrap().shift(i * 2).unwrap())
                    .collect::<Vec<_>>(),
            )
            .unwrap();
            ids.push(
                score
                    .insert_on(
                        piano,
                        staff,
                        0,
                        0,
                        Event {
                            pitches,
                            duration: Duration::Quarter,
                            notation: Default::default(),
                        },
                    )
                    .unwrap(),
            );
        }
        score.set_arpeggio_both_hands(ids[0], true).unwrap();
        let preview =
            score_playback::Audition::for_element(&score, ids[1], Tempo::default(), 48_000)
                .unwrap();
        let (mut control, mut renderer) = Renderer::new(48_000).unwrap();
        control.preview_audition(preview).unwrap();
        let mut output = [0f32; 1024];
        let mut energy = 0f32;
        assert_no_alloc(|| {
            for _ in 0..40 {
                renderer.render(&mut output, 2);
                energy += output.iter().map(|sample| sample * sample).sum::<f32>();
            }
        });
        assert!(energy > 0.001);
        assert!(renderer.preview.is_none());
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
    #[test]
    fn last_orchestra_channel_and_percussion_render_without_callback_allocations() {
        for instrument in [
            score_core::Instrument::Violin,
            score_core::Instrument::SnareDrum,
        ] {
            let mut score =
                Score::new("Orquesta".into(), &[instrument; score_core::MAX_PARTS], 1).unwrap();
            let part = score.parts().back().unwrap().id();
            score
                .insert_on(
                    part,
                    score_core::StaffId::Upper,
                    0,
                    0,
                    Event {
                        pitches: score_core::Pitches::single(Pitch::new(Step::B, 4)),
                        duration: Duration::Quarter,
                        notation: Default::default(),
                    },
                )
                .unwrap();
            let plan = Arc::new(PlaybackPlan::new(&score, Tempo::default(), 48_000).unwrap());
            let channel = score_playback::part_channel(score_core::MAX_PARTS - 1);
            assert!(channel >= 96);
            let (mut control, mut renderer) = Renderer::new(48_000).unwrap();
            control.set_reverb(ReverbSettings {
                enabled: false,
                ..Default::default()
            });
            control.play(plan).unwrap();
            let mut output = [0f32; 4096];
            assert_no_alloc(|| renderer.render(&mut output, 2));
            assert!(output.iter().any(|v| v.abs() > 0.0001));
            control.set_channel_mix(channel, ChannelMix { volume: 0, pan: 0 });
            // The synth ramps a controller change over its next block.
            assert_no_alloc(|| {
                renderer.render(&mut output, 2);
                renderer.render(&mut output, 2);
            });
            assert!(output.iter().all(|v| *v == 0.));
        }
    }
}
