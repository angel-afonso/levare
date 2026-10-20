use super::*;

#[test]
#[ignore = "requires the optional local VSCO CE library and sfizz engine"]
fn installed_vsco_renders_real_samples_and_keeps_pause_seek_and_fallback() {
    use score_core::{Duration, Event, Notation, Pitch, Pitches, Step};
    use score_platform::Renderer;
    use score_playback::{PlaybackPlan, Tempo};
    use std::sync::Arc;
    let manifest =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../soundpacks/vsco-ce/manifest.json");
    let pack = VscoPack::load(&manifest).unwrap();
    let mut score = Score::default();
    score
        .insert(
            0,
            0,
            Event {
                pitches: Pitches::single(Pitch::new(Step::A, 4)),
                duration: Duration::Whole,
                notation: Notation::default(),
            },
        )
        .unwrap();
    let plan = Arc::new(PlaybackPlan::new(&score, Tempo::default(), 48_000).unwrap());
    let (mut control, mut native) =
        Renderer::with_bank(48_000, &pack, &score, Instrument::Piano).unwrap();
    let (mut gm_control, mut gm) = Renderer::new(48_000).unwrap();
    native.offline();
    control.play(plan.clone()).unwrap();
    gm_control.play(plan.clone()).unwrap();
    let mut actual = [0f32; 4096];
    let mut reference = [0f32; 4096];
    assert_no_alloc::assert_no_alloc(|| {
        native.render(&mut actual, 2);
        gm.render(&mut reference, 2);
    });
    assert!(actual.iter().any(|v| v.abs() > 0.0001));
    assert_ne!(
        actual, reference,
        "VSCO must render different recordings, not silently use GM"
    );
    let frame = control.position().unwrap().frame;
    control.pause();
    assert_no_alloc::assert_no_alloc(|| native.render(&mut actual, 2));
    assert!(actual.iter().all(|v| *v == 0.));
    assert_eq!(control.position().unwrap().frame, frame);
    control.play_from_frame(plan, 24_000).unwrap();
    assert_no_alloc::assert_no_alloc(|| native.render(&mut actual, 2));
    assert!(actual.iter().all(|v| *v == 0.));
    assert_eq!(control.position().unwrap().frame, 24_000);
    control.resume();
    assert_no_alloc::assert_no_alloc(|| native.render(&mut actual, 2));
    assert!(actual.iter().any(|v| v.abs() > 0.0001));
    control.stop();
    assert_no_alloc::assert_no_alloc(|| native.render(&mut actual, 2));
    assert!(actual.iter().all(|v| *v == 0.));

    // Guitar is not in VSCO. Both renderers must use the identical GM fallback.
    let piano = score.parts()[0].id();
    let guitar = score.add_part(Instrument::Guitar).unwrap();
    score.remove_part(piano).unwrap();
    score
        .insert_on(
            guitar,
            score_core::StaffId::Upper,
            0,
            0,
            Event {
                pitches: Pitches::single(Pitch::new(Step::E, 4)),
                duration: Duration::Quarter,
                notation: Notation::default(),
            },
        )
        .unwrap();
    let plan = Arc::new(PlaybackPlan::new(&score, Tempo::default(), 48_000).unwrap());
    let (mut control, mut native) =
        Renderer::with_bank(48_000, &pack, &score, Instrument::Guitar).unwrap();
    let (mut gm_control, mut gm) = Renderer::new(48_000).unwrap();
    control.play(plan.clone()).unwrap();
    gm_control.play(plan).unwrap();
    assert_no_alloc::assert_no_alloc(|| {
        native.render(&mut actual, 2);
        gm.render(&mut reference, 2);
    });
    assert_eq!(actual, reference);
}

#[test]
#[ignore = "requires the optional local VSCO CE library and sfizz engine"]
fn repeated_flutes_have_independent_native_voices_and_strings_use_pizzicato() {
    use score_core::{Duration, Event, Notation, Pitch, Pitches, PlayingTechnique, StaffId, Step};
    use score_platform::Renderer;
    use score_playback::{PlaybackPlan, Tempo};
    use std::sync::Arc;
    let manifest =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../soundpacks/vsco-ce/manifest.json");
    let pack = VscoPack::load(&manifest).unwrap();
    let note = |duration, technique| Event {
        pitches: Pitches::single(Pitch::new(Step::A, 4)),
        duration,
        notation: Notation {
            technique,
            ..Default::default()
        },
    };
    let mut score = Score::default();
    let piano = score.parts()[0].id();
    let first = score.add_part(Instrument::Flute).unwrap();
    let second = score.add_part(Instrument::Flute).unwrap();
    score.remove_part(piano).unwrap();
    score
        .insert_on(first, StaffId::Upper, 0, 0, note(Duration::Whole, None))
        .unwrap();
    score
        .insert_on(second, StaffId::Upper, 0, 0, note(Duration::Quarter, None))
        .unwrap();
    let plan = Arc::new(PlaybackPlan::new(&score, Tempo::default(), 48_000).unwrap());
    let (mut control, mut renderer) =
        Renderer::with_bank(48_000, &pack, &score, Instrument::Flute).unwrap();
    renderer.offline();
    control.play(plan).unwrap();
    let mut output = [0f32; 2048];
    // At 1.5s the short flute's release is over; the other must still sound.
    assert_no_alloc::assert_no_alloc(|| {
        for _ in 0..72 {
            renderer.render(&mut output, 2);
        }
    });
    assert!(output.iter().map(|v| v * v).sum::<f32>() > 0.001);

    let mut strings = Score::default();
    let piano = strings.parts()[0].id();
    let violin = strings.add_part(Instrument::Violin).unwrap();
    strings.remove_part(piano).unwrap();
    strings
        .insert_on(
            violin,
            StaffId::Upper,
            0,
            0,
            note(Duration::Whole, Some(PlayingTechnique::Pizzicato)),
        )
        .unwrap();
    let (mut control, mut renderer) =
        Renderer::with_bank(48_000, &pack, &strings, Instrument::Violin).unwrap();
    control
        .play(Arc::new(
            PlaybackPlan::new(&strings, Tempo::default(), 48_000).unwrap(),
        ))
        .unwrap();
    // Live mode, within the preloaded portion, exercises the actual callback path.
    assert_no_alloc::assert_no_alloc(|| {
        for _ in 0..8 {
            renderer.render(&mut output, 2);
        }
    });
    assert!(output.iter().map(|v| v * v).sum::<f32>() > 0.001);
}

#[test]
#[ignore = "requires the optional local VSCO CE library and sfizz engine"]
fn installed_vsco_preserves_the_integrated_harmonic_timbre() {
    use score_core::{
        Duration, Event, Harmonic, Notation, Pitch, Pitches, PlayingTechnique, StaffId, Step,
    };
    use score_platform::Renderer;
    use score_playback::{PlaybackPlan, Tempo};
    use std::sync::Arc;
    let pack = VscoPack::load(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("../../soundpacks/vsco-ce/manifest.json"),
    )
    .unwrap();
    for technique in [PlayingTechnique::Arco, PlayingTechnique::Pizzicato] {
        let mut score = Score::default();
        let piano = score.parts()[0].id();
        let part = score.add_part(Instrument::Violin).unwrap();
        score.remove_part(piano).unwrap();
        score
            .insert_on(
                part,
                StaffId::Upper,
                0,
                0,
                Event {
                    pitches: Pitches::single(Pitch::new(Step::A, 5)),
                    duration: Duration::Whole,
                    notation: Notation {
                        harmonic: Some(Harmonic::Natural),
                        technique: Some(technique),
                        ..Default::default()
                    },
                },
            )
            .unwrap();
        let plan = Arc::new(PlaybackPlan::new(&score, Tempo::default(), 48_000).unwrap());
        let (mut optional_control, mut optional) =
            Renderer::with_bank(48_000, &pack, &score, Instrument::Violin).unwrap();
        let (mut direct_control, mut direct) = Renderer::new(48_000).unwrap();
        optional.offline();
        optional_control.play(plan.clone()).unwrap();
        direct_control.play(plan).unwrap();
        let mut actual = [0f32; 8192];
        let mut expected = actual;
        assert_no_alloc::assert_no_alloc(|| {
            optional.render(&mut actual, 2);
            direct.render(&mut expected, 2);
        });
        assert_eq!(
            actual, expected,
            "VSCO CE must decline harmonic attacks instead of playing a normal recording"
        );
        assert!(actual.iter().any(|v| v.abs() > 0.001));
    }
}
