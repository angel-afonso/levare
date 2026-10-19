use crate::{
    Control, Renderer,
    mixer::{ChannelMix, ReverbSettings},
};
use score_core::{Duration, Event, Instrument, Notation, Pitch, Pitches, Score, StaffId, Step};
use score_playback::{PlaybackPlan, Tempo};
use std::sync::Arc;

fn ensemble() -> Score {
    let mut score = Score::default();
    let flute = score.add_part(Instrument::Flute).unwrap();
    for part in [score.parts()[0].id(), flute] {
        score
            .insert_on(
                part,
                StaffId::Upper,
                0,
                0,
                Event {
                    pitches: Pitches::single(Pitch::new(Step::A, 4)),
                    duration: Duration::Whole,
                    notation: Notation::default(),
                },
            )
            .unwrap();
    }
    score
}
fn energies(samples: &[f32]) -> (f64, f64) {
    samples
        .as_chunks::<2>()
        .0
        .iter()
        .fold((0., 0.), |(l, r), s| {
            (l + f64::from(s[0]).powi(2), r + f64::from(s[1]).powi(2))
        })
}
fn controls((mut control, mut renderer): (Control, Renderer), score: &Score) {
    let plan = Arc::new(PlaybackPlan::new(score, Tempo::default(), 48_000).unwrap());
    control.set_reverb(ReverbSettings {
        enabled: false,
        ..Default::default()
    });
    control.set_channel_mix(0, ChannelMix { volume: 0, pan: 0 });
    control.set_channel_mix(1, ChannelMix { volume: 0, pan: 0 });
    control.play(plan.clone()).unwrap();
    let mut samples = [0.; 16384];
    assert_no_alloc::assert_no_alloc(|| renderer.render(&mut samples, 2));
    assert!(
        samples.iter().all(|s| *s == 0.),
        "A muted part must be silent even on its first attack"
    );

    control.stop();
    control.set_channel_mix(
        1,
        ChannelMix {
            volume: 100,
            pan: -100,
        },
    );
    control.play(plan).unwrap();
    assert_no_alloc::assert_no_alloc(|| renderer.render(&mut samples, 2));
    let (left, right) = energies(&samples[8192..]);
    assert!(
        left > 0.001 && left > right * 100.,
        "left pan: {left}/{right}"
    );

    // Change a sustained note, without retriggering the transport.
    let frame = control.position().unwrap().frame;
    control.set_channel_mix(
        1,
        ChannelMix {
            volume: 100,
            pan: 100,
        },
    );
    assert_no_alloc::assert_no_alloc(|| renderer.render(&mut samples, 2));
    let (left, right) = energies(&samples[8192..]);
    assert!(
        right > 0.001 && right > left * 100.,
        "right pan: {left}/{right}"
    );
    assert!(control.position().unwrap().frame > frame);
    control.set_channel_mix(
        1,
        ChannelMix {
            volume: 0,
            pan: 100,
        },
    );
    assert_no_alloc::assert_no_alloc(|| renderer.render(&mut samples, 2));
    assert!(samples[8192..].iter().all(|s| s.abs() < 1e-6));

    control.set_reverb(ReverbSettings::default());
    control.set_channel_mix(1, ChannelMix::default());
    renderer.render(&mut samples, 2);
    control.pause();
    assert_no_alloc::assert_no_alloc(|| renderer.render(&mut samples, 2));
    assert!(samples.iter().all(|s| *s == 0.));
    control.resume();
    renderer.render(&mut samples, 2);
    assert!(samples.iter().any(|s| s.abs() > 1e-4));
    control.stop();
    assert_no_alloc::assert_no_alloc(|| renderer.render(&mut samples, 2));
    assert!(
        samples.iter().all(|s| *s == 0.),
        "Stop must clear the shared room too"
    );
}
#[test]
fn live_mix_controls_independent_gm_channels_and_shared_room_without_allocations() {
    controls(Renderer::new(48_000).unwrap(), &ensemble());
}

#[test]
fn parts_take_stage_seats_except_a_lone_part_and_duplicates_do_not_stack() {
    let mut score = Score::default();
    let piano = score.parts()[0].id();
    assert_eq!(ChannelMix::seated(&score, piano), ChannelMix::default());
    let first = score.add_part(Instrument::Violin).unwrap();
    let second = score.add_part(Instrument::Violin).unwrap();
    let cello = score.add_part(Instrument::Cello).unwrap();
    let pan = |part| ChannelMix::seated(&score, part).pan;
    assert_eq!((pan(first), pan(second), pan(cello)), (-50, -30, 40));
    assert_eq!(ChannelMix::seated(&score, piano).volume, 100);
    assert_eq!(
        ChannelMix::seated(&score, score_core::PartId(999)),
        ChannelMix::default()
    );
}
