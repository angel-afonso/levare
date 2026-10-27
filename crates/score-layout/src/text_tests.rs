use super::*;
use score_core::{
    Connection, ConnectionKind, Dynamic, Event, Instrument, MarkRef, Notation, Pitch,
    PlayingTechnique, Step, TextKind,
};

fn note(step: Step) -> Event {
    Event {
        pitches: score_core::Pitches::single(Pitch::new(step, 4)),
        duration: Duration::Quarter,
        notation: Notation::default(),
    }
}

fn label(layout: &Layout, mark: MarkRef) -> (&PlacedMeasure, &TechniqueLabel) {
    layout
        .measures
        .iter()
        .find_map(|m| {
            m.content
                .techniques
                .iter()
                .chain(&m.content.expressions)
                .find(|label| label.mark == mark)
                .map(|label| (m, label))
        })
        .unwrap()
}

#[test]
fn an_instruction_stands_above_and_an_expression_follows_its_dynamic_below() {
    let mut score = Score::new("".into(), &[Instrument::Violin], 1).unwrap();
    let a = score.insert(0, 0, note(Step::G)).unwrap();
    score.set_dynamic(a, Some(Dynamic::Mp)).unwrap();
    score
        .set_technique(a, Some(PlayingTechnique::Pizzicato))
        .unwrap();
    score
        .set_text(a, TextKind::Technique, Some("sul pont."))
        .unwrap();
    score
        .set_text(a, TextKind::Expression, Some("dolce"))
        .unwrap();
    let layout = LayoutEngine::default().layout_compact(&score, 680.);
    let (m, above) = label(&layout, MarkRef::Text(a, TextKind::Technique));
    assert!(above.position.y < 0., "above the staff");
    let (_, pizz) = label(&layout, MarkRef::Technique(a));
    assert!(
        (above.position.y - pizz.position.y).abs() >= 20.,
        "pizz. and the instruction share a lane"
    );
    let (_, below) = label(&layout, MarkRef::Text(a, TextKind::Expression));
    assert_eq!(below.text, "dolce");
    let dynamic = m
        .content
        .marks
        .iter()
        .find(|mark| mark.mark == MarkRef::Dynamic(a))
        .unwrap();
    assert!(below.position.x > dynamic.center.x + dynamic.half.0 - 4.);
    assert!((below.position.y - 5. - dynamic.center.y).abs() < 1.);
    // Clicking the words selects them alone.
    let at = |label: &TechniqueLabel| Point {
        x: m.origin.x + label.position.x + label.width() / 2.,
        y: m.origin.y + label.position.y - 5.,
    };
    assert_eq!(
        layout.hit(at(below)),
        Some(Hit::Mark(MarkRef::Text(a, TextKind::Expression)))
    );
    assert_eq!(
        layout.hit(at(above)),
        Some(Hit::Mark(MarkRef::Text(a, TextKind::Technique)))
    );
    let system = &layout.systems[0];
    assert!(system.top <= m.origin.y + above.position.y - 16.);
    assert!(system.bottom >= m.origin.y + below.position.y);
}

#[test]
fn a_hairpin_leaves_room_for_the_words_on_its_lane() {
    let mut score = Score::default();
    let ids: Vec<_> = (0..4)
        .map(|i| score.insert(0, i, note(Step::C)).unwrap())
        .collect();
    score
        .set_text(ids[1], TextKind::Expression, Some("espress."))
        .unwrap();
    score
        .add_connection(Connection {
            start: ids[0],
            end: ids[3],
            kind: ConnectionKind::Crescendo,
        })
        .unwrap();
    let layout = LayoutEngine::default().layout_compact(&score, 680.);
    let (m, words) = label(&layout, MarkRef::Text(ids[1], TextKind::Expression));
    let (left, right) = (
        m.origin.x + words.position.x,
        m.origin.x + words.position.x + words.width(),
    );
    assert!(
        layout.hairpins.len() > 1,
        "the wedge is split around the words"
    );
    for hairpin in &layout.hairpins {
        assert!(hairpin.upper.to.x <= left || hairpin.upper.from.x >= right);
    }
}

#[test]
fn moved_words_carry_their_offset_and_their_box() {
    let mut score = Score::default();
    let a = score.insert(0, 0, note(Step::E)).unwrap();
    score
        .set_text(a, TextKind::Technique, Some("col legno"))
        .unwrap();
    let mark = MarkRef::Text(a, TextKind::Technique);
    let before = LayoutEngine::default().layout_compact(&score, 680.);
    let start = label(&before, mark).1.position;
    score
        .set_mark_offset(mark, score_core::Offset { dx: 30, dy: -20 })
        .unwrap();
    let after = LayoutEngine::default().layout_compact(&score, 680.);
    let moved = label(&after, mark).1.position;
    assert_eq!((moved.x - start.x, moved.y - start.y), (30., -20.));
}
