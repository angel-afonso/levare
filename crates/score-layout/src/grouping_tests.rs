use super::*;
use score_core::{Event, Notation, Pitch, Pitches, Step};
#[test]
fn reference_score_gets_family_brackets_and_barline_gaps_on_every_system() {
    let mut s = Score::default();
    let piano = s.parts()[0].id();
    let cello = s.add_part(Instrument::Cello).unwrap();
    let violin1 = s.add_part(Instrument::Violin).unwrap();
    let flute = s.add_part(Instrument::Flute).unwrap();
    let viola = s.add_part(Instrument::Viola).unwrap();
    s.add_part(Instrument::Violin).unwrap();
    s.remove_part(piano).unwrap();
    s.arrange_parts();
    let id = s
        .insert_on(
            violin1,
            StaffId::Upper,
            0,
            0,
            Event {
                pitches: Pitches::single(Pitch::new(Step::G, 4)),
                duration: Duration::Quarter,
                notation: Notation::default(),
            },
        )
        .unwrap();
    let mut engine = LayoutEngine::default();
    let layout = engine.layout_compact(&s, 680.);
    assert!(layout.braces.is_empty());
    assert_eq!(layout.brackets.len(), 4); // two systems, two families
    for row in [0, 2] {
        let wind = layout.measure(flute, row).unwrap();
        let strings = layout.measure(violin1, row).unwrap();
        let bass = layout.measure(cello, row).unwrap();
        assert!(
            layout
                .brackets
                .iter()
                .any(|b| b.family == InstrumentFamily::Woodwinds
                    && b.top == wind.origin.y
                    && b.bottom == wind.origin.y + 40.)
        );
        assert!(
            layout
                .brackets
                .iter()
                .any(|b| b.family == InstrumentFamily::Strings
                    && b.top == strings.origin.y
                    && b.bottom == bass.origin.y + 40.)
        );
        let x = strings.origin.x + strings.width;
        assert!(layout.connectors.iter().any(|l| l.from.x == x
            && l.from.y == strings.origin.y
            && l.to.y == bass.origin.y + 40.));
        assert!(
            !layout.connectors.iter().any(|l| l.from.x == x
                && l.from.y < strings.origin.y
                && l.to.y >= strings.origin.y)
        );
    }
    let bar = layout.measure(violin1, 0).unwrap();
    let head = bar.content.events.iter().find(|e| e.id == id).unwrap();
    let p = Point {
        x: bar.origin.x + head.center.x,
        y: bar.origin.y + head.center.y,
    };
    assert_eq!(layout.hit_test(p), Some(id));
    s.remove_part(cello).unwrap();
    let layout = engine.layout_compact(&s, 680.);
    let last = layout.measure(viola, 0).unwrap();
    assert_eq!(
        layout
            .brackets
            .iter()
            .find(|b| b.family == InstrumentFamily::Strings)
            .unwrap()
            .bottom,
        last.origin.y + 40.
    );
}
#[test]
fn piano_braces_remain_inside_the_ensemble_and_solo_has_no_family_bracket() {
    let mut s = Score::default();
    let piano = s.parts()[0].id();
    let mut engine = LayoutEngine::default();
    let solo = engine.layout(&s, 960.);
    assert_eq!(solo.braces.len(), 1);
    assert!(solo.brackets.is_empty());
    let flute = s.add_part(Instrument::Flute).unwrap();
    s.arrange_parts();
    let layout = engine.layout_compact(&s, 960.);
    assert_eq!(layout.braces.len(), 1);
    assert_eq!(layout.brackets.len(), 1);
    let upper = layout.measure(piano, 0).unwrap();
    let lower = layout.measure_on(piano, StaffId::Lower, 0).unwrap();
    let brace = &layout.braces[0];
    assert_eq!(brace.top, upper.origin.y);
    assert_eq!(brace.bottom, lower.origin.y + 40.);
    assert!(layout.measure(flute, 0).unwrap().origin.y < brace.top);
}
