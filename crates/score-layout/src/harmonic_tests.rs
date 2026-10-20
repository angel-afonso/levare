use super::*;
use score_core::{Articulation, Event, Harmonic, Notation, Pitch, Pitches, Step};
fn fixture(duration: Duration, harmonic: Harmonic) -> (Score, ElementId) {
    let mut score = Score::default();
    let piano = score.parts()[0].id();
    let part = score.add_part(Instrument::Violin).unwrap();
    score.remove_part(piano).unwrap();
    let id = score
        .insert_on(
            part,
            StaffId::Upper,
            0,
            0,
            Event {
                pitches: Pitches::single(Pitch::new(Step::C, 5)),
                duration,
                notation: Notation {
                    harmonic: Some(harmonic),
                    ..Default::default()
                },
            },
        )
        .unwrap();
    (score, id)
}
#[test]
fn harmonic_glyphs_have_bravura_outlines_and_marks_are_clickable_in_each_view() {
    let face =
        ttf_parser::Face::parse(include_bytes!("../../../assets/fonts/Bravura.otf"), 0).unwrap();
    for harmonic in [Harmonic::Natural, Harmonic::Artificial] {
        assert!(bravura_character(harmonic.glyph().chars().next().unwrap()).is_some());
        assert!(
            face.glyph_bounding_box(
                face.glyph_index(harmonic.glyph().chars().next().unwrap())
                    .unwrap()
            )
            .is_some()
        );
        let (score, id) = fixture(Duration::Half, harmonic);
        for view in [ViewMode::Page, ViewMode::Continuous, ViewMode::SingleSystem] {
            let mut engine = LayoutEngine::default();
            let mut layout = engine.layout_view(&score, 1000., view);
            layout.scale = 0.7;
            let bar = &layout.measures[0];
            let mark = bar
                .content
                .marks
                .iter()
                .find(|m| m.mark == MarkRef::Harmonic(id))
                .unwrap();
            assert_eq!(
                bar.content.glyphs[mark.glyph.unwrap()].text,
                harmonic.glyph()
            );
            assert_eq!(
                bar.content.events.iter().filter(|h| h.id == id).count(),
                1,
                "the touch head isn't a chord tone"
            );
            assert_eq!(
                layout.hit(Point {
                    x: (bar.origin.x + mark.center.x) * layout.scale,
                    y: (bar.origin.y + mark.center.y) * layout.scale
                }),
                Some(Hit::Mark(MarkRef::Harmonic(id)))
            );
        }
    }
}
#[test]
fn artificial_dotted_heads_share_stem_and_beams_clear_the_touch_head() {
    let (mut score, id) = fixture(Duration::Eighth.with_dotted(true), Harmonic::Artificial);
    let event = *score.event(id).unwrap();
    score
        .insert_on(
            id.part,
            id.staff,
            0,
            1,
            Event {
                duration: Duration::Sixteenth,
                ..event
            },
        )
        .unwrap();
    let layout = LayoutEngine::default().layout_view(&score, 1000., ViewMode::Page);
    let bar = &layout.measures[0];
    let touch = bar
        .content
        .marks
        .iter()
        .find(|m| m.mark == MarkRef::Harmonic(id))
        .unwrap();
    let base = bar.content.events.iter().find(|h| h.id == id).unwrap();
    assert_eq!(base.center.y - touch.center.y, 15.);
    assert_eq!(
        bar.content
            .glyphs
            .iter()
            .filter(|g| g.element == Some(id) && g.text == "\u{e1e7}")
            .count(),
        2
    );
    let stem = bar.content.beam_stem(id).unwrap();
    assert!(stem.from.y <= touch.center.y || stem.to.y <= touch.center.y - 30.);
    assert!(stem.from.y >= base.center.y || stem.to.y >= base.center.y + 30.);
}
#[test]
fn natural_circle_clears_articulation_fermata_and_technique() {
    let (mut score, id) = fixture(Duration::Quarter, Harmonic::Natural);
    score
        .set_articulation(id, Some(Articulation::Accent))
        .unwrap();
    score.set_fermata(id, true).unwrap();
    score
        .set_technique(id, Some(score_core::PlayingTechnique::Arco))
        .unwrap();
    let layout = LayoutEngine::default().layout_view(&score, 1000., ViewMode::Page);
    let bar = &layout.measures[0];
    let marks = &bar.content.marks;
    let h = marks
        .iter()
        .find(|m| m.mark == MarkRef::Harmonic(id))
        .unwrap();
    let a = marks
        .iter()
        .find(|m| m.mark == MarkRef::Articulation(id))
        .unwrap();
    let f = marks
        .iter()
        .find(|m| m.mark == MarkRef::Fermata(id))
        .unwrap();
    let t = marks
        .iter()
        .find(|m| m.mark == MarkRef::Technique(id))
        .unwrap();
    let metrics = Metrics::bravura();
    let ink = |mark: &MarkBox| {
        let glyph = &bar.content.glyphs[mark.glyph.unwrap()];
        let (_, top, _, bottom) = metrics.bounds(&glyph.text, false);
        (glyph.position.y + top, glyph.position.y + bottom)
    };
    assert!(ink(h).0 > ink(a).1);
    assert!(ink(a).0 > ink(f).1);
    assert!(ink(f).0 > t.center.y + t.half.1);
}
#[test]
fn touch_accidental_is_a_perfect_fourth_and_is_cancelled_on_later_notes() {
    let (mut score, id) = fixture(Duration::Quarter, Harmonic::Artificial);
    score
        .replace_pitch(
            id,
            0,
            Pitch::new(Step::F, 4)
                .unwrap()
                .with_accidental(Some(score_core::Accidental::Flat)),
        )
        .unwrap();
    let b = score
        .insert_on(
            id.part,
            id.staff,
            0,
            1,
            Event {
                pitches: Pitches::single(Pitch::new(Step::B, 4)),
                duration: Duration::Quarter,
                notation: Notation::default(),
            },
        )
        .unwrap();
    let layout = LayoutEngine::default().layout_view(&score, 1000., ViewMode::Page);
    let glyphs = &layout.measures[0].content.glyphs;
    assert!(
        glyphs
            .iter()
            .any(|g| g.element == Some(id) && g.text == "\u{e264}")
    );
    assert!(
        glyphs
            .iter()
            .any(|g| g.element == Some(b) && g.text == "\u{e261}")
    );
}

#[test]
fn simultaneous_voices_reserve_the_generated_touch_head_and_keep_it_selectable() {
    for (upper_artificial, lower_artificial) in [(true, false), (false, true), (true, true)] {
        for duration in [Duration::Whole, Duration::Quarter, Duration::Eighth] {
            let (mut score, first) = fixture(
                duration,
                if upper_artificial {
                    Harmonic::Artificial
                } else {
                    Harmonic::Natural
                },
            );
            if !upper_artificial {
                score
                    .replace_pitch(first, 0, Pitch::new(Step::F, 5).unwrap())
                    .unwrap();
            }
            let event = Event {
                pitches: Pitches::single(Pitch::new(
                    if lower_artificial { Step::C } else { Step::F },
                    5,
                )),
                duration,
                notation: Notation {
                    harmonic: lower_artificial.then_some(Harmonic::Artificial),
                    ..Default::default()
                },
            };
            let second = score
                .insert_in_voice(first.part, first.staff, 0, Voice::Two, 0, event)
                .unwrap();
            if duration == Duration::Eighth {
                let a = *score.event(first).unwrap();
                score
                    .insert_in_voice(first.part, first.staff, 0, Voice::One, 1, a)
                    .unwrap();
                score
                    .insert_in_voice(first.part, first.staff, 0, Voice::Two, 1, event)
                    .unwrap();
            }
            let layout = LayoutEngine::default().layout_view(&score, 1000., ViewMode::Page);
            let bar = &layout.measures[0];
            for (id, other) in [(first, second), (second, first)] {
                if score.event(id).unwrap().notation.harmonic != Some(Harmonic::Artificial) {
                    continue;
                }
                let mark = bar
                    .content
                    .marks
                    .iter()
                    .find(|m| m.mark == MarkRef::Harmonic(id))
                    .unwrap();
                let head = bar.content.events.iter().find(|h| h.id == other).unwrap();
                assert!(
                    (mark.center.y - head.center.y).abs() >= 10.
                        || (mark.center.x - head.center.x).abs() >= 12.
                );
                assert_eq!(
                    layout.hit(Point {
                        x: bar.origin.x + mark.center.x,
                        y: bar.origin.y + mark.center.y
                    }),
                    Some(Hit::Mark(MarkRef::Harmonic(id)))
                );
            }
        }
    }
}
