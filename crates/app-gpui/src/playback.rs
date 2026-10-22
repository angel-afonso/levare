use score_core::{ElementId, Instrument, PartId, Score, StaffId};
use score_platform::AudioOutput;
use score_platform::mixer::{ChannelMix, ReverbSettings};
use score_platform::sound_engine::{RoutingSignature, SoundBank};
use score_playback::{Audition, PlaybackPlan, Style, Tempo};
use std::collections::HashMap;
use std::sync::Arc;

/// UI-owned transport. The audio callback only receives immutable prepared plans.
pub struct Playback {
    output: Option<AudioOutput>,
    pub error: Option<String>,
    pub tempo: Tempo,
    pub follow_repeats: bool,
    pub style: Style,
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
    pub sound_discovery_errors: Vec<String>,
    pub sound_priority: Vec<String>,
    #[cfg(feature = "vsco")]
    sfz_packs: Vec<score_sound_vsco::SfzPack>,
    pub selected_bank: Option<usize>,
    pub sound_error: Option<String>,
    sound_signature: Option<RoutingSignature>,
    pub part_mix: HashMap<PartId, ChannelMix>,
    pub reverb: ReverbSettings,
    mix_channels: [ChannelMix; score_playback::PLAYBACK_CHANNELS],
}

impl Playback {
    pub fn new() -> Self {
        let catalog = crate::sound_banks::discover();
        let mut playback = Self {
            output: None,
            error: None,
            tempo: Tempo::default(),
            follow_repeats: true,
            style: Style::Natural,
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
            banks: catalog.banks,
            sound_discovery_errors: catalog.errors,
            sound_priority: catalog.priority,
            #[cfg(feature = "vsco")]
            sfz_packs: catalog.packs,
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
            Err(error) => {
                self.error = Some(format!(
                    "No se pudo abrir la salida de audio ({error}). Pulsa Reintentar audio en Sonidos."
                ))
            }
        }
    }
    pub fn sound_name(&self) -> String {
        self.selected_bank
            .and_then(|index| self.banks.get(index))
            .map_or("GeneralUser GS", |bank| bank.name())
            .to_owned()
    }
    pub fn sound_description(&self) -> Option<String> {
        self.selected_bank
            .and_then(|index| self.banks.get(index))
            .map(|bank| bank.description().to_owned())
    }
    pub fn sound_priority_name(&self, id: &str) -> String {
        self.banks
            .iter()
            .find(|bank| bank.id() == id)
            .map_or_else(|| id.to_owned(), |bank| bank.name().to_owned())
    }
    pub fn move_sound_priority(&mut self, index: usize, up: bool) -> Result<usize, String> {
        #[cfg(feature = "vsco")]
        {
            let next = if up {
                index.checked_sub(1)
            } else {
                index.checked_add(1)
            };
            let Some(next) = next.filter(|next| *next < self.sound_priority.len()) else {
                return Err("La biblioteca ya está en ese extremo".into());
            };
            let mut priority = self.sound_priority.clone();
            priority.swap(index, next);
            crate::sound_banks::save_priority(&priority)?;
            let selected_id = self
                .selected_bank
                .and_then(|index| self.banks.get(index))
                .map(|bank| bank.id().to_owned());
            self.stop();
            self.banks = crate::sound_banks::banks_from_packs(&self.sfz_packs, &priority);
            self.sound_priority = priority;
            self.selected_bank =
                selected_id.and_then(|id| crate::sound_banks::index_by_id(&self.banks, &id));
            crate::sound_banks::index_by_id(&self.banks, "sfz-combinadas")
                .ok_or_else(|| "Bibliotecas combinadas no disponibles".into())
        }
        #[cfg(not(feature = "vsco"))]
        {
            let _ = (index, up);
            Err("Bibliotecas SFZ no disponibles".into())
        }
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
        score_playback::Timeline::with_repeats(score, self.tempo, self.follow_repeats)
    }
    pub fn toggle_repeats(&mut self) {
        self.stop();
        self.follow_repeats = !self.follow_repeats;
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
            self.error.clone().unwrap_or_else(|| {
                "No hay salida de audio. Pulsa Reintentar audio en Sonidos.".into()
            })
        })?;
        let plan = Arc::new(transport_plan(
            score,
            self.tempo,
            output.sample_rate(),
            self.follow_repeats,
            self.style,
        )?);
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
            let plan = Arc::new(transport_plan(
                score,
                self.tempo,
                output.sample_rate(),
                self.follow_repeats,
                self.style,
            )?);
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
            self.error =
                Some("La salida de audio se desconectó. Pulsa Reintentar audio en Sonidos.".into());
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
fn transport_plan(
    score: &Score,
    tempo: Tempo,
    rate: u32,
    follow: bool,
    style: Style,
) -> Result<PlaybackPlan, String> {
    let mut plan = PlaybackPlan::with_style(score, tempo, rate, follow, style)
        .map_err(|error| error.to_string())?;
    plan.duration_frames = plan.timeline.total_frames(rate);
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
            follow_repeats: true,
            style: Style::Natural,
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
            sound_discovery_errors: Vec::new(),
            sound_priority: Vec::new(),
            #[cfg(feature = "vsco")]
            sfz_packs: Vec::new(),
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
        let mut score = Score::default();
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
        score
            .set_tempo_at(
                0,
                score_core::TempoMark {
                    bpm: 115,
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(playback.position(&score, cursor).tick, chosen.tick);
        playback.stop();
        assert!(!playback.has_ruler_position());
        assert_eq!(playback.position(&score, cursor).tick, 0.);
    }

    #[test]
    fn transport_plan_includes_the_rulers_empty_final_bars() {
        let score = Score::default();
        let plan = transport_plan(&score, Tempo::default(), 48_000, true, Style::Natural).unwrap();
        assert_eq!(plan.duration_frames, 384_000);
        assert!(plan.messages.is_empty());
        assert!(plan.elements_at(360_000).iter().all(Option::is_none));
    }

    #[test]
    fn transport_starts_natural_and_can_request_exact_literal_attacks() {
        assert_eq!(offline().style, Style::Natural);
        let mut score = Score::default();
        score
            .insert(
                0,
                0,
                score_core::Event {
                    pitches: score_core::Pitches::single(score_core::Pitch::new(
                        score_core::Step::C,
                        4,
                    )),
                    duration: score_core::Duration::Quarter,
                    notation: Default::default(),
                },
            )
            .unwrap();
        let natural =
            transport_plan(&score, Tempo::default(), 48_000, false, Style::Natural).unwrap();
        let literal =
            transport_plan(&score, Tempo::default(), 48_000, false, Style::Literal).unwrap();
        assert_ne!(natural.messages, literal.messages);
        assert_eq!(natural.duration_frames, literal.duration_frames);
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
    #[test]
    fn repeat_mode_defaults_on_and_toggle_clears_the_previous_clock() {
        let mut playback = offline();
        assert!(playback.follow_repeats);
        playback.clock = Some((192_000, 48_000));
        playback.seek_tick = Some(192);
        playback.toggle_repeats();
        assert!(!playback.follow_repeats);
        assert!(playback.clock.is_none());
        assert!(!playback.has_ruler_position());
        playback.toggle_repeats();
        assert!(playback.follow_repeats);
    }

    #[test]
    fn transport_plan_follows_repeats_and_can_use_written_order() {
        let mut score = Score::new("Repeat".into(), &[Instrument::Piano], 2).unwrap();
        score
            .set_bar_form(
                1,
                score_core::BarForm {
                    repeat_end: Some(2),
                    ..Default::default()
                },
            )
            .unwrap();
        let repeated =
            transport_plan(&score, Tempo::default(), 48_000, true, Style::Natural).unwrap();
        let linear =
            transport_plan(&score, Tempo::default(), 48_000, false, Style::Natural).unwrap();
        assert_eq!(repeated.duration_frames, linear.duration_frames * 2);
        assert_eq!(repeated.timeline.instances().len(), 4);
        assert_eq!(linear.timeline.instances().len(), 2);
    }
}
