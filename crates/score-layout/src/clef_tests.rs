use super::*;
use score_core::{Event, Notation, Pitch, Pitches, Step};

#[test]
fn clef_changes_draw_inside_systems_move_notes_and_invalidate_cached_measures() {
    let mut score = Score::new("Viola".into(), &[Instrument::Viola, Instrument::Cello], 6).unwrap();
    for p in [PartId(0), PartId(1)] {
        for bar in 0..6 {
            score
                .insert_on(
                    p,
                    StaffId::Upper,
                    bar,
                    0,
                    Event {
                        pitches: Pitches::single(Pitch::new(Step::C, 4)),
                        duration: Duration::Whole,
                        notation: Notation::default(),
                    },
                )
                .unwrap();
        }
    }
    let mut engine = LayoutEngine::default();
    let before = engine.layout(&score, 1400.);
    let old = before.measure(PartId(0), 1).unwrap().content.events[0]
        .center
        .y;
    score
        .set_clef_at(PartId(0), StaffId::Upper, 1, Clef::Treble)
        .unwrap();
    score
        .set_clef_at(PartId(1), StaffId::Upper, 1, Clef::Tenor)
        .unwrap();
    let after = engine.layout(&score, 1400.);
    let viola = after.measure(PartId(0), 1).unwrap();
    let cello = after.measure(PartId(1), 1).unwrap();
    assert_eq!(viola.content.events[0].center.y - old, 30.);
    for (bar, clef, baseline) in [(viola, Clef::Treble, 30.), (cello, Clef::Tenor, 10.)] {
        let glyph = bar
            .content
            .glyphs
            .iter()
            .find(|g| g.element.is_none() && g.text == clef.glyph())
            .unwrap();
        assert_eq!(glyph.position.y, baseline);
        assert!(bar.content.events[0].center.x > glyph.position.x + 30.);
    }
    assert_eq!(
        viola.content.events[0].center.x,
        cello.content.events[0].center.x
    );
    assert_eq!(
        after.measure(PartId(1), 0).unwrap().content.events[0]
            .center
            .y,
        before.measure(PartId(1), 0).unwrap().content.events[0]
            .center
            .y
    );
    score
        .remove_clef_change(PartId(0), StaffId::Upper, 1)
        .unwrap();
    let restored = engine.layout(&score, 1400.);
    assert_eq!(
        restored.measure(PartId(0), 1).unwrap().content.events[0]
            .center
            .y,
        old
    );
}
#[test]
fn clef_changes_leave_room_for_simultaneous_key_time_and_repeat_signs() {
    let mut score = Score::new("Cello".into(), &[Instrument::Cello], 3).unwrap();
    score
        .set_clef_at(PartId(0), StaffId::Upper, 1, Clef::Tenor)
        .unwrap();
    score
        .set_key_signature_at(1, KeySignature::new(2, score_core::KeyMode::Major).unwrap())
        .unwrap();
    score
        .set_time_signature_at(
            1,
            TimeSignature {
                numerator: 3,
                denominator: 4,
            },
        )
        .unwrap();
    score
        .set_bar_form(
            1,
            score_core::BarForm {
                repeat_start: true,
                ..Default::default()
            },
        )
        .unwrap();
    score
        .insert_on(
            PartId(0),
            StaffId::Upper,
            1,
            0,
            Event {
                pitches: Pitches::single(Pitch::new(Step::D, 4)),
                duration: Duration::Quarter,
                notation: Default::default(),
            },
        )
        .unwrap();
    let layout = LayoutEngine::default().layout(&score, 1600.);
    let bar = layout.measure(PartId(0), 1).unwrap();
    let clef = bar
        .content
        .glyphs
        .iter()
        .find(|g| g.text == Clef::Tenor.glyph())
        .unwrap();
    let sharp = bar
        .content
        .glyphs
        .iter()
        .find(|g| g.text == "\u{e262}")
        .unwrap();
    assert!(sharp.position.x > clef.position.x + 25.);
    let right = bar
        .content
        .glyphs
        .iter()
        .filter(|g| g.element.is_none())
        .map(|g| g.position.x + Metrics::bravura().bounds(&g.text, false).2)
        .fold(0., f32::max);
    assert!(bar.content.events[0].center.x > right);
    let repeat = bar
        .content
        .lines
        .iter()
        .filter(|l| {
            l.from.y == 0.
                && l.to.y == 40.
                && l.from.x == l.to.x
                && l.from.x > 0.
                && l.from.x < bar.width - 10.
        })
        .map(|l| l.from.x)
        .fold(f32::INFINITY, f32::min);
    let signature_right = bar
        .content
        .glyphs
        .iter()
        .filter(|g| g.element.is_none() && g.text != "\u{e044}")
        .map(|g| g.position.x + Metrics::bravura().bounds(&g.text, false).2)
        .fold(0., f32::max);
    assert!(repeat > signature_right);
}
