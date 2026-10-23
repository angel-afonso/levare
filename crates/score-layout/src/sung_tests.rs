use super::*;
use score_core::{
    ChordSymbol, Dynamic, Event, Instrument, Join, MarkRef, Notation, Pitch, PlayingTechnique,
    Step, Syllable,
};

fn note(step: Step, octave: u8) -> Event {
    Event {
        pitches: score_core::Pitches::single(Pitch::new(step, octave)),
        duration: Duration::Quarter,
        notation: Notation::default(),
    }
}

fn sing(score: &mut Score, id: ElementId, verse: usize, text: &str, join: Join) {
    score
        .set_syllable(id, verse, Some(Syllable::new(text, join).unwrap()))
        .unwrap();
}

fn lyrics(layout: &Layout) -> Vec<(f32, f32, &LyricLabel)> {
    layout
        .measures
        .iter()
        .flat_map(|m| {
            m.content
                .lyrics
                .iter()
                .map(move |l| (m.origin.x + l.position.x, m.origin.y + l.position.y, l))
        })
        .collect()
}

#[test]
fn each_verse_keeps_one_line_under_low_and_high_notes_with_dynamics_below() {
    let mut score = Score::new("".into(), &[Instrument::Soprano], 1).unwrap();
    let ids: Vec<_> = [(Step::C, 4), (Step::A, 5), (Step::D, 4), (Step::E, 5)]
        .into_iter()
        .enumerate()
        .map(|(i, (step, octave))| score.insert(0, i, note(step, octave)).unwrap())
        .collect();
    sing(&mut score, ids[0], 0, "A", Join::Hyphen);
    sing(&mut score, ids[1], 0, "ve", Join::Word);
    sing(&mut score, ids[2], 0, "Ma", Join::Hyphen);
    sing(&mut score, ids[3], 0, "ría", Join::Word);
    sing(&mut score, ids[0], 1, "Sal", Join::Word);
    score.set_dynamic(ids[0], Some(Dynamic::P)).unwrap();
    let layout = LayoutEngine::default().layout_compact(&score, 680.);
    let words = lyrics(&layout);
    let first: Vec<_> = words.iter().filter(|w| w.2.verse == 0).collect();
    assert_eq!(first.len(), 4);
    assert!(
        first.iter().all(|w| w.1 == first[0].1),
        "verse 1 on one line"
    );
    let second = words.iter().find(|w| w.2.verse == 1).unwrap();
    assert!((second.1 - first[0].1 - 20.).abs() < 0.1);
    let m = layout.measure_on(ids[0].part, ids[0].staff, 0).unwrap();
    let lowest_note = m
        .content
        .events
        .iter()
        .map(|e| m.origin.y + e.center.y)
        .fold(f32::NEG_INFINITY, f32::max);
    assert!(first[0].1 - 14. > lowest_note, "the words clear the low C");
    let dynamic = m
        .content
        .marks
        .iter()
        .find(|mark| mark.mark == MarkRef::Dynamic(ids[0]))
        .unwrap();
    assert!(
        m.origin.y + dynamic.center.y - dynamic.half.1 > second.1,
        "p under the words"
    );
    // Two hyphens: «A-ve» and «Ma-ría».
    assert_eq!(layout.lyric_lines.len(), 2);
    for line in &layout.lyric_lines {
        assert!((line.from.y - (first[0].1 - 4.)).abs() < 0.1);
    }
    let (x, y, label) = first[1];
    let at = Point {
        x: x + label.width() / 2.,
        y: y - 5.,
    };
    assert_eq!(layout.hit(at), Some(Hit::Mark(MarkRef::Lyric(ids[1], 0))));
    assert!(layout.systems[0].bottom >= second.1);
}

#[test]
fn a_held_syllable_draws_its_extender_to_the_last_note() {
    let mut score = Score::default();
    let ids: Vec<_> = (0..4)
        .map(|i| score.insert(0, i, note(Step::G, 4)).unwrap())
        .collect();
    sing(&mut score, ids[0], 0, "Ah", Join::Melisma);
    sing(&mut score, ids[3], 0, "sí", Join::Word);
    let layout = LayoutEngine::default().layout_compact(&score, 680.);
    let m = layout.measure_on(ids[0].part, ids[0].staff, 0).unwrap();
    let head = |id: ElementId| {
        m.origin.x
            + m.content
                .events
                .iter()
                .find(|e| e.id == id)
                .unwrap()
                .center
                .x
    };
    let [extender] = &layout.lyric_lines[..] else {
        panic!("{:?}", layout.lyric_lines);
    };
    assert!(extender.to.x > head(ids[2]) && extender.to.x < head(ids[3]));
}

#[test]
fn chord_symbols_take_the_outer_lane_above_techniques_and_answer_clicks() {
    let mut score = Score::new("".into(), &[Instrument::Violin], 1).unwrap();
    let a = score.insert(0, 0, note(Step::A, 5)).unwrap();
    let b = score.insert(0, 1, note(Step::B, 5)).unwrap();
    score
        .set_technique(a, Some(PlayingTechnique::Pizzicato))
        .unwrap();
    score
        .set_chord(a, Some(ChordSymbol::parse("Bbmaj7").unwrap()))
        .unwrap();
    score
        .set_chord(b, Some(ChordSymbol::parse("Am").unwrap()))
        .unwrap();
    let layout = LayoutEngine::default().layout_compact(&score, 680.);
    let m = layout.measure_on(a.part, a.staff, 0).unwrap();
    let label = |mark: MarkRef| {
        m.content
            .techniques
            .iter()
            .find(|label| label.mark == mark)
            .unwrap()
    };
    let (chord, pizz, other) = (
        label(MarkRef::Chord(a)),
        label(MarkRef::Technique(a)),
        label(MarkRef::Chord(b)),
    );
    assert_eq!(chord.text, "B♭maj7");
    assert!(chord.position.y <= pizz.position.y - 20.);
    assert_eq!(chord.position.y, other.position.y, "one line of chords");
    let at = Point {
        x: m.origin.x + other.position.x + 6.,
        y: m.origin.y + other.position.y - 6.,
    };
    assert_eq!(layout.hit(at), Some(Hit::Mark(MarkRef::Chord(b))));
    assert!(layout.systems[0].top <= m.origin.y + chord.position.y - 16.);
}
