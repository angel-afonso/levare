//! Offline comparison through the same renderer as the app. Native file streaming
//! uses sfizz's offline mode so rendering faster than real time cannot underrun.
use score_core::{Duration, Event, Instrument, Notation, Pitch, Pitches, Score, Step};
use score_platform::Renderer;
use score_playback::{PlaybackPlan, Tempo};
use score_sound_vsco::VscoPack;
use std::{fs::File, io::Write, path::PathBuf, sync::Arc};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args_os().skip(1);
    let manifest = args
        .next()
        .map(PathBuf::from)
        .unwrap_or_else(|| "soundpacks/vsco-ce/manifest.json".into());
    let output = args
        .next()
        .map(PathBuf::from)
        .unwrap_or_else(|| "exports/vsco-comparison".into());
    let pack = VscoPack::load(&manifest)?;
    std::fs::create_dir_all(&output)?;
    for (name, instrument, octave) in [
        ("piano", Instrument::Piano, 4),
        ("violin", Instrument::Violin, 4),
        ("flauta", Instrument::Flute, 5),
    ] {
        let mut score = Score::default();
        let piano = score.parts()[0].id();
        let part = if instrument == Instrument::Piano {
            piano
        } else {
            let part = score.add_part(instrument)?;
            score.remove_part(piano)?;
            part
        };
        for (index, step) in [
            Step::C,
            Step::E,
            Step::G,
            Step::B,
            Step::A,
            Step::G,
            Step::E,
            Step::C,
        ]
        .into_iter()
        .enumerate()
        {
            score.insert_on(
                part,
                score_core::StaffId::Upper,
                index / 4,
                index % 4,
                Event {
                    pitches: Pitches::single(Pitch::new(step, octave)),
                    duration: Duration::Quarter,
                    notation: Notation::default(),
                },
            )?;
        }
        let plan = Arc::new(PlaybackPlan::new(&score, Tempo::default(), 48_000)?);
        for vsco in [false, true] {
            let (mut control, mut renderer) = if vsco {
                Renderer::with_bank(48_000, &pack, &score, instrument)?
            } else {
                Renderer::new(48_000)?
            };
            renderer.offline();
            control.play(plan.clone())?;
            let frames = plan.duration_frames as usize + 48_000;
            let mut samples = Vec::with_capacity(frames * 2);
            let mut block = [0f32; 1024];
            while samples.len() < frames * 2 {
                renderer.render(&mut block, 2);
                samples.extend_from_slice(&block);
            }
            let path = output.join(format!(
                "{name}-{}.wav",
                if vsco { "vsco" } else { "generaluser" }
            ));
            write_wav(&path, &samples)?;
            let energy: f64 = samples.iter().map(|v| f64::from(v * v)).sum();
            if !energy.is_finite() || energy < 0.001 {
                return Err(format!("No hay audio válido: {}", path.display()).into());
            }
            println!(
                "{} · RMS {:.4}",
                path.display(),
                (energy / samples.len() as f64).sqrt()
            );
        }
    }
    Ok(())
}

fn write_wav(path: &std::path::Path, samples: &[f32]) -> std::io::Result<()> {
    let mut file = File::create(path)?;
    let bytes = (samples.len() * 2) as u32;
    file.write_all(b"RIFF")?;
    file.write_all(&(36 + bytes).to_le_bytes())?;
    file.write_all(b"WAVEfmt ")?;
    file.write_all(&16u32.to_le_bytes())?;
    for value in [1u16, 2] {
        file.write_all(&value.to_le_bytes())?;
    }
    for value in [48_000u32, 48_000 * 4] {
        file.write_all(&value.to_le_bytes())?;
    }
    for value in [4u16, 16] {
        file.write_all(&value.to_le_bytes())?;
    }
    file.write_all(b"data")?;
    file.write_all(&bytes.to_le_bytes())?;
    for sample in samples {
        file.write_all(&((sample.clamp(-1., 1.) * 32767.) as i16).to_le_bytes())?;
    }
    Ok(())
}
