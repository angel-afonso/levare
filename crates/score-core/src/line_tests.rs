use super::*;

fn note(step: Step, duration: Duration) -> Event {
    Event {
        pitches: Pitches::single(Pitch::new(step, 5)),
        duration,
        notation: Notation::default(),
    }
}

fn rest() -> Event {
    Event {
        pitches: Pitches::default(),
        ..note(Step::C, Duration::Quarter)
    }
}

fn line(start: ElementId, end: ElementId, kind: ConnectionKind) -> Connection {
    Connection { start, end, kind }
}

#[test]
fn range_lines_cover_one_note_or_a_passage_and_only_some_cover_rests() {
    let mut score = Score::default();
    let a = score
        .insert(0, 0, note(Step::C, Duration::Quarter))
        .unwrap();
    let silence = score.insert(0, 1, rest()).unwrap();
    let b = score
        .insert(0, 2, note(Step::E, Duration::Quarter))
        .unwrap();
    for kind in [
        ConnectionKind::OttavaAlta,
        ConnectionKind::Pedal,
        ConnectionKind::Trill,
    ] {
        score.add_connection(line(a, a, kind)).unwrap();
    }
    // A glissando slides to another note; a trill trills notes only.
    assert!(
        score
            .add_connection(line(a, a, ConnectionKind::Glissando))
            .is_err()
    );
    assert!(
        score
            .add_connection(line(silence, b, ConnectionKind::Trill))
            .is_err()
    );
    assert!(
        score
            .add_connection(line(a, silence, ConnectionKind::Glissando))
            .is_err()
    );
    // An octave line may begin on the rest before its first note.
    score
        .add_connection(line(silence, b, ConnectionKind::OttavaBassa))
        .unwrap();
    score
        .add_connection(line(a, b, ConnectionKind::Glissando))
        .unwrap();
    assert!(
        score
            .add_connection(line(b, a, ConnectionKind::Pedal))
            .is_err()
    );
    score.validate().unwrap();
}

#[test]
fn lines_of_one_kind_never_overlap_but_meet_at_the_next_note() {
    let mut score = Score::default();
    let ids: Vec<_> = (0..4)
        .map(|i| {
            score
                .insert(0, i, note(Step::G, Duration::Quarter))
                .unwrap()
        })
        .collect();
    score
        .add_connection(line(ids[0], ids[1], ConnectionKind::OttavaAlta))
        .unwrap();
    // 8va and 8vb share one lane: they cannot both shift a note.
    assert!(
        score
            .add_connection(line(ids[1], ids[2], ConnectionKind::OttavaBassa))
            .is_err()
    );
    score
        .add_connection(line(ids[2], ids[3], ConnectionKind::OttavaBassa))
        .unwrap();
    score
        .add_connection(line(ids[0], ids[2], ConnectionKind::Pedal))
        .unwrap();
    assert!(
        score
            .add_connection(line(ids[2], ids[3], ConnectionKind::Pedal))
            .is_err()
    );
    score
        .add_connection(line(ids[3], ids[3], ConnectionKind::Pedal))
        .unwrap();
    assert_eq!(score.ottava_steps(ids[1]), 7);
    assert_eq!(score.ottava_steps(ids[3]), -7);
    assert!(score.connection_contains(line(ids[0], ids[2], ConnectionKind::Pedal), ids[1]));
}

#[test]
fn an_octave_line_changes_the_sound_and_the_trill_takes_the_note_above_in_key() {
    let mut score = Score::default();
    let a = score.insert(0, 0, note(Step::B, Duration::Half)).unwrap();
    let written = score.event(a).unwrap().pitches;
    let plain = score.sounding_pitches(a).unwrap().first().unwrap();
    score
        .add_connection(line(a, a, ConnectionKind::OttavaAlta))
        .unwrap();
    assert_eq!(score.event(a).unwrap().pitches, written);
    let high = score.sounding_pitches(a).unwrap().first().unwrap();
    assert_eq!(high.diatonic(), plain.diatonic() + 7);
    // B in C major trills with C, an octave up under the line as well.
    let upper = score.trill_upper(a).unwrap();
    assert_eq!(upper.step, Step::C);
    assert_eq!(upper.diatonic(), high.diatonic() + 1);
}

#[test]
fn erasing_an_end_note_takes_its_lines_with_it() {
    let mut score = Score::default();
    let a = score
        .insert(0, 0, note(Step::C, Duration::Quarter))
        .unwrap();
    let b = score
        .insert(0, 1, note(Step::D, Duration::Quarter))
        .unwrap();
    score
        .add_connection(line(a, b, ConnectionKind::Glissando))
        .unwrap();
    score
        .add_connection(line(b, b, ConnectionKind::Trill))
        .unwrap();
    score.erase(b).unwrap();
    assert!(score.connections().is_empty());
}
