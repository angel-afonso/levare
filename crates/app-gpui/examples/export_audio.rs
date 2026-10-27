//! Offline stereo float-WAV export using the app's score interpretation and banks.
use score_platform::{
    Renderer,
    mixer::{ChannelMix, ReverbSettings},
};
use score_playback::{PlaybackPlan, Tempo, Timeline, part_channel};
use std::{
    fs::OpenOptions,
    io::{BufWriter, Write},
    path::Path,
    sync::Arc,
};

const RATE: u32 = 48_000;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if !(2..=3).contains(&args.len()) {
        return Err("usage: export_audio input.partitura output-float.wav [sound-pack-id]".into());
    }
    let score = score_io::load(Path::new(&args[0]))?;
    let timeline = Timeline::with_repeats(&score, Tempo::default(), true);
    let plan = Arc::new(PlaybackPlan::with_repeats(
        &score,
        Tempo::default(),
        RATE,
        true,
    )?);
    println!(
        "Duration: {:.3} seconds; {} played bars",
        plan.duration_frames as f64 / f64::from(RATE),
        timeline.instances().len()
    );
    for start in [0, 80, 128] {
        if let Some((index, _)) = timeline
            .instances()
            .iter()
            .enumerate()
            .find(|(_, instance)| instance.bar == start)
        {
            println!(
                "Movement at bar {}: {:.3} seconds",
                start + 1,
                timeline.performed_frame(index, 0, RATE) as f64 / f64::from(RATE)
            );
        }
    }
    #[cfg(feature = "vsco")]
    let (mut control, mut renderer) = if let Some(id) = args.get(2) {
        use score_platform::sound_engine::SoundBank;
        let id = id.to_string_lossy();
        let (packs, _) = score_sound_vsco::SfzPack::discover_all(&[]);
        let pack = packs
            .into_iter()
            .find(|pack| pack.id() == id)
            .ok_or_else(|| format!("No sound pack with id {id}"))?;
        println!("Sound bank: {} with GeneralUser fallback", pack.name());
        Renderer::with_bank(RATE, &pack, &score, score_core::Instrument::Viola)?
    } else if let Some(bank) = score_sound_vsco::VscoPack::discover() {
        println!("Sound bank: VSCO CE with GeneralUser fallback");
        Renderer::with_bank(RATE, &bank, &score, score_core::Instrument::Viola)?
    } else {
        println!("Sound bank: GeneralUser GS");
        Renderer::new(RATE)?
    };
    #[cfg(not(feature = "vsco"))]
    let (mut control, mut renderer) = Renderer::new(RATE)?;
    renderer.offline();
    // Leave ample headroom in the float master; normalize once after rendering.
    control.set_volume(25);
    control.set_reverb(ReverbSettings {
        enabled: true,
        amount: 16,
        room: 42,
    });
    for (index, part) in score.parts().iter().enumerate() {
        let mut mix = ChannelMix::seated(&score, part.id());
        if score
            .part_name(part.id())
            .to_lowercase()
            .contains("solista")
        {
            mix.pan = 0;
            mix.volume = 100;
        } else {
            mix.volume = 76;
        }
        control.set_channel_mix(part_channel(index), mix);
        // The interpretation uses the paired channel for independent staff hairpins.
        control.set_channel_mix(part_channel(index) + 16, mix);
    }
    let total = plan.duration_frames + u64::from(RATE) * 4;
    let bytes = u32::try_from(total * 2 * 4)?;
    let path = Path::new(&args[1]);
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent)?;
    }
    let mut output = BufWriter::new(OpenOptions::new().write(true).create_new(true).open(path)?);
    output.write_all(b"RIFF")?;
    output.write_all(&(bytes + 36).to_le_bytes())?;
    output.write_all(b"WAVEfmt ")?;
    output.write_all(&16u32.to_le_bytes())?;
    output.write_all(&3u16.to_le_bytes())?; // IEEE float
    output.write_all(&2u16.to_le_bytes())?;
    output.write_all(&RATE.to_le_bytes())?;
    output.write_all(&(RATE * 8).to_le_bytes())?;
    output.write_all(&8u16.to_le_bytes())?;
    output.write_all(&32u16.to_le_bytes())?;
    output.write_all(b"data")?;
    output.write_all(&bytes.to_le_bytes())?;
    control.play(plan)?;
    let mut done = 0;
    let mut peak = 0f32;
    let mut clipped = 0u64;
    let mut next_report = 10;
    let mut samples = [0f32; 8192];
    let mut encoded = Vec::with_capacity(samples.len() * 4);
    while done < total {
        let frames = (total - done).min((samples.len() / 2) as u64) as usize;
        let block = &mut samples[..frames * 2];
        renderer.render(block, 2);
        encoded.clear();
        for &sample in block.iter() {
            if !sample.is_finite() {
                return Err("non-finite audio sample".into());
            }
            peak = peak.max(sample.abs());
            clipped += u64::from(sample.abs() >= 1.);
            encoded.extend_from_slice(&sample.to_le_bytes());
        }
        output.write_all(&encoded)?;
        done += frames as u64;
        if done * 100 / total >= next_report {
            println!("Rendered {next_report}%");
            next_report += 10;
        }
    }
    output.flush()?;
    if control.failed() || peak == 0. || clipped != 0 {
        return Err(format!(
            "Invalid render: peak={peak}, clipped={clipped}, failed={}",
            control.failed()
        )
        .into());
    }
    println!(
        "Complete: peak={peak:.9}; clipped={clipped}; {:.3} seconds with release",
        total as f64 / f64::from(RATE)
    );
    Ok(())
}
