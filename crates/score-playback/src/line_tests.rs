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
