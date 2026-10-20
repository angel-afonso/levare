use super::*;
use score_core::{
    Connection, ConnectionKind, Duration, Event, Harmonic, KeyMode, KeySignature, Notation,
};
#[test]
fn artificial_harmonic_has_one_sounding_attack_and_matching_audition_and_chase() {
    for technique in [PlayingTechnique::Arco, PlayingTechnique::Pizzicato] {
        let mut score = Score::default();
        let part = score.add_part(Instrument::Violin).unwrap();
        score
            .set_key_signature_at(0, KeySignature::new(1, KeyMode::Major).unwrap())
            .unwrap();
        let mut ids = Vec::new();
        for i in 0..3 {
            ids.push(
                score
                    .insert_on(
                        part,
                        StaffId::Upper,
                        0,
                        i,
                        Event {
                            pitches: Pitches::single(Pitch::new(Step::F, 4)),
                            duration: Duration::Quarter,
                            notation: Notation {
                                harmonic: Some(if i < 2 {
                                    Harmonic::Artificial
                                } else {
                                    Harmonic::Natural
                                }),
                                technique: Some(technique),
                                ..Default::default()
                            },
                        },
                    )
                    .unwrap(),
            );
        }
        score
            .add_connection(Connection {
                start: ids[0],
                end: ids[1],
                kind: ConnectionKind::Tie,
            })
            .unwrap();
        let plan = PlaybackPlan::new(&score, Tempo::default(), 48_000).unwrap();
        let attacks: Vec<_> = plan
            .messages
            .iter()
            .filter_map(|m| {
                if let Message::NoteOn(note) = m.message {
                    Some(note)
                } else {
                    None
                }
            })
            .collect();
        assert_eq!(
            attacks.iter().map(|note| note.key).collect::<Vec<_>>(),
            [90, 66]
        );
        assert!(attacks.iter().all(|note| note.technique == technique));
        assert_eq!(attacks[0].harmonic, Some(Harmonic::Artificial));
        assert_eq!(attacks[1].harmonic, Some(Harmonic::Natural));
        assert_eq!(plan.sounds.len(), 2);
        assert_eq!(
            plan.sounds[0].written_end,
            plan.timeline.frame_at(96, 48_000)
        );
        let audition = Audition::for_element(&score, ids[0], Tempo::default(), 48_000).unwrap();
        assert_eq!(
            audition.keys.iter().flatten().copied().collect::<Vec<_>>(),
            [90]
        );
        assert_eq!(audition.technique, technique);
        assert_eq!(audition.note_at(0).harmonic, Some(Harmonic::Artificial));
        let natural = Audition::for_element(&score, ids[2], Tempo::default(), 48_000).unwrap();
        assert_eq!(natural.keys[0], Some(66));
        let chase = plan.held_notes_at(plan.timeline.frame_at(48, 48_000));
        assert!(
            chase
                .iter()
                .flatten()
                .any(|m| m.note.key == 90 && m.note.harmonic == Some(Harmonic::Artificial))
        );
    }
}

#[test]
fn normal_and_harmonic_unisons_keep_both_attacks_and_independent_releases_and_chase() {
    let mut score = Score::default();
    let piano = score.parts()[0].id();
    let part = score.add_part(Instrument::Violin).unwrap();
    score.remove_part(piano).unwrap();
    let event = Event {
        pitches: Pitches::single(Pitch::new(Step::A, 5)),
        duration: Duration::Quarter,
        notation: Notation::default(),
    };
    score
        .insert_in_voice(part, StaffId::Upper, 0, score_core::Voice::One, 0, event)
        .unwrap();
    score
        .insert_in_voice(
            part,
            StaffId::Upper,
            0,
            score_core::Voice::Two,
            0,
            Event {
                duration: Duration::Whole,
                notation: Notation {
                    harmonic: Some(Harmonic::Natural),
                    ..Default::default()
                },
                ..event
            },
        )
        .unwrap();
    let plan = PlaybackPlan::new(&score, Tempo::default(), 48_000).unwrap();
    assert_eq!(
        plan.messages
            .iter()
            .filter(|m| matches!(m.message, Message::NoteOn(_)))
            .count(),
        2
    );
    let early = plan.held_notes_at(plan.timeline.frame_at(24, 48_000));
    assert_eq!(early.iter().flatten().count(), 2);
    assert!(early.iter().flatten().any(|n| n.note.harmonic.is_none()));
    assert!(early.iter().flatten().any(|n| n.note.harmonic.is_some()));
    let late = plan.held_notes_at(plan.timeline.frame_at(96, 48_000));
    assert_eq!(late.iter().flatten().count(), 1);
    assert!(late.iter().flatten().all(|n| n.note.harmonic.is_some()));
}
