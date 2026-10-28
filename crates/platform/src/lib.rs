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
pub use renderer::{Control, Health, PlaybackPosition, Renderer};
use score_playback::PlaybackPlan;
use std::{
    fmt,
    sync::{
        Arc,
        mpsc::{self, Receiver, Sender},
    },
};

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
    stream: Option<cpal::Stream>,
    /// The renderer comes back here when its stream is dropped, so the
    /// stream can be reopened with the same sounds and position.
    returned: Receiver<Renderer>,
    lender: Sender<Renderer>,
    pub control: Control,
    sample_rate: u32,
    device: Option<String>,
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
        let device = default_device()?;
        let supported = device.default_output_config().map_err(error)?;
        let sample_rate = supported.sample_rate().0;
        let (control, renderer) = match pack {
            Some((bank, score, audition)) => {
                Renderer::with_bank(sample_rate, bank, score, audition)?
            }
            None => Renderer::new(sample_rate)?,
        };
        let (lender, returned) = mpsc::channel();
        let name = device.name().ok();
        let stream = start(&device, supported, Lent::new(renderer, lender.clone()))?;
        Ok(Self {
            stream: Some(stream),
            returned,
            lender,
            control,
            sample_rate,
            device: name,
        })
    }

    /// Open the stream again on the default device, with the same renderer:
    /// the same sounds, and playback carries on where it was. For a stream
    /// that reported an error, or a default device that changed.
    pub fn reopen(&mut self) -> Result<(), AudioError> {
        // Dropping the stream stops its thread and hands the renderer back.
        self.stream = None;
        let renderer = self
            .returned
            .recv_timeout(std::time::Duration::from_secs(1))
            .map_err(|_| error("La salida de audio no se pudo reanudar"))?;
        let lent = Lent::new(renderer, self.lender.clone());
        let device = default_device()?;
        let supported = config_at(&device, self.sample_rate)?;
        let name = device.name().ok();
        // Errors of the old stream are behind us; the new one reports its own.
        self.control.set_health(Health::Running);
        match start(&device, supported, lent) {
            Ok(stream) => {
                self.stream = Some(stream);
                self.device = name;
                Ok(())
            }
            Err(failure) => {
                self.control.set_health(Health::Interrupted);
                Err(failure)
            }
        }
    }

    /// The name of the device this output plays on.
    pub fn device_name(&self) -> Option<&str> {
        self.device.as_deref()
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

/// The name of the system's default output device, if there is one.
pub fn default_output_device_name() -> Option<String> {
    cpal::default_host().default_output_device()?.name().ok()
}

fn default_device() -> Result<cpal::Device, AudioError> {
    cpal::default_host()
        .default_output_device()
        .ok_or_else(|| error("No hay dispositivo de salida de audio"))
}

/// The device's own configuration when it runs at `rate`, else the closest
/// one it offers at that rate: a renderer and its plans keep their rate.
fn config_at(device: &cpal::Device, rate: u32) -> Result<cpal::SupportedStreamConfig, AudioError> {
    let preferred = device.default_output_config().map_err(error)?;
    if preferred.sample_rate().0 == rate {
        return Ok(preferred);
    }
    device
        .supported_output_configs()
        .map_err(error)?
        .filter_map(|range| range.try_with_sample_rate(cpal::SampleRate(rate)))
        .max_by_key(|config| {
            (
                config.sample_format() == preferred.sample_format(),
                config.channels() == preferred.channels(),
                config.sample_format() == SampleFormat::F32,
            )
        })
        .ok_or_else(|| error("La salida de audio usa otra frecuencia"))
}

/// A renderer lent to a stream's callback. However the callback is dropped,
/// with its stream or because the stream failed to build, the renderer is
/// sent back to its output, off the audio thread.
struct Lent {
    renderer: Option<Renderer>,
    back: Sender<Renderer>,
}

impl Lent {
    fn new(renderer: Renderer, back: Sender<Renderer>) -> Self {
        Self {
            renderer: Some(renderer),
            back,
        }
    }
}

impl Drop for Lent {
    fn drop(&mut self) {
        if let Some(renderer) = self.renderer.take() {
            let _ = self.back.send(renderer);
        }
    }
}

fn start(
    device: &cpal::Device,
    supported: cpal::SupportedStreamConfig,
    lent: Lent,
) -> Result<cpal::Stream, AudioError> {
    let config = supported.config();
    let stream = match supported.sample_format() {
        SampleFormat::F32 => build::<f32>(device, &config, lent),
        SampleFormat::F64 => build::<f64>(device, &config, lent),
        SampleFormat::I16 => build::<i16>(device, &config, lent),
        SampleFormat::I32 => build::<i32>(device, &config, lent),
        SampleFormat::U16 => build::<u16>(device, &config, lent),
        SampleFormat::U32 => build::<u32>(device, &config, lent),
        format => return Err(error(format!("Formato de audio no compatible: {format}"))),
    }?;
    stream.play().map_err(error)?;
    Ok(stream)
}

fn build<T: SizedSample + FromSample<f32>>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    mut lent: Lent,
) -> Result<cpal::Stream, AudioError> {
    let channels = usize::from(config.channels);
    let Some(renderer) = &lent.renderer else {
        return Err(error("La salida de audio no tiene sonido que reproducir"));
    };
    if renderer.sample_rate() != config.sample_rate.0 {
        return Err(error("La salida de audio usa otra frecuencia"));
    }
    let health = renderer.health_flag();
    device
        .build_output_stream(
            config,
            move |output: &mut [T], info: &cpal::OutputCallbackInfo| {
                if let Some(renderer) = &mut lent.renderer {
                    let time = info.timestamp();
                    renderer.set_output_latency(
                        time.playback
                            .duration_since(&time.callback)
                            .unwrap_or_default(),
                    );
                    renderer.render(output, channels);
                }
            },
            move |failure| {
                // Only a vanished device is final. ALSA reports everything
                // else, including errors after suspend, as backend errors
                // the stream may not recover from alone: reopen it.
                let level = match failure {
                    cpal::StreamError::DeviceNotAvailable => Health::Disconnected,
                    cpal::StreamError::BackendSpecific { .. } => Health::Interrupted,
                };
                health.fetch_max(level as u8, std::sync::atomic::Ordering::AcqRel);
            },
            None,
        )
        .map_err(error)
}
