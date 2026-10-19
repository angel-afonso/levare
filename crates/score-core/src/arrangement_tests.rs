use super::*;
#[test]
fn orchestral_order_preserves_instrument_identity_notes_links_and_duplicate_order() {
    let mut score = Score::default();
    let piano = score.parts()[0].id();
    let event = Event {
        pitches: Pitches::single(Pitch::new(Step::C, 4)),
        duration: Duration::Quarter,
        notation: Notation::default(),
    };
    let a = score.insert(0, 0, event).unwrap();
    let b = score.insert(0, 1, event).unwrap();
    score.set_dynamic(a, Some(Dynamic::P)).unwrap();
    score
        .add_connection(Connection {
            start: a,
            end: b,
            kind: ConnectionKind::Tie,
        })
        .unwrap();
    let violin1 = score.add_part(Instrument::Violin).unwrap();
    let cello = score.add_part(Instrument::Cello).unwrap();
    let flute = score.add_part(Instrument::Flute).unwrap();
    let violin2 = score.add_part(Instrument::Violin).unwrap();
    let horn = score.add_part(Instrument::Horn).unwrap();
    assert!(score.arrange_parts());
    assert_eq!(
        score.parts().iter().map(Part::id).collect::<Vec<_>>(),
        [flute, horn, piano, violin1, violin2, cello]
    );
    assert_eq!(score.connections()[0].start, a);
    assert!(score.event(a).is_some());
    assert_eq!(score.event(a).unwrap().notation.dynamic, Some(Dynamic::P));
    assert!(!score.arrange_parts());
    score.validate().unwrap();
}
