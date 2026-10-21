use super::*;
use score_core::{BarForm, Barline, Event, Notation, Pitch, Pitches, Step};

fn score() -> Score {
    Score::new("Form".into(), &[Instrument::Violin], 4).unwrap()
}

fn vertical_lines(measure: &PlacedMeasure) -> impl Iterator<Item = &Line> {
    measure
        .content
        .lines
        .iter()
        .filter(|line| line.from.x == line.to.x && line.from.y == 0. && line.to.y == 40.)
}

#[test]
fn last_bar_closes_with_a_final_barline() {
    let score = score();
    let metrics = Metrics::bravura();
    let layout = LayoutEngine::default().layout(&score, 1000.);
    for measure in &layout.measures {
        let heavy = vertical_lines(measure).find(|line| line.thickness == metrics.thick);
        assert_eq!(heavy.is_some(), measure.index == 3);
        if let Some(line) = heavy {
            assert_eq!(line.from.x + metrics.thick / 2., measure.width);
        }
    }
}

#[test]
fn repeat_end_draws_dots_thin_and_thick() {
    let mut score = score();
    let metrics = Metrics::bravura();
    // A repeat takes precedence over an explicit double or final barline.
    for barline in [Barline::Single, Barline::Double, Barline::Final] {
        score
            .set_bar_form(
                1,
                BarForm {
                    repeat_end: Some(2),
                    barline,
                    ..Default::default()
                },
            )
            .unwrap();
        let layout = LayoutEngine::default().layout(&score, 1000.);
        let measure = &layout.measures[1];
        let dots: Vec<_> = measure
            .content
            .glyphs
            .iter()
            .filter(|g| g.text == "\u{e044}")
            .collect();
        assert_eq!(dots.len(), 2);
        let heavy = vertical_lines(measure)
            .find(|l| l.thickness == metrics.thick)
            .unwrap();
        let thin = vertical_lines(measure)
            .find(|l| l.thickness == metrics.bar)
            .unwrap();
        assert!(thin.from.x < heavy.from.x);
        let (_, _, right, _) = metrics.bounds("\u{e044}", false);
        for (dot, y) in dots.iter().zip([15., 25.]) {
            assert_eq!(dot.position.y, y);
            assert!(
                (thin.from.x - metrics.bar / 2. - dot.position.x - right - metrics.dot_gap).abs()
                    < 0.001
            );
        }
        assert!(measure.content.time_positions.last().unwrap().1 < dots[0].position.x);
    }
}

#[test]
fn repeat_start_after_signatures_at_system_start() {
    let mut score = score();
    score.set_key_signature(KeySignature::new(3, score_core::KeyMode::Major).unwrap());
    score
        .set_bar_form(
            0,
            BarForm {
                repeat_start: true,
                ..Default::default()
            },
        )
        .unwrap();
    let id = score
        .insert(
            0,
            0,
            Event {
                pitches: Pitches::single(Pitch::new(Step::G, 4)),
                duration: Duration::Quarter,
                notation: Notation::default(),
            },
        )
        .unwrap();
    let metrics = Metrics::bravura();
    let layout = LayoutEngine::default().layout(&score, 1000.);
    let measure = &layout.measures[0];
    let heavy = vertical_lines(measure)
        .find(|l| l.thickness == metrics.thick)
        .unwrap();
    let signatures_right = measure
        .content
        .glyphs
        .iter()
        .filter(|g| g.element.is_none() && g.text != "\u{e044}")
        .map(|g| g.position.x + metrics.bounds(&g.text, g.rotated).2)
        .fold(0., f32::max);
    assert!(heavy.from.x - metrics.thick / 2. > signatures_right);
    let head = measure
        .content
        .events
        .iter()
        .find(|event| event.id == id)
        .unwrap();
    assert!(heavy.from.x < head.center.x);
    let thin = vertical_lines(measure)
        .filter(|l| l.from.x > heavy.from.x && l.thickness == metrics.bar)
        .min_by(|a, b| a.from.x.total_cmp(&b.from.x))
        .unwrap();
    let dots: Vec<_> = measure
        .content
        .glyphs
        .iter()
        .filter(|g| g.text == "\u{e044}")
        .collect();
    assert_eq!(dots.len(), 2);
    let (left, _, right, _) = metrics.bounds("\u{e044}", false);
    for dot in dots {
        assert!(
            (dot.position.x + left - thin.from.x - metrics.bar / 2. - metrics.dot_gap).abs()
                < 0.001
        );
        assert!(dot.position.x + right < head.center.x);
    }
    assert!(vertical_lines(measure).any(|line| line.from.x == 0. && line.thickness == metrics.bar));
}

#[test]
fn double_barline_has_two_thin_lines() {
    let mut score = score();
    let metrics = Metrics::bravura();
    // An explicit double barline is preserved even in the last measure.
    for index in [1, 3] {
        score
            .set_bar_form(
                index,
                BarForm {
                    barline: Barline::Double,
                    ..Default::default()
                },
            )
            .unwrap();
    }
    let layout = LayoutEngine::default().layout(&score, 1000.);
    for index in [1, 3] {
        let measure = &layout.measures[index];
        let lines: Vec<_> = vertical_lines(measure).collect();
        assert_eq!(lines.len(), 2);
        assert!(lines.iter().all(|line| line.thickness == metrics.bar));
        assert!((lines[0].from.x - lines[1].from.x - metrics.bar_gap - metrics.bar).abs() < 0.001);
    }
}

#[test]
fn form_change_invalidates_only_affected_measures() {
    let mut score = score();
    let mut engine = LayoutEngine::default();
    let mut before = engine.layout(&score, 1000.);
    engine.layout(&score, 1000.);
    assert_eq!(engine.recomputed_measures, 0);
    for form in [
        BarForm {
            barline: Barline::Double,
            ..Default::default()
        },
        BarForm {
            repeat_end: Some(2),
            ..Default::default()
        },
        BarForm {
            repeat_start: true,
            ..Default::default()
        },
    ] {
        score.set_bar_form(2, form).unwrap();
        let after = engine.layout(&score, 1000.);
        assert_eq!(engine.recomputed_measures, 1);
        for (old, new) in before.measures.iter().zip(&after.measures) {
            assert_eq!(old.width, new.width);
            assert_eq!(Arc::ptr_eq(&old.content, &new.content), new.index != 2);
        }
        before = after;
    }
}

#[test]
fn grouped_staves_share_the_heavy_barline() {
    for instruments in [
        vec![Instrument::Violin, Instrument::Violin],
        vec![Instrument::Piano],
    ] {
        let mut score = Score::new("Groups".into(), &instruments, 4).unwrap();
        let part = score.parts()[0].id();
        // Exercise the start of a system and an interior repeat together.
        for index in [0, 1] {
            score
                .set_bar_form(
                    index,
                    BarForm {
                        repeat_start: true,
                        repeat_end: Some(2),
                        ..Default::default()
                    },
                )
                .unwrap();
        }
        let layout = LayoutEngine::default().layout(&score, 1000.);
        for index in [0, 1, 3] {
            let measure = layout.measure(part, index).unwrap();
            for line in vertical_lines(measure) {
                let x = measure.origin.x + line.from.x;
                assert!(
                    layout
                        .connectors
                        .iter()
                        .any(|connector| connector.from.x == x
                            && connector.thickness == line.thickness
                            && connector.from.y == measure.origin.y
                            && connector.to.y > measure.origin.y + 40.)
                );
            }
        }
    }
}

#[test]
fn repeat_start_aligns_when_divisi_enters_inside_a_system() {
    let mut score = score();
    let part = score.parts()[0].id();
    score
        .set_divisi_span(part, score_core::DivisiPoint::new(1, 0), None, true)
        .unwrap();
    score
        .set_bar_form(
            1,
            BarForm {
                repeat_start: true,
                ..Default::default()
            },
        )
        .unwrap();
    let metrics = Metrics::bravura();
    let layout = LayoutEngine::default().layout(&score, 1000.);
    let upper = layout.measure(part, 1).unwrap();
    let lower = layout.measure_on(part, StaffId::Lower, 1).unwrap();
    let repeat_x = signature_prefix(&score, 1, false) - 18. - 2.;
    assert!(repeat_x > 0.);
    for measure in [upper, lower] {
        let heavy = vertical_lines(measure)
            .find(|l| l.thickness == metrics.thick)
            .unwrap();
        assert_eq!(heavy.from.x, repeat_x + metrics.thick / 2.);
        for glyph in measure
            .content
            .glyphs
            .iter()
            .filter(|g| g.text.chars().all(|c| matches!(c, '\u{e050}' | '\u{e05c}' | '\u{e062}' | '\u{e069}' | '\u{e260}'..='\u{e262}' | '\u{e080}'..='\u{e089}')))
        {
            assert!(glyph.position.x + metrics.bounds(&glyph.text, glyph.rotated).2 < repeat_x);
        }
    }
    for (x, thickness) in repeat_start_strokes(upper.origin.x + repeat_x, &metrics) {
        assert!(layout.connectors.iter().any(|line| {
            line.from.x == x
                && line.thickness == thickness
                && line.from.y == upper.origin.y
                && line.to.y == lower.origin.y + 40.
        }));
    }
    // A repeat moved after the entering row's signatures must not connect
    // through the beginning of those signatures.
    assert!(
        !layout
            .connectors
            .iter()
            .any(|line| line.from.x == upper.origin.x && line.thickness == metrics.thick)
    );
    assert!(vertical_lines(lower).any(|line| line.from.x == 0. && line.thickness == metrics.bar));
}

#[test]
fn repeat_start_aligns_across_transposed_keys_and_instrument_families() {
    let mut score = Score::new(
        "Transposed repeats".into(),
        &[
            Instrument::Flute,
            Instrument::ClarinetBb,
            Instrument::HornF,
            Instrument::TrumpetBb,
        ],
        8,
    )
    .unwrap();
    for index in [0, 1, 4] {
        score
            .set_bar_form(
                index,
                BarForm {
                    repeat_start: true,
                    ..Default::default()
                },
            )
            .unwrap();
    }
    assert_ne!(
        score.written_key_at(score.parts()[0].id(), 0),
        score.written_key_at(score.parts()[1].id(), 0)
    );
    let metrics = Metrics::bravura();
    let layout = LayoutEngine::default().layout(&score, 1600.);
    for index in [0, 1, 4] {
        let first = index == 0 || index == 4;
        let repeat_x = if first {
            signature_prefix(&score, index, true) - 18. - 2.
        } else {
            0.
        };
        for part in score.parts() {
            let measure = layout.measure(part.id(), index).unwrap();
            let heavy = vertical_lines(measure)
                .find(|line| line.thickness == metrics.thick)
                .unwrap();
            assert_eq!(heavy.from.x, repeat_x + metrics.thick / 2.);
        }
        let parts: Vec<_> = score.parts().iter().collect();
        for parts in parts.chunks(2) {
            let upper = layout.measure(parts[0].id(), index).unwrap();
            let lower = layout.measure(parts[1].id(), index).unwrap();
            for (x, thickness) in repeat_start_strokes(upper.origin.x + repeat_x, &metrics) {
                assert!(layout.connectors.iter().any(|line| line.from.x == x
                    && line.thickness == thickness
                    && line.from.y == upper.origin.y
                    && line.to.y == lower.origin.y + 40.));
            }
        }
    }
}

#[test]
fn ending_bracket_has_label_hooks_and_space() {
    let mut score = score();
    let plain = LayoutEngine::default().layout(&score, 1000.);
    for (bar, number) in [(1, 1), (2, 2)] {
        score
            .set_ending(score_core::Ending {
                start: bar,
                end: bar,
                numbers: vec![number],
            })
            .unwrap();
    }
    let layout = LayoutEngine::default().layout(&score, 1000.);
    assert_eq!(layout.endings.len(), 2);
    assert_eq!(layout.endings[0].label, "1.");
    assert_eq!(layout.endings[1].label, "2.");
    assert_eq!(layout.endings[0].lines.len(), 3);
    assert_eq!(layout.endings[1].lines.len(), 2);
    let metrics = Metrics::bravura();
    for ending in &layout.endings {
        assert!(
            ending
                .lines
                .iter()
                .all(|line| line.thickness == metrics.ending)
        );
        assert_eq!(ending.lines[1].from.x, ending.lines[0].from.x);
        assert_eq!(ending.lines[1].to.y - ending.lines[1].from.y, 16.);
    }
    assert_eq!(
        layout.endings[0].lines[2].from.x,
        layout.endings[0].lines[0].to.x
    );
    assert_eq!(
        layout.measures[0].origin.y - plain.measures[0].origin.y,
        30.
    );
}

#[test]
fn ending_across_a_system_break_continues_without_label() {
    let mut score = Score::new("Long ending".into(), &[Instrument::Violin], 8).unwrap();
    score
        .set_ending(score_core::Ending {
            start: 1,
            end: 6,
            numbers: vec![1, 2],
        })
        .unwrap();
    let layout = LayoutEngine::default().layout(&score, 2000.);
    assert_eq!(layout.systems.len(), 2);
    assert_eq!(layout.endings.len(), 2);
    assert_eq!(layout.endings[0].label, "1., 2.");
    assert_eq!(layout.endings[0].lines.len(), 2);
    assert!(layout.endings[1].label.is_empty());
    assert_eq!(layout.endings[1].lines.len(), 1);
    let continuation = layout.measures.iter().find(|bar| bar.index == 4).unwrap();
    assert_eq!(
        layout.endings[1].lines[0].from.x,
        continuation.origin.x + 2.
    );
}

#[test]
fn navigation_marks_sit_above_the_first_part() {
    let mut score = Score::new(
        "Navigation".into(),
        &[Instrument::Violin, Instrument::Viola],
        4,
    )
    .unwrap();
    score
        .set_bar_form(
            1,
            BarForm {
                segno: true,
                coda: true,
                ..Default::default()
            },
        )
        .unwrap();
    score
        .set_bar_form(
            2,
            BarForm {
                to_coda: true,
                fine: true,
                jump: Some(score_core::Jump::DaCapoAlFine),
                ..Default::default()
            },
        )
        .unwrap();
    let layout = LayoutEngine::default().layout(&score, 1000.);
    assert_eq!(layout.navigation.len(), 5);
    for label in &layout.navigation {
        let bar = layout
            .measures
            .iter()
            .find(|bar| bar.index == label.measure)
            .unwrap();
        assert!(label.position.y < bar.origin.y);
        if label.right_aligned {
            assert_eq!(label.position.x, bar.origin.x + bar.width);
        }
    }
    let segno = layout
        .navigation
        .iter()
        .find(|label| label.text == "\u{e047}")
        .unwrap();
    let coda = layout
        .navigation
        .iter()
        .find(|label| label.text == "\u{e048}")
        .unwrap();
    assert!(coda.position.x > segno.position.x);
    let jump = layout
        .navigation
        .iter()
        .find(|label| label.text == "D.C. al Fine")
        .unwrap();
    assert!(jump.right_aligned);
    let fine = layout
        .navigation
        .iter()
        .find(|label| label.text == "Fine")
        .unwrap();
    assert_eq!(fine.position.y - jump.position.y, 18.);
    let coda = layout
        .navigation
        .iter()
        .find(|label| label.text == "To Coda \u{e048}")
        .unwrap();
    assert_eq!(coda.position.y - fine.position.y, 18.);
}

#[test]
fn segno_after_tempo_on_the_same_bar() {
    let mut score = score();
    score
        .set_bar_form(
            0,
            BarForm {
                segno: true,
                ..Default::default()
            },
        )
        .unwrap();
    let layout = LayoutEngine::default().layout(&score, 1000.);
    let tempo = &layout.tempos[0];
    let segno = &layout.navigation[0];
    assert!(segno.position.x > tempo.position.x + tempo.width);
    assert!(segno.position.x + 40. <= layout.width - 12.);
    assert_eq!(
        text_runs("To Coda \u{e048}"),
        vec![
            TextRun::Text("To Coda ".into()),
            TextRun::Music("\u{e048}".into())
        ]
    );
    assert_eq!(bravura_character('\u{e047}'), Some('\u{e047}'));
}

#[test]
fn form_marks_keep_their_bounds_and_positions_in_every_presentation() {
    let mut score = Score::new("Paged form".into(), &[Instrument::Violin], 80).unwrap();
    score
        .set_ending(score_core::Ending {
            start: 1,
            end: 75,
            numbers: vec![1],
        })
        .unwrap();
    for bar in (0..80).step_by(4) {
        score
            .set_bar_form(
                bar,
                BarForm {
                    segno: true,
                    ..Default::default()
                },
            )
            .unwrap();
        score
            .set_bar_form(
                bar + 2,
                BarForm {
                    fine: true,
                    jump: Some(score_core::Jump::DaCapoAlFine),
                    ..Default::default()
                },
            )
            .unwrap();
    }
    let original = score.clone();
    let mut engine = LayoutEngine::default();
    for mode in [ViewMode::Page, ViewMode::Continuous, ViewMode::SingleSystem] {
        for part in [false, true] {
            let layout = if part {
                engine.layout_part(&score, 1000., mode)
            } else {
                engine.layout_view(&score, 1000., mode)
            };
            if mode == ViewMode::Page {
                assert!(layout.pages.len() > 1);
            }
            for label in &layout.navigation {
                let system = layout
                    .systems
                    .iter()
                    .find(|system| {
                        (system.first_measure..=system.last_measure).contains(&label.measure)
                    })
                    .unwrap();
                assert!(label.position.y - label.size >= system.top - 0.01);
                assert!(label.position.y + 4. <= system.bottom);
                let page = &layout.pages[layout.page_for_measure(label.measure).unwrap()];
                assert!(label.position.y - label.size >= page.top);
                assert!(label.position.y + 4. < page.top + page.height);
            }
            for ending in &layout.endings {
                let system = layout
                    .systems
                    .iter()
                    .find(|system| {
                        ending.lines[0].from.y >= system.top
                            && ending.lines[0].from.y <= system.bottom
                    })
                    .unwrap();
                for line in &ending.lines {
                    assert!(line.from.y.min(line.to.y) - line.thickness / 2. >= system.top - 0.01);
                    assert!(
                        line.from.y.max(line.to.y) + line.thickness / 2. <= system.bottom + 0.01
                    );
                }
                if !ending.label.is_empty() {
                    assert!(ending.label_position.y - 14. >= system.top - 0.01);
                    assert!(ending.label_position.y + 4. <= system.bottom);
                }
            }
            let preview = engine.layout_pitch_preview(&score, &layout);
            for (actual, expected) in preview.navigation.iter().zip(&layout.navigation) {
                assert_eq!(actual.position, expected.position);
            }
            for (actual, expected) in preview.endings.iter().zip(&layout.endings) {
                assert_eq!(actual.label_position, expected.label_position);
                assert_eq!(actual.lines[0].from, expected.lines[0].from);
            }
        }
    }
    assert_eq!(score, original);
}
