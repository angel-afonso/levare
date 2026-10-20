//! Same sounding A5 and dynamic: ordinary arco, harmonic arco, ordinary pizzicato,
//! harmonic pizzicato. Rendered by the app's engine, with no hardware required.
use score_core::{
    Duration, Event, Harmonic, Instrument, Notation, Pitch, Pitches, PlayingTechnique, Score,
    StaffId, Step,
};
use score_platform::{Renderer, mixer::ReverbSettings};
use score_playback::{PlaybackPlan, Tempo};
use std::{io::Write, path::Path, sync::Arc};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output = std::env::args_os()
        .nth(1)
        .unwrap_or_else(|| "output/audio/comparacion-armonicos.wav".into());
    let mut samples = Vec::<f32>::new();
    for (technique, harmonic) in [
        (PlayingTechnique::Arco, None),
        (PlayingTechnique::Arco, Some(Harmonic::Natural)),
        (PlayingTechnique::Pizzicato, None),
        (PlayingTechnique::Pizzicato, Some(Harmonic::Natural)),
    ] {
        let mut score = Score::default();
        let piano = score.parts()[0].id();
        let part = score.add_part(Instrument::Violin)?;
        score.remove_part(piano)?;
        score.insert_on(
            part,
            StaffId::Upper,
            0,
            0,
            Event {
                pitches: Pitches::single(Pitch::new(Step::A, 5)),
                duration: Duration::Whole,
                notation: Notation {
                    technique: Some(technique),
                    harmonic,
                    ..Default::default()
                },
            },
        )?;
        let plan = Arc::new(PlaybackPlan::new(&score, Tempo::default(), 48_000)?);
        let (mut control, mut renderer) = Renderer::new(48_000)?;
        control.set_volume(100);
        control.set_reverb(ReverbSettings {
            enabled: false,
            ..Default::default()
        });
        control.play(plan)?;
        for _ in 0..225 {
            // 2.4 seconds including the release; 0.3 seconds between examples.
            let mut block = [0f32; 1024];
            renderer.render(&mut block, 2);
            samples.extend_from_slice(&block);
        }
        samples.extend(std::iter::repeat_n(0., 28_800));
    }
    write_wav(Path::new(&output), &samples)?;
    println!(
        "{}: arco normal, armónico al arco, pizzicato normal, armónico pizzicato",
        Path::new(&output).display()
    );
    Ok(())
}
fn write_wav(path: &Path, samples: &[f32]) -> std::io::Result<()> {
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent)?;
    }
    let mut file = std::fs::File::create(path)?;
    let bytes = (samples.len() * 2) as u32;
    file.write_all(b"RIFF")?;
    file.write_all(&(36 + bytes).to_le_bytes())?;
    file.write_all(b"WAVEfmt ")?;
    file.write_all(&16u32.to_le_bytes())?;
    for n in [1u16, 2] {
        file.write_all(&n.to_le_bytes())?;
    }
    for n in [48_000u32, 192_000] {
        file.write_all(&n.to_le_bytes())?;
    }
    for n in [4u16, 16] {
        file.write_all(&n.to_le_bytes())?;
    }
    file.write_all(b"data")?;
    file.write_all(&bytes.to_le_bytes())?;
    for sample in samples {
        file.write_all(&((sample.clamp(-1., 1.) * 32767.) as i16).to_le_bytes())?;
    }
    Ok(())
}
