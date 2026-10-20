use super::*;
use score_core::{Duration, Event, KeyMode, KeySignature, Notation};
#[test]
fn full_orchestra_has_independent_channels_and_written_notes_sound_in_concert_pitch() {
    let mut instruments = vec![
        Instrument::ClarinetBb,
        Instrument::HornF,
        Instrument::Piccolo,
        Instrument::DoubleBass,
        Instrument::SnareDrum,
    ];
    instruments.resize(score_core::MAX_PARTS, Instrument::Violin);
    let mut score = Score::new("Orquesta".into(), &instruments, 2).unwrap();
    score
        .set_key_signature_at(1, KeySignature::new(1, KeyMode::Major).unwrap())
        .unwrap();
    let ids: Vec<_> = score.parts().iter().map(|p| p.id()).collect();
    let mut elements = Vec::new();
    for part in ids {
        for bar in 0..2 {
            elements.push(
                score
                    .insert_on(
                        part,
                        StaffId::Upper,
                        bar,
                        0,
                        Event {
                            pitches: Pitches::single(
                                Pitch::new(if bar == 0 { Step::C } else { Step::F }, 4).map(|p| {
                                    if bar == 0 {
                                        p.with_accidental(Some(score_core::Accidental::Natural))
                                    } else {
                                        p
                                    }
                                }),
                            ),
                            duration: Duration::Whole,
                            notation: Notation::default(),
                        },
                    )
                    .unwrap(),
            );
        }
    }
    let plan = PlaybackPlan::new(&score, Tempo::default(), 48_000).unwrap();
    let channels: std::collections::HashSet<_> = plan.tracks.iter().map(|t| t.channel).collect();
    assert_eq!(channels.len(), score_core::MAX_PARTS);
    for channel in channels {
        assert!(usize::from(channel) + 16 < PLAYBACK_CHANNELS);
        assert!(![9, 15].contains(&(channel % 16)));
    }
    let keys: Vec<_> = plan
        .messages
        .iter()
        .filter_map(|m| {
            if m.frame == 0 {
                if let Message::NoteOn(n) = m.message {
                    Some(n.key)
                } else {
                    None
                }
            } else {
                None
            }
        })
        .collect();
    assert_eq!(&keys[..5], &[58, 53, 72, 48, 38]);
    for id in elements {
        let audition = Audition::for_element(&score, id, Tempo::default(), 48_000).unwrap();
        let span = plan.spans.iter().find(|s| s.element == id).unwrap();
        assert_eq!(audition.keys[0], span.pitches.first().map(midi_key));
    }
}
