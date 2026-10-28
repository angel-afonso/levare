//! Built-in SoundFont audio. The callback owns its synth and never touches UI state.
pub mod mixer;
#[cfg(test)]
mod mixer_tests;
mod renderer;
mod reverb;
pub mod sound_engine;

#[cfg(test)]
#[global_allocator]
static TEST_ALLOCATOR: assert_no_alloc::AllocDisabler = assert_no_alloc::AllocDisabler;

use cpal::{
    FromSample, SampleFormat, SizedSample,
    traits::{DeviceTrait, HostTrait, StreamTrait},
};
pub use renderer::{Control, PlaybackPosition, Renderer};
use score_playback::PlaybackPlan;
use std::{fmt, sync::Arc};

#[derive(Debug)]
pub struct AudioError(String);
impl fmt::Display for AudioError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for AudioError {}
impl AudioError {
    pub fn new(value: impl fmt::Display) -> Self {
        Self(value.to_string())
    }
}
fn error(value: impl fmt::Display) -> AudioError {
    AudioError(value.to_string())
}

/// Field order matters: stop/drop the stream before releasing the control-side
/// plan owners, so the callback can never free the last plan reference.
pub struct AudioOutput {
    _stream: cpal::Stream,
    pub control: Control,
    sample_rate: u32,
}

impl AudioOutput {
    pub fn open() -> Result<Self, AudioError> {
        Self::open_config(None)
    }
    pub fn open_with_bank(
        bank: &dyn sound_engine::SoundBank,
        score: &score_core::Score,
        audition: score_core::Instrument,
    ) -> Result<Self, AudioError> {
        Self::open_config(Some((bank, score, audition)))
    }
    fn open_config(
        pack: Option<(
            &dyn sound_engine::SoundBank,
            &score_core::Score,
            score_core::Instrument,
        )>,
    ) -> Result<Self, AudioError> {
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .ok_or_else(|| error("No hay dispositivo de salida de audio"))?;
        let supported = device.default_output_config().map_err(error)?;
        let config = supported.config();
        let sample_rate = config.sample_rate.0;
        let (control, renderer) = match pack {
            Some((bank, score, audition)) => {
                Renderer::with_bank(sample_rate, bank, score, audition)?
            }
            None => Renderer::new(sample_rate)?,
        };
        let stream = match supported.sample_format() {
            SampleFormat::F32 => build::<f32>(&device, &config, renderer),
            SampleFormat::F64 => build::<f64>(&device, &config, renderer),
            SampleFormat::I16 => build::<i16>(&device, &config, renderer),
            SampleFormat::I32 => build::<i32>(&device, &config, renderer),
            SampleFormat::U16 => build::<u16>(&device, &config, renderer),
            SampleFormat::U32 => build::<u32>(&device, &config, renderer),
            format => return Err(error(format!("Formato de audio no compatible: {format}"))),
        }?;
        stream.play().map_err(error)?;
        Ok(Self {
            _stream: stream,
            control,
            sample_rate,
        })
    }
    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }
    pub fn play(&mut self, plan: Arc<PlaybackPlan>) -> Result<(), AudioError> {
        self.control.play(plan)
    }
    pub fn play_from(
        &mut self,
        plan: Arc<PlaybackPlan>,
        selection: Option<score_core::ElementId>,
    ) -> Result<(), AudioError> {
        match selection {
            Some(element) => self.control.play_from(plan, element),
            None => self.control.play(plan),
        }
    }
}

fn build<T: SizedSample + FromSample<f32>>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    mut renderer: Renderer,
) -> Result<cpal::Stream, AudioError> {
    let channels = usize::from(config.channels);
    let failure = renderer.failure_flag();
    device
        .build_output_stream(
            config,
            move |output: &mut [T], info: &cpal::OutputCallbackInfo| {
                let time = info.timestamp();
                renderer.set_output_latency(
                    time.playback
                        .duration_since(&time.callback)
                        .unwrap_or_default(),
                );
                renderer.render(output, channels);
            },
            move |_| failure.store(true, std::sync::atomic::Ordering::Release),
            None,
        )
        .map_err(error)
}
