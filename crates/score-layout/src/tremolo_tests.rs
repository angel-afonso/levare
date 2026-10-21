use super::*;
use score_core::{Event, Notation, Pitch, Pitches, Step, Tremolo};

fn fixture(duration: Duration, octave: u8) -> (Score, ElementId) {
    let mut score = Score::new("Tremolo".into(), &[Instrument::Violin], 4).unwrap();
    let id = score
        .insert(
            0,
            0,
            Event {
                pitches: Pitches::single(Pitch::new(Step::C, octave)),
                duration,
                notation: Notation {
                    tremolo: Some(Tremolo::Three),
                    ..Default::default()
                },
            },
        )
        .unwrap();
    (score, id)
}
fn check_stem(layout: &Layout, id: ElementId, metrics: &Metrics) {
    let bar = layout
        .measures
        .iter()
        .find(|bar| bar.part == id.part && bar.index == id.measure)
        .unwrap();
    let stem = bar
        .content
        .lines
        .iter()
        .find(|line| {
            line.element == Some(id) && line.thickness == metrics.stem && line.from.x == line.to.x
        })
        .unwrap();
    let glyph = bar
        .content
        .glyphs
        .iter()
        .find(|glyph| glyph.element == Some(id) && glyph.text == Tremolo::Three.glyph())
        .unwrap();
    let (left, top, right, bottom) = metrics.bounds(&glyph.text, false);
    assert!((glyph.position.x + (left + right) / 2. - stem.from.x).abs() < 0.001);
    let center = glyph.position.y + (top + bottom) / 2.;
    assert!((center - (stem.from.y + (stem.to.y - stem.from.y) * 0.55)).abs() < 0.001);
    assert!(center > stem.from.y.min(stem.to.y) && center < stem.from.y.max(stem.to.y));
}
#[test]
fn tremolo_strokes_cross_the_stem_between_head_and_tip() {
    let metrics = Metrics::bravura();
    for octave in [4, 6] {
        let (score, id) = fixture(Duration::Quarter, octave);
        let layout = LayoutEngine::default().layout(&score, 1000.);
        check_stem(&layout, id, &metrics);
    }
}
#[test]
fn beamed_eighths_keep_their_tremolo_on_the_beam_stem() {
    let (mut score, id) = fixture(Duration::Eighth, 4);
    let second = score.insert(0, 1, *score.event(id).unwrap()).unwrap();
    let layout = LayoutEngine::default().layout(&score, 1000.);
    assert!(!layout.measures[0].content.beams.is_empty());
    for element in [id, second] {
        check_stem(&layout, element, &Metrics::bravura());
    }
}
#[test]
fn whole_note_tremolo_sits_above_the_head() {
    let (score, id) = fixture(Duration::Whole, 4);
    let layout = LayoutEngine::default().layout(&score, 1000.);
    let bar = &layout.measures[0];
    let head = bar
        .content
        .events
        .iter()
        .find(|head| head.id == id)
        .unwrap();
    let glyph = bar
        .content
        .glyphs
        .iter()
        .find(|glyph| glyph.text == Tremolo::Three.glyph())
        .unwrap();
    let (left, _, right, bottom) = Metrics::bravura().bounds(&glyph.text, false);
    assert!((glyph.position.x + (left + right) / 2. - head.center.x).abs() < 0.001);
    assert!((glyph.position.y + bottom - (head.center.y - 14.)).abs() < 0.001);
    assert!(
        bar.content.ink_top <= glyph.position.y + Metrics::bravura().bounds(&glyph.text, false).1
    );
}
#[test]
fn tremolo_is_selectable_as_a_mark() {
    let (score, id) = fixture(Duration::Quarter, 4);
    for mode in [ViewMode::Page, ViewMode::Continuous, ViewMode::SingleSystem] {
        let mut layout = LayoutEngine::default().layout_view(&score, 1000., mode);
        layout.scale = 0.7;
        let bar = &layout.measures[0];
        let mark = bar
            .content
            .marks
            .iter()
            .find(|mark| mark.mark == MarkRef::Tremolo(id))
            .unwrap();
        assert_eq!(
            layout.hit(Point {
                x: (bar.origin.x + mark.center.x) * layout.scale,
                y: (bar.origin.y + mark.center.y) * layout.scale
            }),
            Some(Hit::Mark(MarkRef::Tremolo(id)))
        );
    }
}
