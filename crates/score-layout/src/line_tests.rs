use super::*;
use score_core::{
    Connection, ConnectionKind as K, Event, Instrument, MarkRef, Notation, Pitch, Step,
};

fn note(step: Step, octave: u8, duration: Duration) -> Event {
    Event {
        pitches: score_core::Pitches::single(Pitch::new(step, octave)),
        duration,
        notation: Notation::default(),
    }
}

/// Six bars of whole notes, long enough to wrap at 680 units.
fn passage() -> (Score, Vec<ElementId>) {
    let mut score = Score::default();
    for _ in 0..5 {
        score.append_measure().unwrap();
    }
    let ids = (0..6)
        .map(|i| {
            score
                .insert(i, 0, note(Step::E, 5, Duration::Whole))
                .unwrap()
        })
        .collect();
    (score, ids)
}

fn add(score: &mut Score, start: ElementId, end: ElementId, kind: K) -> Connection {
    let c = Connection { start, end, kind };
    score.add_connection(c).unwrap();
    c
}

fn marks(layout: &Layout, kind: K) -> Vec<&LineMark> {
    layout
        .line_marks
        .iter()
        .filter(|line| line.connection.kind == kind)
        .collect()
}

fn ink_rows(layout: &Layout, line: &LineMark) -> (f32, f32) {
    layout
        .measures
        .iter()
        .filter(|m| m.origin.y == line.staff_origin)
        .fold((f32::INFINITY, f32::NEG_INFINITY), |(top, bottom), m| {
            (
                top.min(m.origin.y + m.content.ink_top),
                bottom.max(m.origin.y + m.content.ink_bottom),
            )
        })
}

#[test]
fn an_octave_line_wraps_with_its_label_once_and_hooks_at_its_end() {
    let (mut score, ids) = passage();
    add(&mut score, ids[0], ids[5], K::OttavaAlta);
    let layout = LayoutEngine::default().layout_compact(&score, 680.);
    let pieces = marks(&layout, K::OttavaAlta);
    assert!(pieces.len() > 1, "{}", pieces.len());
    for (i, line) in pieces.iter().enumerate() {
        let (top, _) = ink_rows(&layout, line);
        let (_, line_top, _, line_bottom) = line.ink();
        assert!(line_bottom < top, "the 8va crosses its notes");
        assert!(line.from.x < line.to.x);
        // The label opens the line; later systems repeat it.
        assert_eq!(line.glyphs.len(), 1);
        assert_eq!(line.glyphs[0].text, "\u{e511}");
        let last = line.strokes.last().unwrap();
        let hooked = last.from.x == last.to.x && last.to.y > last.from.y;
        assert_eq!(hooked, i + 1 == pieces.len());
        assert!(layout.systems.iter().any(|s| s.top <= line_top));
    }
}

#[test]
fn pedal_stands_below_an_octave_bassa_and_both_below_the_notes() {
    let (mut score, ids) = passage();
    add(&mut score, ids[0], ids[1], K::OttavaBassa);
    add(&mut score, ids[0], ids[1], K::Pedal);
    let layout = LayoutEngine::default().layout_compact(&score, 680.);
    let (low, pedal) = (
        marks(&layout, K::OttavaBassa)[0],
        marks(&layout, K::Pedal)[0],
    );
    let (_, bottom) = ink_rows(&layout, low);
    assert!(low.ink().1 > bottom);
    assert!(pedal.ink().1 >= low.ink().3, "the pedal overlaps the 8vb");
    assert_eq!(pedal.glyphs[0].text, "\u{e650}");
    // The pedal's release hooks up at the end of its last note.
    let release = pedal.strokes.last().unwrap();
    assert!(release.to.y < release.from.y);
    let system = &layout.systems[0];
    assert!(system.bottom >= pedal.ink().3);
}

#[test]
fn a_short_trill_is_the_sign_and_a_long_one_adds_the_wavy_line_under_an_8va() {
    let mut score = Score::default();
    let short = score
        .insert(0, 0, note(Step::G, 4, Duration::Quarter))
        .unwrap();
    let long = score
        .insert(0, 1, note(Step::A, 4, Duration::Half.with_dotted(true)))
        .unwrap();
    add(&mut score, short, short, K::Trill);
    add(&mut score, long, long, K::Trill);
    add(&mut score, long, long, K::OttavaAlta);
    let layout = LayoutEngine::default().layout_compact(&score, 680.);
    let trills = marks(&layout, K::Trill);
    let glyphs = |line: &LineMark| {
        line.glyphs
            .iter()
            .map(|g| g.text.clone())
            .collect::<Vec<_>>()
    };
    let first = trills.iter().find(|t| t.connection.start == short).unwrap();
    assert_eq!(glyphs(first), ["\u{e566}"]);
    let second = trills.iter().find(|t| t.connection.start == long).unwrap();
    assert_eq!(second.glyphs.len(), 2);
    assert!(second.glyphs[1].text.chars().all(|c| c == '\u{eaa4}'));
    assert!(second.glyphs[1].text.chars().count() >= 2);
    let octave = marks(&layout, K::OttavaAlta)[0];
    assert!(
        octave.ink().3 <= second.ink().1,
        "the 8va overlaps the trill"
    );
}

#[test]
fn a_glissando_rises_from_head_to_head_and_lines_answer_clicks() {
    let mut score = Score::default();
    let low = score
        .insert(0, 0, note(Step::C, 4, Duration::Half))
        .unwrap();
    let high = score
        .insert(0, 1, note(Step::C, 5, Duration::Half))
        .unwrap();
    let slide = add(&mut score, low, high, K::Glissando);
    let pedal = add(&mut score, low, high, K::Pedal);
    let layout = LayoutEngine::default().layout_compact(&score, 680.);
    let line = marks(&layout, K::Glissando)[0];
    assert!(line.from.y > line.to.y);
    let m = layout.measure_on(low.part, low.staff, 0).unwrap();
    let head = |id: ElementId| m.content.events.iter().find(|e| e.id == id).unwrap().center;
    assert!(line.from.x > m.origin.x + head(low).x);
    assert!(line.to.x < m.origin.x + head(high).x);
    let middle = Point {
        x: (line.from.x + line.to.x) / 2.,
        y: (line.from.y + line.to.y) / 2.,
    };
    assert_eq!(
        layout.hit(middle),
        Some(Hit::Mark(MarkRef::Connection(slide)))
    );
    let (l, t, r, b) = marks(&layout, K::Pedal)[0].areas[0];
    let label = Point {
        x: (l + r) / 2.,
        y: (t + b) / 2.,
    };
    assert_eq!(
        layout.hit(label),
        Some(Hit::Mark(MarkRef::Connection(pedal)))
    );
}

#[test]
fn every_line_glyph_is_in_the_painted_repertoire_and_staves_keep_clear() {
    let mut score =
        Score::new("Líneas".into(), &[Instrument::Flute, Instrument::Violin], 2).unwrap();
    let mut ids = Vec::new();
    for part in score.parts().iter().map(|p| p.id()).collect::<Vec<_>>() {
        let id = score
            .insert_in(part, 0, 0, note(Step::E, 4, Duration::Whole))
            .unwrap();
        ids.push(id);
    }
    for &id in &ids {
        for kind in [K::Trill, K::OttavaAlta] {
            add(&mut score, id, id, kind);
        }
    }
    add(&mut score, ids[0], ids[0], K::Pedal);
    let codes: Vec<_> = glyph_codes().collect();
    for layout in [
        LayoutEngine::default().layout(&score, 900.),
        LayoutEngine::default().layout_compact(&score, 900.),
    ] {
        for line in &layout.line_marks {
            for c in line.glyphs.iter().flat_map(|g| g.text.chars()) {
                assert!(codes.contains(&u32::from(c)), "{c:?}");
            }
        }
        // The flute's pedal stays above everything the violin writes.
        let pedal = marks(&layout, K::Pedal)[0];
        let violin = layout
            .line_marks
            .iter()
            .filter(|line| line.connection.start.part == ids[1].part)
            .map(|line| line.ink().1)
            .chain(
                layout
                    .measures
                    .iter()
                    .filter(|m| m.part == ids[1].part)
                    .map(|m| m.origin.y + m.content.ink_top),
            )
            .fold(f32::INFINITY, f32::min);
        assert!(pedal.ink().3 < violin, "{} >= {violin}", pedal.ink().3);
    }
}
