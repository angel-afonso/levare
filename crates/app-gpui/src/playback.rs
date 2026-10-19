use score_core::{ElementId, Instrument, PartId, Pitches, Score, StaffId};
use score_platform::AudioOutput;
use score_playback::{PlaybackPlan, Tempo, midi_key};
use std::sync::Arc;

/// UI-owned transport. The audio callback only receives immutable prepared plans.
pub struct Playback {
    output: Option<AudioOutput>,
    pub error: Option<String>,
    pub tempo: Tempo,
    pub audition: bool,
    pub volume: u32,
    pub playhead: Option<ElementId>,
    pub playheads: Vec<ElementId>,
    follow_part: PartId,
    follow_staff: StaffId,
    was_active: bool,
    pub just_finished: bool,
    clock: Option<(u64, u32)>,
    seek_tick: Option<u64>,
}

impl Playback {
    pub fn new() -> Self {
        let mut playback = Self {
            output: None,
            error: None,
            tempo: Tempo::default(),
            audition: true,
            volume: 70,
            playhead: None,
            playheads: Vec::new(),
            follow_part: PartId::default(),
            follow_staff: StaffId::Upper,
            was_active: false,
            just_finished: false,
            clock: None,
            seek_tick: None,
        };
        playback.connect();
        playback
    }
    pub fn connect(&mut self) {
        self.stop();
        self.output = None;
        match AudioOutput::open() {
            Ok(mut output) => {
                output.control.set_volume(self.volume);
                self.output = Some(output);
                self.error = None;
            }
            Err(error) => self.error = Some(error.to_string()),
        }
    }
    pub fn position(&self, score: &Score, cursor: score_edit::Cursor) -> crate::playhead::Position {
        if self.clock.is_none()
            && let Some(tick) = self.seek_tick
        {
            return crate::playhead::position_at_tick(score, tick as f64, self.tempo);
        }
        crate::playhead::position(score, cursor, self.clock, self.tempo)
    }
    pub fn has_ruler_position(&self) -> bool {
        self.seek_tick.is_some()
    }
    pub fn is_playing(&self) -> bool {
        self.output
            .as_ref()
            .is_some_and(|output| output.control.is_playing())
    }
    pub fn is_paused(&self) -> bool {
        self.output
            .as_ref()
            .is_some_and(|output| output.control.is_paused())
    }
    pub fn follow_staff(&mut self, part: PartId, staff: StaffId) {
        self.follow_part = part;
        self.follow_staff = staff;
    }
    pub fn toggle(
        &mut self,
        score: &Score,
        selection: Option<ElementId>,
        part: PartId,
        staff: StaffId,
    ) -> Result<(), String> {
        if self.is_playing() {
            if let Some(output) = &mut self.output {
                output.control.pause();
            }
            return Ok(());
        }
        if self.is_paused() {
            if let Some(output) = &mut self.output {
                output.control.resume();
            }
            return Ok(());
        }
        self.follow_part = part;
        self.follow_staff = staff;
        let output = self.output.as_mut().ok_or_else(|| {
            self.error
                .clone()
                .unwrap_or_else(|| "No hay audio disponible".into())
        })?;
        let plan = Arc::new(transport_plan(score, self.tempo, output.sample_rate())?);
        if let Some(tick) = self.seek_tick {
            let frame = self.tempo.frame_at(tick, output.sample_rate());
            output.control.play_from_frame(plan, frame)
        } else {
            match selection {
                Some(id) => output.control.play_from(plan, id),
                None => output.control.play(plan),
            }
        }
        .map_err(|error| error.to_string())?;
        self.was_active = true;
        self.tick();
        Ok(())
    }

    pub fn seek(&mut self, score: &Score, tick: u64) -> Result<(), String> {
        let was_playing = self.is_playing();
        let was_paused = self.is_paused();
        if was_playing || was_paused {
            let output = self.output.as_mut().expect("active audio output");
            let plan = Arc::new(transport_plan(score, self.tempo, output.sample_rate())?);
            output
                .control
                .play_from_frame(plan, self.tempo.frame_at(tick, output.sample_rate()))
                .map_err(|error| error.to_string())?;
        }
        self.seek_tick = Some(tick);
        self.clock = None;
        self.playhead = None;
        self.playheads.clear();
        self.just_finished = false;
        if was_playing || was_paused {
            self.tick();
        }
        Ok(())
    }
    pub fn stop(&mut self) {
        if let Some(output) = &mut self.output {
            output.control.stop();
        }
        self.playhead = None;
        self.clock = None;
        self.seek_tick = None;
        self.playheads.clear();
        self.was_active = false;
        self.just_finished = false;
    }
    pub fn preview(
        &mut self,
        pitches: Pitches,
        instrument: Instrument,
        technique: score_core::PlayingTechnique,
    ) -> Result<(), String> {
        if self.audition
            && let Some(output) = &mut self.output
        {
            let keys = audition_keys(pitches);
            output
                .control
                .preview_chord(keys, technique.program(instrument))
                .map_err(|error| error.to_string())?;
        }
        Ok(())
    }
    pub fn change_tempo(&mut self, delta: i16) {
        let seek_tick = self.seek_tick;
        self.stop();
        self.seek_tick = seek_tick;
        let bpm = (self.tempo.bpm() as i16 + delta).clamp(30, 240) as u16;
        self.tempo = Tempo::new(bpm).expect("bounded tempo");
    }
    pub fn change_volume(&mut self, delta: i32) {
        self.volume = (self.volume as i32 + delta).clamp(0, 100) as u32;
        if let Some(output) = &mut self.output {
            output.control.set_volume(self.volume);
        }
    }
    /// Notify GPUI only while progressing or if the audio device fails.
    pub fn tick(&mut self) -> bool {
        self.just_finished = false;
        let was = self.playhead;
        let Some(output) = &mut self.output else {
            return false;
        };
        if output.control.failed() {
            self.output = None;
            self.playhead = None;
            self.clock = None;
            self.playheads.clear();
            self.error = Some("La salida de audio se desconectó. Pulsa Reintentar audio.".into());
            return true;
        }
        let playing = output.control.is_playing();
        let active = playing || output.control.is_paused();
        let changed = active != self.was_active;
        self.just_finished = self.was_active && !active;
        self.was_active = active;
        let position = output.control.position();
        self.clock = position
            .as_ref()
            .map(|position| (position.frame, output.sample_rate()));
        self.playheads.clear();
        if let Some(position) = position {
            self.playheads
                .extend(position.elements.into_iter().flatten());
        }
        // Prefer the chosen staff; during its implicit gaps, reveal another
        // sounding part at the same musical time.
        self.playhead = self
            .playheads
            .iter()
            .copied()
            .find(|id| id.part == self.follow_part && id.staff == self.follow_staff)
            .or_else(|| {
                self.playheads
                    .iter()
                    .copied()
                    .find(|id| id.part == self.follow_part)
            })
            .or_else(|| self.playheads.first().copied());
        changed || was != self.playhead || playing
    }
}

/// The visible ruler includes implicit rests through the final bar.
fn transport_plan(score: &Score, tempo: Tempo, rate: u32) -> Result<PlaybackPlan, String> {
    let mut plan = PlaybackPlan::new(score, tempo, rate).map_err(|error| error.to_string())?;
    plan.duration_frames = tempo.frame_at(
        score.measures().len() as u64 * u64::from(score.time().capacity()),
        rate,
    );
    Ok(plan)
}

fn audition_keys(pitches: Pitches) -> [Option<u8>; score_core::MAX_CHORD_NOTES] {
    let mut keys = [None; score_core::MAX_CHORD_NOTES];
    let mut count = 0;
    for pitch in pitches.iter() {
        let key = Some(midi_key(pitch));
        if !keys.contains(&key) {
            keys[count] = key;
            count += 1;
        }
    }
    keys
}
#[cfg(test)]
mod tests {
    use super::*;

    fn offline() -> Playback {
        Playback {
            output: None,
            error: None,
            tempo: Tempo::default(),
            audition: false,
            volume: 70,
            playhead: None,
            playheads: Vec::new(),
            follow_part: PartId::default(),
            follow_staff: StaffId::Upper,
            was_active: false,
            just_finished: false,
            clock: None,
            seek_tick: None,
        }
    }

    #[test]
    fn idle_seek_is_independent_of_edit_cursor_and_survives_clock_polling_and_tempo() {
        let score = Score::default();
        let mut playback = offline();
        let cursor = score_edit::Cursor::default();
        playback.seek(&score, 432).unwrap();
        assert!(!playback.is_playing());
        assert!(!playback.is_paused());
        playback.tick();
        let chosen = playback.position(&score, cursor);
        assert_eq!((chosen.tick, chosen.measure, chosen.beat), (432., 2, 2.));
        let moved_cursor = score_edit::Cursor {
            measure: 1,
            ..cursor
        };
        assert_eq!(playback.position(&score, moved_cursor).tick, chosen.tick);
        playback.change_tempo(-5);
        assert_eq!(playback.position(&score, cursor).tick, chosen.tick);
        playback.stop();
        assert!(!playback.has_ruler_position());
        assert_eq!(playback.position(&score, cursor).tick, 0.);
    }

    #[test]
    fn transport_plan_includes_the_rulers_empty_final_bars() {
        let score = Score::default();
        let plan = transport_plan(&score, Tempo::default(), 48_000).unwrap();
        assert_eq!(plan.duration_frames, 384_000);
        assert!(plan.messages.is_empty());
        assert!(plan.elements_at(360_000).iter().all(Option::is_none));
    }

    #[test]
    fn audition_deduplicates_enharmonic_tones_without_losing_other_notes() {
        use score_core::{
            Accidental::{Flat, Natural, Sharp},
            Pitch, Step,
        };
        let pitches = Pitches::new(&[
            Pitch::new(Step::C, 4).unwrap().with_accidental(Some(Sharp)),
            Pitch::new(Step::D, 4).unwrap().with_accidental(Some(Flat)),
            Pitch::new(Step::E, 4)
                .unwrap()
                .with_accidental(Some(Natural)),
        ])
        .unwrap();
        assert_eq!(
            audition_keys(pitches),
            [Some(61), Some(64), None, None, None, None, None, None]
        );
    }
}
