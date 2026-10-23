use super::*;
use score_core::{Duration, Event, GraceKind, Notation, Pitch, Pitches, Step};

fn attacks(score: &Score) -> Vec<(u64, u8)> {
    let plan = PlaybackPlan::new(score, Tempo::default(), 48_000).unwrap();
    plan.messages
        .iter()
        .filter_map(|message| match message.message {
            Message::NoteOn(note) => Some((message.frame, note.key)),
            _ => None,
        })
        .collect()
}

fn decorated(kind: GraceKind, notes: usize) -> Score {
    let mut score = Score::default();
    let id = score
        .insert(
            0,
            0,
            Event {
                pitches: Pitches::single(Pitch::new(Step::D, 5)),
                duration: Duration::Quarter,
                notation: Notation::default(),
            },
        )
        .unwrap();
    score.set_grace(id, Some(kind)).unwrap();
    score.transpose_grace_note(id, 0, -1).unwrap();
    for _ in 1..notes {
        score.add_grace_note(id).unwrap();
    }
    score.set_grace(id, Some(kind)).unwrap();
    score
}

#[test]
fn an_acciaccatura_is_crushed_onto_the_beat_and_the_note_follows() {
    assert_eq!(
        attacks(&decorated(GraceKind::Acciaccatura, 1)),
        [(0, 72), (3_360, 74)]
    );
}

#[test]
fn an_appoggiatura_takes_half_of_its_note_shared_by_the_group() {
    assert_eq!(
        attacks(&decorated(GraceKind::Appoggiatura, 1)),
        [(0, 72), (12_000, 74)]
    );
    assert_eq!(
        attacks(&decorated(GraceKind::Appoggiatura, 2)),
        [(0, 72), (6_000, 72), (12_000, 74)]
    );
}
