use super::*;
use crate::{Renderer, mixer::ReverbSettings};
use score_core::{Articulation, Duration, Dynamic, Event, Notation, Pitch, Pitches, Step};
use score_playback::{Audition, PlaybackPlan, Tempo};
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};

struct Probe {
    attack: AtomicU64,
    count: AtomicU64,
}
struct Bank {
    accept: bool,
    probe: Arc<Probe>,
}
impl Bank {
    fn new(accept: bool) -> Self {
        Self {
            accept,
            probe: Arc::new(Probe {
                attack: AtomicU64::new(0),
                count: AtomicU64::new(0),
            }),
        }
    }
}
struct Engine {
    accept: bool,
    probe: Arc<Probe>,
    held: [bool; 4096],
}
fn attack(channel: u8, key: u8, level: u8, articulation: Option<Articulation>) -> u64 {
    u64::from(channel)
        | (u64::from(key) << 8)
        | (u64::from(level) << 16)
        | (articulation.map_or(0, |a| a as u64 + 1) << 24)
}
impl SoundBank for Bank {
    fn id(&self) -> &str {
        "test-bank"
    }
    fn name(&self) -> &str {
        "Test bank"
    }
    fn description(&self) -> &str {
        "Contract test"
    }
    fn prepare(
        &self,
        _: &Score,
        _: Instrument,
        _: u32,
    ) -> Result<Box<dyn SoundEngine>, AudioError> {
        Ok(Box::new(Engine {
            accept: self.accept,
            probe: self.probe.clone(),
            held: [false; 4096],
        }))
    }
}
impl SoundEngine for Engine {
    fn midi(&mut self, _: u8, _: u8, _: u8, _: u8) {}
    fn note_on(&mut self, c: u8, note: NoteEvent) -> bool {
        if !self.accept {
            return false;
        }
        self.held[usize::from(c) * 128 + usize::from(note.key)] = true;
        self.probe.attack.store(
            attack(c, note.key, note.level, note.articulation),
            Ordering::Relaxed,
        );
        self.probe.count.fetch_add(1, Ordering::Relaxed);
        true
    }
    fn note_off(&mut self, c: u8, key: u8) {
        self.held[usize::from(c) * 128 + usize::from(key)] = false;
    }
    fn silence(&mut self, channel: Option<u8>, _: bool) {
        if let Some(c) = channel {
            self.held[usize::from(c) * 128..usize::from(c) * 128 + 128].fill(false);
        } else {
            self.held.fill(false);
        }
    }
    fn render(&mut self, left: &mut [f32], right: &mut [f32]) {
        if self.held.iter().any(|h| *h) {
            for (l, r) in left.iter_mut().zip(right) {
                *l += 0.04;
                *r += 0.02;
            }
        }
    }
}
fn score() -> (Score, score_core::ElementId) {
    let mut score = Score::default();
    let id = score
        .insert(
            0,
            0,
            Event {
                pitches: Pitches::single(Pitch::new(Step::A, 4)),
                duration: Duration::Quarter,
                notation: Notation {
                    dynamic: Some(Dynamic::P),
                    articulation: Some(Articulation::Staccato),
                    ..Default::default()
                },
            },
        )
        .unwrap();
    (score, id)
}

#[test]
fn contract_preserves_preview_gate_and_style_and_seek_chases_the_same_attack() {
    let (score, id) = score();
    let bank = Bank::new(true);
    let (mut control, mut renderer) =
        Renderer::with_bank(48_000, &bank, &score, Instrument::Piano).unwrap();
    control.set_volume(100);
    control.set_reverb(ReverbSettings {
        enabled: false,
        ..Default::default()
    });
    let audition = Audition::for_element(&score, id, Tempo::default(), 48_000).unwrap();
    assert_eq!(audition.duration_frames, 6000);
    control.preview_audition(audition).unwrap();
    let mut samples = [0f32; 12000];
    assert_no_alloc::assert_no_alloc(|| renderer.render(&mut samples, 2));
    assert_eq!(
        bank.probe.attack.load(Ordering::Relaxed),
        attack(15, 69, 56, Some(Articulation::Staccato))
    );
    assert!(
        samples
            .as_chunks::<2>()
            .0
            .iter()
            .all(|s| *s == [0.04, 0.02]),
        "An accepted attack must not be doubled by GM"
    );
    renderer.render(&mut samples, 2);
    assert!(samples.iter().all(|s| *s == 0.));
    let plan = Arc::new(PlaybackPlan::new(&score, Tempo::default(), 48_000).unwrap());
    control.play_from_frame(plan, 4000).unwrap();
    renderer.render(&mut samples[..128], 2);
    assert_eq!(
        bank.probe.attack.load(Ordering::Relaxed),
        attack(0, 69, 56, Some(Articulation::Staccato))
    );
    control.pause();
    let count = bank.probe.count.load(Ordering::Relaxed);
    assert_no_alloc::assert_no_alloc(|| renderer.render(&mut samples, 2));
    assert!(samples.iter().all(|s| *s == 0.));
    control.resume();
    renderer.render(&mut samples[..128], 2);
    assert_eq!(bank.probe.count.load(Ordering::Relaxed), count);
    control.stop();
    assert_no_alloc::assert_no_alloc(|| renderer.render(&mut samples, 2));
    assert!(samples.iter().all(|s| *s == 0.));
}

#[test]
fn declined_attack_keeps_general_midi_fallback_identical() {
    let (score, id) = score();
    let bank = Bank::new(false);
    let (mut control, mut extra) =
        Renderer::with_bank(48_000, &bank, &score, Instrument::Piano).unwrap();
    let (mut plain_control, mut plain) = Renderer::new(48_000).unwrap();
    let audition = Audition::for_element(&score, id, Tempo::default(), 48_000).unwrap();
    control.preview_audition(audition).unwrap();
    plain_control.preview_audition(audition).unwrap();
    let mut actual = [0f32; 8192];
    let mut expected = actual;
    assert_no_alloc::assert_no_alloc(|| {
        extra.render(&mut actual, 2);
        plain.render(&mut expected, 2);
    });
    assert_eq!(actual, expected);
    assert!(actual.iter().any(|s| s.abs() > 1e-4));
}

#[test]
fn gm_audition_dynamics_and_accent_change_actual_audio_without_optional_banks() {
    let energy = |dynamic, articulation| {
        let (mut score, id) = score();
        score.set_dynamic(id, Some(dynamic)).unwrap();
        score.set_articulation(id, articulation).unwrap();
        let (mut control, mut renderer) = Renderer::new(48_000).unwrap();
        control
            .preview_audition(Audition::for_element(&score, id, Tempo::default(), 48_000).unwrap())
            .unwrap();
        let mut samples = [0f32; 4096];
        assert_no_alloc::assert_no_alloc(|| renderer.render(&mut samples, 2));
        samples.iter().map(|s| s * s).sum::<f32>()
    };
    let quiet = energy(Dynamic::P, None);
    assert!(energy(Dynamic::F, None) > quiet * 2.);
    assert!(energy(Dynamic::P, Some(Articulation::Accent)) > quiet * 1.3);
}

#[test]
fn harmonic_timbre_reaches_fallback_audition_playback_pause_and_seek() {
    use score_core::{Harmonic, PlayingTechnique, StaffId};
    let mut score = Score::default();
    let piano = score.parts()[0].id();
    let part = score.add_part(Instrument::Violin).unwrap();
    score.remove_part(piano).unwrap();
    let id = score
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
                    ..Default::default()
                },
            },
        )
        .unwrap();
    let bank = Bank::new(false);
    let (mut fallback_control, mut fallback) =
        Renderer::with_bank(48_000, &bank, &score, Instrument::Violin).unwrap();
    let (mut direct_control, mut direct) = Renderer::new(48_000).unwrap();
    let preview = Audition::for_element(&score, id, Tempo::default(), 48_000).unwrap();
    assert_eq!(preview.note_at(0).harmonic, Some(Harmonic::Natural));
    fallback_control.preview_audition(preview).unwrap();
    direct_control.preview_audition(preview).unwrap();
    let mut actual = [0f32; 8192];
    let mut expected = actual;
    assert_no_alloc::assert_no_alloc(|| {
        fallback.render(&mut actual, 2);
        direct.render(&mut expected, 2);
    });
    assert_eq!(
        actual, expected,
        "an unsupported optional provider uses the same harmonic timbre"
    );
    assert!(actual.iter().any(|v| v.abs() > 0.001));
    let harmonic = actual;
    let (mut plain_control, mut plain) = Renderer::new(48_000).unwrap();
    let mut normal = preview;
    normal.harmonic = None;
    plain_control.preview_audition(normal).unwrap();
    plain.render(&mut expected, 2);
    assert_ne!(
        harmonic, expected,
        "the same sounding key must change timbre"
    );
    // A sustained harmonic keeps its phase and envelope across a pause.
    let plan = Arc::new(PlaybackPlan::new(&score, Tempo::default(), 48_000).unwrap());
    let (mut a_control, mut a) = Renderer::new(48_000).unwrap();
    let (mut b_control, mut b) = Renderer::new(48_000).unwrap();
    a_control.play(plan.clone()).unwrap();
    b_control.play(plan.clone()).unwrap();
    a.render(&mut actual, 2);
    b.render(&mut expected, 2);
    assert_eq!(actual, expected);
    a_control.pause();
    assert_no_alloc::assert_no_alloc(|| a.render(&mut actual, 2));
    assert!(actual.iter().all(|v| *v == 0.));
    a_control.resume();
    a.render(&mut actual, 2);
    b.render(&mut expected, 2);
    assert_eq!(
        actual, expected,
        "resume continues the same oscillator rather than retriggering"
    );
    a_control.play_from_frame(plan, 24_000).unwrap();
    assert_no_alloc::assert_no_alloc(|| a.render(&mut actual, 2));
    assert!(actual.iter().any(|v| v.abs() > 0.001));
    a_control.stop();
    a.render(&mut actual, 2);
    assert!(actual.iter().all(|v| *v == 0.));
    // Natural/artificial notation shares timbre at the same sounding pitch.
    let mut artificial = preview;
    artificial.harmonic = Some(Harmonic::Artificial);
    assert_eq!(artificial.note_at(0).technique, PlayingTechnique::Arco);
    assert_eq!(artificial.note_at(0).harmonic, Some(Harmonic::Artificial));
}

#[test]
fn releasing_one_timbre_does_not_cut_a_same_pitch_normal_or_harmonic_voice() {
    use score_core::{Harmonic, PlayingTechnique};
    for release_harmonic in [false, true] {
        let normal = NoteEvent {
            key: 81,
            level: 88,
            instrument: Instrument::Violin,
            technique: PlayingTechnique::Arco,
            harmonic: None,
            articulation: None,
            legato: false,
            tremolo: None,
        };
        let harmonic = NoteEvent {
            harmonic: Some(Harmonic::Natural),
            ..normal
        };
        let mut mixed = GeneralMidi::new(48_000).unwrap();
        let mut reference = GeneralMidi::new(48_000).unwrap();
        mixed.note_on(0, normal);
        mixed.note_on(0, harmonic);
        reference.note_on(0, if release_harmonic { normal } else { harmonic });
        for block in 0..1100 {
            if block == 50 {
                if release_harmonic {
                    mixed.harmonic_off(0, 81);
                } else {
                    mixed.note_off(0, 81);
                }
            }
            let mut actual = [0.; 64];
            let mut right = actual;
            let mut expected = actual;
            let mut expected_right = actual;
            assert_no_alloc::assert_no_alloc(|| {
                mixed.render(&mut actual, &mut right);
                reference.render(&mut expected, &mut expected_right);
            });
            if block > 1000 {
                let error = actual
                    .iter()
                    .zip(expected)
                    .map(|(a, b)| (a - b).abs())
                    .fold(0., f32::max);
                assert!(
                    error < 1e-5,
                    "release of harmonic={release_harmonic} damaged the surviving timbre: {error}"
                );
            }
        }
    }
}
