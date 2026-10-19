use score_core::{ElementId, Instrument, PartId, Score, StaffId};
use score_platform::AudioOutput;
use score_platform::mixer::{ChannelMix, ReverbSettings};
use score_platform::sound_engine::{RoutingSignature, SoundBank};
use score_playback::{Audition, PlaybackPlan, Tempo};
use std::collections::HashMap;
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
    pub banks: Vec<Arc<dyn SoundBank>>,
    pub selected_bank: Option<usize>,
    pub sound_error: Option<String>,
    sound_signature: Option<RoutingSignature>,
    pub part_mix: HashMap<PartId, ChannelMix>,
    pub reverb: ReverbSettings,
    mix_channels: [ChannelMix; score_playback::PLAYBACK_CHANNELS],
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
            banks: crate::sound_banks::discover(),
            selected_bank: None,
            sound_error: None,
            sound_signature: None,
            part_mix: HashMap::new(),
            reverb: ReverbSettings::default(),
            mix_channels: [ChannelMix::default(); score_playback::PLAYBACK_CHANNELS],
        };
        playback.connect();
        playback
    }
    pub fn connect(&mut self) {
        self.stop();
        self.output = None;
        self.selected_bank = None;
        self.sound_signature = None;
        match AudioOutput::open() {
            Ok(output) => self.install_output(output),
            Err(error) => self.error = Some(error.to_string()),
        }
    }
    pub fn sound_name(&self) -> &'static str {
        self.selected_bank
            .and_then(|index| self.banks.get(index))
            .map_or("GeneralUser GS", |bank| bank.name())
    }
    pub fn sound_description(&self) -> Option<&'static str> {
        self.selected_bank
            .and_then(|index| self.banks.get(index))
            .map(|bank| bank.description())
    }
    pub fn choose_sound(
        &mut self,
        bank: Option<usize>,
        score: &Score,
        audition_part: PartId,
    ) -> Result<(), String> {
        self.stop();
        let audition = score
            .part(audition_part)
            .map_or(Instrument::Piano, |p| p.instrument());
        self.apply_mixer(score, audition_part);
        let output = match bank {
            Some(index) => {
                let bank = self
                    .banks
                    .get(index)
                    .ok_or("El banco de sonidos no está disponible")?;
                AudioOutput::open_with_bank(bank.as_ref(), score, audition)
            }
            None => AudioOutput::open(),
        }
        .map_err(|error| error.to_string())?;
        self.install_output(output);
        self.selected_bank = bank;
        self.sound_signature = bank.map(|_| RoutingSignature::new(score, audition));
        self.sound_error = None;
        Ok(())
    }
    fn install_output(&mut self, mut output: AudioOutput) {
        output.control.set_volume(self.volume);
        output.control.set_reverb(self.reverb);
        for (channel, mix) in self.mix_channels.iter().enumerate() {
            output.control.set_channel_mix(channel as u8, *mix);
        }
        self.output = Some(output);
        self.error = None;
    }
    /// The session's adjustment for a part, else its seat on the stage.
    pub fn part_mix(&self, score: &Score, part: PartId) -> ChannelMix {
        self.part_mix
            .get(&part)
            .copied()
            .unwrap_or_else(|| ChannelMix::seated(score, part))
    }
    pub fn part_mix_mut(&mut self, score: &Score, part: PartId) -> &mut ChannelMix {
        self.part_mix
            .entry(part)
            .or_insert_with(|| ChannelMix::seated(score, part))
    }
    pub fn apply_mixer(&mut self, score: &Score, audition: PartId) {
        self.mix_channels.fill(ChannelMix::default());
        for (index, part) in score.parts().iter().enumerate() {
            let channel = usize::from(score_playback::part_channel(index));
            let mix = self.part_mix(score, part.id());
            self.mix_channels[channel] = mix;
            // A piano may split its hands into the second bank for hairpins.
            self.mix_channels[channel + 16] = mix;
        }
        self.mix_channels[15] = self.part_mix(score, audition);
        if let Some(output) = &mut self.output {
            output.control.set_reverb(self.reverb);
            for (channel, mix) in self.mix_channels.iter().enumerate() {
                output.control.set_channel_mix(channel as u8, *mix);
            }
        }
    }
    pub fn reset_part_mix(&mut self, score: &Score, audition: PartId) {
        self.part_mix.clear();
        self.apply_mixer(score, audition);
    }
    fn prepare_sound(&mut self, score: &Score, audition: Instrument) -> Result<(), String> {
        if let Some(index) = self.selected_bank {
            let signature = RoutingSignature::new(score, audition);
            if self.sound_signature.as_ref() != Some(&signature) {
                let bank = self
                    .banks
                    .get(index)
                    .ok_or("El banco de sonidos ya no está disponible")?;
                let output = AudioOutput::open_with_bank(bank.as_ref(), score, audition)
                    .map_err(|error| error.to_string())?;
                self.install_output(output);
                self.sound_signature = Some(signature);
            }
        }
        Ok(())
    }
    pub fn position(&self, score: &Score, cursor: score_edit::Cursor) -> crate::playhead::Position {
        if self.clock.is_none()
            && let Some(tick) = self.seek_tick
        {
            return crate::playhead::position_at_tick(score, tick as f64, &self.timeline(score));
        }
        crate::playhead::position(score, cursor, self.clock, &self.timeline(score))
    }
    /// Musical to audio time at the current tempo, fermatas included.
    pub fn timeline(&self, score: &Score) -> score_playback::Timeline {
        score_playback::Timeline::new(score, self.tempo)
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
        self.apply_mixer(score, part);
        self.prepare_sound(
            score,
            score
                .part(part)
                .map_or(Instrument::Piano, |part| part.instrument()),
        )?;
        let output = self.output.as_mut().ok_or_else(|| {
            self.error
                .clone()
                .unwrap_or_else(|| "No hay audio disponible".into())
        })?;
        let plan = Arc::new(transport_plan(score, self.tempo, output.sample_rate())?);
        if let Some(tick) = self.seek_tick {
            let frame = plan.timeline.frame_at(tick, output.sample_rate());
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
                .play_from_frame(
                    plan.clone(),
                    plan.timeline.frame_at(tick, output.sample_rate()),
                )
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
    pub fn preview(&mut self, score: &Score, id: ElementId) -> Result<(), String> {
        if !self.audition {
            return Ok(());
        }
        let instrument = score
            .part(id.part)
            .ok_or("El instrumento no existe")?
            .instrument();
        self.apply_mixer(score, id.part);
        self.prepare_sound(score, instrument)?;
        if let Some(output) = &mut self.output {
            let audition = Audition::for_element(score, id, self.tempo, output.sample_rate())
                .map_err(|error| error.to_string())?;
            output
                .control
                .preview_audition(audition)
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
    plan.duration_frames = plan.timeline.frame_at(
        score.measures().len() as u64 * u64::from(score.time().capacity()),
        rate,
    );
    Ok(plan)
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
            banks: Vec::new(),
            selected_bank: None,
            sound_error: None,
            sound_signature: None,
            part_mix: HashMap::new(),
            reverb: ReverbSettings::default(),
            mix_channels: [ChannelMix::default(); score_playback::PLAYBACK_CHANNELS],
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
    fn mix_follows_part_identity_across_reordering_removal_and_engine_preparation() {
        let mut score = Score::default();
        let piano = score.parts()[0].id();
        let mut playback = offline();
        let chosen = ChannelMix {
            volume: 65,
            pan: -30,
        };
        playback.part_mix.insert(piano, chosen);
        let flute = score.add_part(Instrument::Flute).unwrap();
        score.arrange_parts();
        playback.apply_mixer(&score, piano);
        // An untouched part takes its stage seat (flute slightly left).
        assert_eq!(playback.mix_channels[0], ChannelMix::seated(&score, flute));
        assert_eq!(playback.mix_channels[0].pan, -15);
        assert_eq!(playback.mix_channels[1], chosen);
        assert_eq!(playback.mix_channels[17], chosen);
        assert_eq!(playback.mix_channels[15], chosen);
        let with_flute = score.clone();
        score.remove_part(flute).unwrap();
        playback.apply_mixer(&score, piano);
        assert_eq!(playback.mix_channels[0], chosen);
        assert_eq!(playback.mix_channels[1], ChannelMix::default());
        playback.apply_mixer(&with_flute, flute);
        assert_eq!(playback.mix_channels[1], chosen);
        assert_eq!(
            playback.mix_channels[15],
            ChannelMix::seated(&with_flute, flute)
        );
        playback.reset_part_mix(&Score::default(), piano);
        assert_eq!(
            playback.part_mix(&Score::default(), piano),
            ChannelMix::default()
        );
    }
}
