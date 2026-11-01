use super::*;
use score_core::{
    Connection, ConnectionKind, Duration, Event, Instrument, Notation, Pitch, Pitches, Step,
};

fn note(step: Step, octave: u8, duration: Duration) -> Event {
    Event {
        pitches: Pitches::single(Pitch::new(step, octave)),
        duration,
        notation: Notation::default(),
    }
}

fn plan(score: &Score) -> PlaybackPlan {
    PlaybackPlan::new(score, Tempo::default(), 48_000).unwrap()
}

fn ons(plan: &PlaybackPlan) -> Vec<(u64, u8)> {
    plan.messages
        .iter()
        .filter_map(|m| match m.message {
            Message::NoteOn(note) => Some((m.frame, note.key)),
            _ => None,
        })
        .collect()
}

fn offs(plan: &PlaybackPlan, key: u8) -> Vec<u64> {
    plan.messages
        .iter()
        .filter_map(|m| (m.message == Message::NoteOff(key)).then_some(m.frame))
        .collect()
}

fn line(score: &mut Score, start: ElementId, end: ElementId, kind: ConnectionKind) {
    score
        .add_connection(Connection { start, end, kind })
        .unwrap();
}

#[test]
fn octave_lines_move_the_sound_and_leave_the_writing() {
    let mut score = Score::new("".into(), &[Instrument::Violin], 2).unwrap();
    let a = score
        .insert(0, 0, note(Step::C, 5, Duration::Quarter))
        .unwrap();
    let b = score
        .insert(0, 1, note(Step::D, 5, Duration::Quarter))
        .unwrap();
    line(&mut score, a, a, ConnectionKind::OttavaAlta);
    assert_eq!(ons(&plan(&score)), [(0, 84), (24_000, 74)]);
    score.remove_connection(Connection {
        start: a,
        end: a,
        kind: ConnectionKind::OttavaAlta,
    });
    line(&mut score, a, b, ConnectionKind::OttavaBassa);
    assert_eq!(ons(&plan(&score)), [(0, 60), (24_000, 62)]);
    assert_eq!(
        score.event(a).unwrap().pitches,
        Pitches::single(Pitch::new(Step::C, 5))
    );
}

#[test]
fn the_pedal_holds_notes_to_its_release_and_repeats_restrike() {
    let mut score = Score::default();
    let a = score
        .insert(0, 0, note(Step::C, 4, Duration::Quarter))
        .unwrap();
    let b = score
        .insert(0, 1, note(Step::E, 4, Duration::Quarter))
        .unwrap();
    let c = score
        .insert(0, 2, note(Step::C, 4, Duration::Quarter))
        .unwrap();
    line(&mut score, a, c, ConnectionKind::Pedal);
    let plan = plan(&score);
    // C4 is released just before it is struck again; E4 rings to the pedal's end.
    assert_eq!(offs(&plan, 60)[0], 47_999);
    assert_eq!(offs(&plan, 64), [72_000]);
    let _ = b;
}

#[test]
fn a_trill_alternates_with_the_note_above_in_the_key() {
    let mut score = Score::new("".into(), &[Instrument::Flute], 1).unwrap();
    let a = score
        .insert(0, 0, note(Step::D, 5, Duration::Half))
        .unwrap();
    line(&mut score, a, a, ConnectionKind::Trill);
    let keys: Vec<u8> = ons(&plan(&score)).into_iter().map(|(_, key)| key).collect();
    assert!(keys.len() >= 10);
    assert!(
        keys.chunks(2)
            .all(|pair| pair[0] == 74 && pair.get(1).is_none_or(|k| *k == 76))
    );
}

#[test]
fn glissandos_run_chromatically_or_on_white_keys() {
    let mut violin = Score::new("".into(), &[Instrument::Violin], 1).unwrap();
    let a = violin
        .insert(0, 0, note(Step::C, 5, Duration::Quarter))
        .unwrap();
    let b = violin
        .insert(0, 1, note(Step::G, 5, Duration::Quarter))
        .unwrap();
    line(&mut violin, a, b, ConnectionKind::Glissando);
    let keys: Vec<u8> = ons(&plan(&violin))
        .into_iter()
        .map(|(_, key)| key)
        .collect();
    assert_eq!(keys, [72, 73, 74, 75, 76, 77, 78, 79]);

    let mut piano = Score::default();
    let a = piano
        .insert(0, 0, note(Step::C, 5, Duration::Quarter))
        .unwrap();
    let b = piano
        .insert(0, 1, note(Step::G, 5, Duration::Quarter))
        .unwrap();
    line(&mut piano, a, b, ConnectionKind::Glissando);
    let keys: Vec<u8> = ons(&plan(&piano)).into_iter().map(|(_, key)| key).collect();
    assert_eq!(keys, [72, 74, 76, 77, 79]);
}

#[test]
fn mordents_and_turns_play_their_neighbours_in_the_key_then_the_note() {
    use score_core::{KeyMode, KeySignature, Ornament};
    for (ornament, keys) in [
        (Ornament::Mordent, &[79, 78, 79][..]),
        (Ornament::InvertedMordent, &[79, 81, 79][..]),
        (Ornament::Turn, &[81, 79, 78, 79][..]),
    ] {
        let mut score = Score::new("".into(), &[Instrument::Flute], 1).unwrap();
        score.set_key_signature(KeySignature::new(1, KeyMode::Major).unwrap());
        let a = score
            .insert(0, 0, note(Step::G, 5, Duration::Half))
            .unwrap();
        score.set_ornament(a, Some(ornament)).unwrap();
        let ons = ons(&plan(&score));
        assert_eq!(
            ons.iter().map(|(_, key)| *key).collect::<Vec<_>>(),
            keys,
            "{ornament:?}"
        );
        // Quick notes of 70 ms; the note takes the rest of its half.
        let quick = 48_000 * 70 / 1000;
        for (index, (frame, _)) in ons.iter().enumerate() {
            assert_eq!(*frame, quick * index as u64, "{ornament:?}");
        }
    }
}

#[test]
fn a_breath_or_caesura_cuts_the_note_and_its_slur_before_the_next() {
    use score_core::Pause;
    let release = |pause: Option<Pause>| {
        let mut score = Score::new("".into(), &[Instrument::Flute], 1).unwrap();
        let a = score
            .insert(0, 0, note(Step::C, 5, Duration::Quarter))
            .unwrap();
        let b = score
            .insert(0, 1, note(Step::D, 5, Duration::Quarter))
            .unwrap();
        line(&mut score, a, b, ConnectionKind::Slur);
        score.set_pause(a, pause).unwrap();
        offs(&plan(&score), 72)[0]
    };
    // Slurred, the note reaches into the next one.
    assert!(release(None) > 24_000);
    assert_eq!(release(Some(Pause::Breath)), 18_000);
    assert_eq!(release(Some(Pause::Caesura)), 12_000);
}

#[test]
fn sfz_and_fp_strike_their_note_and_leave_the_level_they_name() {
    use score_core::Dynamic;
    let mut score = Score::default();
    let ids: Vec<_> = (0..4)
        .map(|index| {
            score
                .insert(0, index, note(Step::C, 4, Duration::Quarter))
                .unwrap()
        })
        .collect();
    score.set_dynamic(ids[0], Some(Dynamic::P)).unwrap();
    score.set_dynamic(ids[1], Some(Dynamic::Sfz)).unwrap();
    score.set_dynamic(ids[2], Some(Dynamic::Fp)).unwrap();
    let levels: Vec<u8> = plan(&score)
        .messages
        .iter()
        .filter_map(|m| match m.message {
            Message::NoteOn(note) => Some(note.level),
            _ => None,
        })
        .collect();
    let (p, f) = (Dynamic::P.velocity(), Dynamic::F.velocity());
    // Small metric weights shape every attack alike, so compare with p.
    assert!(
        levels[1] >= f - 4 && levels[1] > levels[0] + 30,
        "{levels:?}"
    );
    assert!(levels[2] >= f - 4, "{levels:?}");
    assert!(levels[3].abs_diff(p) <= 6, "{levels:?}");
}
