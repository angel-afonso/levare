// The expected runs are lists of bar ranges, often only one.
#![allow(clippy::single_range_in_vec_init)]
use super::*;
use score_core::{
    BarForm, BarNumbers, Barline, Dynamic, Ending, Event, Instrument, Jump, KeyMode, LayoutBreak,
    Notation, Numbering, PageSetup, Pitch, Pitches, Step, TempoMark, Words,
};
use std::ops::Range;

fn note() -> Event {
    Event {
        pitches: Pitches::single(Pitch::new(Step::F, 4)),
        duration: Duration::Quarter,
        notation: Notation::default(),
    }
}

fn rest(notation: Notation) -> Event {
    Event {
        pitches: Pitches::default(),
        duration: Duration::Whole,
        notation,
    }
}

/// A violin part of `bars` bars with a note in each bar of `played`.
fn violin(bars: usize, played: &[usize]) -> Score {
    let mut score = Score::new("Partes".into(), &[Instrument::Violin], bars).unwrap();
    for &bar in played {
        score.insert(bar, 0, note()).unwrap();
    }
    score
}

fn part(score: &Score) -> Layout {
    LayoutEngine::default().layout_part(score, 1000., ViewMode::Page)
}

fn runs(score: &Score) -> Vec<Range<usize>> {
    let layout = part(score);
    assert_eq!(layout.multirests, score.multirests(score.parts()[0].id()));
    layout.multirests
}

fn h_bar(measure: &PlacedMeasure) -> bool {
    measure
        .content
        .lines
        .iter()
        .any(|line| line.thickness == STAFF_SPACE && line.from.y == line.to.y)
}

fn count(measure: &PlacedMeasure) -> Option<&str> {
    measure
        .content
        .glyphs
        .iter()
        .find(|glyph| {
            glyph.position.y < 0.
                && glyph
                    .text
                    .chars()
                    .all(|c| ('\u{e080}'..='\u{e089}').contains(&c))
        })
        .map(|glyph| glyph.text.as_str())
}

#[test]
fn empty_bars_of_a_part_become_one_multirest_with_its_count() {
    let mut score = violin(12, &[0, 11]);
    let id = score.parts()[0].id();
    score
        .set_numbering(Numbering {
            bars: BarNumbers::EveryBar,
            ..Default::default()
        })
        .unwrap();
    let mut engine = LayoutEngine::default();
    let layout = engine.layout_part(&score, 1000., ViewMode::Page);
    assert_eq!(layout.multirests, [1..11]);
    // The run takes one place in the system and draws one H-bar with «10».
    assert_eq!(layout.systems.len(), 1);
    let first = layout.measure(id, 1).unwrap();
    assert!(h_bar(first));
    assert_eq!(count(first), Some(multirest::count_text(10).as_str()));
    assert_eq!(multirest::count_text(10), "\u{e081}\u{e080}");
    assert!(!first.content.glyphs.iter().any(|g| g.text == "\u{e4e3}"));
    for bar in 2..11 {
        let gathered = layout.measure(id, bar).unwrap();
        assert!(layout.gathered(bar));
        assert_eq!(gathered.origin.x, first.origin.x);
        assert_eq!(gathered.width, first.width);
        assert!(gathered.content.lines.is_empty() && gathered.content.glyphs.is_empty());
        // The caret and the playhead still find every bar, on the H-bar.
        let caret = layout
            .voice_cursor(id, StaffId::Upper, bar, Voice::One, 0)
            .unwrap();
        assert!(caret.x > first.origin.x && caret.x < first.origin.x + first.width);
        let capacity = f64::from(score.time_at(bar).capacity());
        assert!(gathered.content.x_at_tick(0.) < gathered.content.x_at_tick(capacity));
    }
    // A click on the H-bar is in its first bar.
    let point = Point {
        x: first.origin.x + first.width / 2.,
        y: first.origin.y + 20.,
    };
    assert_eq!(layout.measure_at(point), Some((id, StaffId::Upper, 1)));
    // The next bar follows the run and keeps its own number.
    let next = layout.measure(id, 11).unwrap();
    assert!((next.origin.x - (first.origin.x + first.width)).abs() < 0.01);
    let numbered: Vec<_> = layout.numbered_bars().map(|m| m.index).collect();
    assert_eq!(numbered, [1, 11]);
    // The full score never gathers bars.
    let full = LayoutEngine::default().layout_view(&score, 1000., ViewMode::Page);
    assert!(full.multirests.is_empty());
    assert!(!full.measures.iter().any(h_bar));
    // Writing in a gathered bar splits the run at once, with the same engine.
    score.insert(5, 0, note()).unwrap();
    let edited = engine.layout_part(&score, 1000., ViewMode::Page);
    assert_eq!(edited.multirests, [1..5, 6..11]);
    assert_eq!(
        count(edited.measure(id, 1).unwrap()),
        Some(multirest::count_text(4).as_str())
    );
    assert!(!h_bar(edited.measure(id, 5).unwrap()));
    assert!(!edited.measure(id, 5).unwrap().content.events.is_empty());
}

#[test]
fn a_long_rest_counts_as_one_bar_in_its_system_in_every_view() {
    let score = violin(60, &[0, 59]);
    for mode in [ViewMode::Page, ViewMode::Continuous, ViewMode::SingleSystem] {
        let layout = LayoutEngine::default().layout_part(&score, 1000., mode);
        assert_eq!(layout.multirests, [1..59], "{mode:?}");
        assert_eq!(layout.systems.len(), 1, "{mode:?}");
        assert_eq!(layout.measures.len(), 60);
    }
    // One bar alone is a bar, not a multi-measure rest.
    assert_eq!(runs(&violin(4, &[0, 2])), Vec::<Range<usize>>::new());
    assert_eq!(runs(&violin(5, &[0, 2])), [3..5]);
}

#[test]
fn whatever_a_player_must_see_at_a_bar_breaks_the_run() {
    let tempo = |score: &Score| TempoMark {
        text: "Più mosso".into(),
        ..score.tempo_at(0).clone()
    };
    let words = |text: &str| Words::new(text).unwrap();
    type Change = Box<dyn Fn(&mut Score)>;
    let id = |score: &Score| score.parts()[0].id();
    // Shown at the start of bar 5: a new run begins there.
    let starts: Vec<(&str, Change)> = vec![
        (
            "time",
            Box::new(|s| {
                s.set_time_signature_at(
                    5,
                    TimeSignature {
                        numerator: 3,
                        denominator: 4,
                    },
                )
                .map(|_| ())
                .unwrap()
            }),
        ),
        (
            "key",
            Box::new(|s| {
                s.set_key_signature_at(5, KeySignature::new(2, KeyMode::Major).unwrap())
                    .map(|_| ())
                    .unwrap()
            }),
        ),
        (
            "clef",
            Box::new(move |s| {
                let part = id(s);
                s.set_clef_at(part, StaffId::Upper, 5, Clef::Alto)
                    .map(|_| ())
                    .unwrap()
            }),
        ),
        (
            "tempo",
            Box::new(move |s| {
                let mark = tempo(s);
                s.set_tempo_at(5, mark).map(|_| ()).unwrap()
            }),
        ),
        (
            "rehearsal",
            Box::new(|s| s.toggle_rehearsal_mark(5).map(|_| ()).unwrap()),
        ),
        (
            "repeat start",
            Box::new(|s| {
                s.set_bar_form(
                    5,
                    BarForm {
                        repeat_start: true,
                        ..Default::default()
                    },
                )
                .map(|_| ())
                .unwrap()
            }),
        ),
        (
            "repeat end",
            Box::new(|s| {
                s.set_bar_form(
                    4,
                    BarForm {
                        repeat_end: Some(2),
                        ..Default::default()
                    },
                )
                .map(|_| ())
                .unwrap()
            }),
        ),
        (
            "double barline",
            Box::new(|s| {
                s.set_bar_form(
                    4,
                    BarForm {
                        barline: Barline::Double,
                        ..Default::default()
                    },
                )
                .map(|_| ())
                .unwrap()
            }),
        ),
        (
            "final barline",
            Box::new(|s| {
                s.set_bar_form(
                    4,
                    BarForm {
                        barline: Barline::Final,
                        ..Default::default()
                    },
                )
                .map(|_| ())
                .unwrap()
            }),
        ),
        (
            "system break",
            Box::new(|s| {
                s.toggle_layout_break(4, LayoutBreak::System)
                    .map(|_| ())
                    .unwrap()
            }),
        ),
        (
            "page break",
            Box::new(|s| {
                s.toggle_layout_break(4, LayoutBreak::Page)
                    .map(|_| ())
                    .unwrap()
            }),
        ),
    ];
    for (name, change) in &starts {
        let mut score = violin(10, &[0]);
        assert_eq!(runs(&score), [1..10]);
        change(&mut score);
        assert_eq!(runs(&score), [1..5, 5..10], "{name}");
    }
    // Shown in bar 5 itself: bar 5 stands on its own.
    let alone: Vec<(&str, Change)> = vec![
        (
            "note",
            Box::new(|s| s.insert(5, 0, note()).map(|_| ()).unwrap()),
        ),
        (
            "segno",
            Box::new(|s| {
                s.set_bar_form(
                    5,
                    BarForm {
                        segno: true,
                        ..Default::default()
                    },
                )
                .map(|_| ())
                .unwrap()
            }),
        ),
        (
            "jump",
            Box::new(|s| {
                s.set_bar_form(
                    5,
                    BarForm {
                        jump: Some(Jump::DaCapo),
                        ..Default::default()
                    },
                )
                .map(|_| ())
                .unwrap()
            }),
        ),
        (
            "ending",
            Box::new(|s| {
                s.set_ending(Ending {
                    start: 5,
                    end: 5,
                    numbers: vec![1],
                })
                .map(|_| ())
                .unwrap()
            }),
        ),
        (
            "dynamic on a rest",
            Box::new(|s| {
                s.insert(
                    5,
                    0,
                    rest(Notation {
                        dynamic: Some(Dynamic::P),
                        ..Default::default()
                    }),
                )
                .map(|_| ())
                .unwrap()
            }),
        ),
        (
            "text on a rest",
            Box::new(move |s| {
                s.insert(
                    5,
                    0,
                    rest(Notation {
                        technique_text: Some(words("con sord.")),
                        ..Default::default()
                    }),
                )
                .map(|_| ())
                .unwrap()
            }),
        ),
    ];
    for (name, change) in &alone {
        let mut score = violin(10, &[0]);
        change(&mut score);
        assert_eq!(runs(&score), [1..5, 6..10], "{name}");
        let layout = part(&score);
        let bar = layout.measure(id(&score), 5).unwrap();
        assert!(!layout.gathered(5) && !h_bar(bar), "{name}");
    }
    // A plain rest written by hand is still an empty bar.
    let mut written = violin(10, &[0]);
    written.insert(5, 0, rest(Notation::default())).unwrap();
    assert_eq!(runs(&written), [1..10]);
}

#[test]
fn the_musician_can_draw_every_empty_bar() {
    let mut score = violin(10, &[0]);
    score
        .set_page_setup(PageSetup {
            separate_empty_bars: true,
            ..Default::default()
        })
        .unwrap();
    let layout = part(&score);
    assert!(layout.multirests.is_empty());
    assert!(!layout.measures.iter().any(h_bar));
    assert_eq!(
        layout
            .measures
            .iter()
            .filter(|m| m.content.glyphs.iter().any(|g| g.text == "\u{e4e3}"))
            .count(),
        9
    );
}

#[test]
fn a_piano_part_rests_only_where_both_hands_do() {
    let mut score = Score::new("Piano".into(), &[Instrument::Piano], 8).unwrap();
    let id = score.parts()[0].id();
    score.insert_on(id, StaffId::Upper, 0, 0, note()).unwrap();
    let bass = Event {
        pitches: Pitches::single(Pitch::new(Step::C, 3)),
        ..note()
    };
    score.insert_on(id, StaffId::Lower, 3, 0, bass).unwrap();
    let layout = part(&score);
    assert_eq!(layout.multirests, [1..3, 4..8]);
    for staff in [StaffId::Upper, StaffId::Lower] {
        let first = layout.measure_on(id, staff, 4).unwrap();
        assert!(h_bar(first), "{staff:?}");
        // The count goes once, above the right hand.
        assert_eq!(count(first).is_some(), staff == StaffId::Upper);
        for bar in 5..8 {
            let gathered = layout.measure_on(id, staff, bar).unwrap();
            assert_eq!(gathered.origin, first.origin);
            assert!(gathered.content.lines.is_empty());
        }
        assert!(!h_bar(layout.measure_on(id, staff, 3).unwrap()));
    }
    // The hands share one barline at the end of a run, not one per bar.
    let end = layout.measure(id, 1).unwrap();
    let x = end.origin.x + end.width;
    assert_eq!(
        layout
            .connectors
            .iter()
            .filter(|line| (line.from.x - x).abs() < 0.01)
            .count(),
        1
    );
    // A piano that never plays is one multi-measure rest.
    let lower = Score::new("Piano".into(), &[Instrument::Piano], 8).unwrap();
    assert_eq!(runs(&lower), [0..8]);
}
