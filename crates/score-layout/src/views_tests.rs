use super::*;
use score_core::{Connection, ConnectionKind, Event, KeyMode, Notation, Pitch, Pitches, Step};

fn note() -> Event {
    Event {
        pitches: Pitches::single(Pitch::new(Step::F, 4)),
        duration: Duration::Quarter,
        notation: Notation::default(),
    }
}
fn piece(bars: usize) -> (Score, Vec<ElementId>) {
    let mut score = Score::default();
    while score.measures().len() < bars {
        score.append_measure().unwrap();
    }
    let ids = (0..bars)
        .map(|bar| score.insert(bar, 0, note()).unwrap())
        .collect();
    (score, ids)
}
fn close(a: f32, b: f32) {
    assert!((a - b).abs() < 0.01, "{a} != {b}");
}

#[test]
fn tempo_labels_follow_each_view_reserve_ink_and_can_be_clicked_after_zoom() {
    use score_core::{TempoBeat, TempoMark};
    let (mut score, _) = piece(40);
    score
        .set_tempo_at(
            5,
            TempoMark {
                bpm: 84,
                text: "Andante cantabile".into(),
                ..Default::default()
            },
        )
        .unwrap();
    score
        .set_tempo_at(
            20,
            TempoMark {
                bpm: 132,
                beat: TempoBeat::DottedQuarter,
                text: "Allegro".into(),
            },
        )
        .unwrap();
    let mut engine = LayoutEngine::default();
    for mode in [ViewMode::Page, ViewMode::Continuous, ViewMode::SingleSystem] {
        let mut layout = engine.layout_view(&score, 1000., mode);
        assert_eq!(layout.tempos.len(), 3);
        for scale in [0.28, 0.7, 1.5] {
            layout.scale = scale;
            for mark in &layout.tempos {
                let bar = layout
                    .measures
                    .iter()
                    .find(|bar| bar.index == mark.measure)
                    .unwrap();
                let system = layout
                    .systems
                    .iter()
                    .find(|s| (s.first_measure..=s.last_measure).contains(&mark.measure))
                    .unwrap();
                assert!(mark.position.y - 30. >= system.top - 3.);
                // A tempo clears ink in its own horizontal span; distant high
                // notes elsewhere in the bar no longer push it upward.
                let metrics = Metrics::bravura();
                for glyph in &bar.content.glyphs {
                    let (left, top, right, _) = metrics.bounds(&glyph.text, glyph.rotated);
                    let x = bar.origin.x + glyph.position.x;
                    if x + right >= mark.position.x && x + left <= mark.position.x + mark.width {
                        assert!(
                            mark.position.y + 6. <= bar.origin.y + glyph.position.y + top - 5.99
                        );
                    }
                }
                assert!(mark.position.x + mark.width <= bar.origin.x + bar.width);
                assert_eq!(
                    layout.hit_tempo(Point {
                        x: (mark.position.x + 10.) * scale,
                        y: (mark.position.y - 10.) * scale
                    }),
                    Some(mark.measure)
                );
                if mode == ViewMode::Page {
                    let page = &layout.pages[layout.page_for_measure(mark.measure).unwrap()];
                    assert!(mark.position.y > page.top && mark.position.y < page.top + page.height);
                }
            }
        }
        score
            .set_tempo_at(
                20,
                TempoMark {
                    bpm: 108,
                    text: "Moderato".into(),
                    ..Default::default()
                },
            )
            .unwrap();
        let changed = engine.layout_view(&score, 1000., mode);
        assert_eq!(changed.tempos.last().unwrap().mark.bpm, 108);
        score.remove_tempo_change(20);
        let removed = engine.layout_view(&score, 1000., mode);
        assert_eq!(removed.tempos.len(), 2);
        score
            .set_tempo_at(
                20,
                TempoMark {
                    bpm: 132,
                    beat: TempoBeat::DottedQuarter,
                    text: "Allegro".into(),
                },
            )
            .unwrap();
    }
}

#[test]
fn pages_keep_systems_inside_sheets_and_stay_fixed_when_the_window_changes() {
    let (score, ids) = piece(40);
    let mut engine = LayoutEngine::default();
    let mut layout = engine.layout_view(&score, 500., ViewMode::Page);
    assert!(layout.pages.len() > 1);
    assert_eq!(layout.pages[0].first_measure, 0);
    assert_eq!(layout.pages.last().unwrap().last_measure, 39);
    for pages in layout.pages.windows(2) {
        assert_eq!(pages[0].last_measure + 1, pages[1].first_measure);
        assert!(pages[1].top > pages[0].top + pages[0].height);
    }
    for system in &layout.systems {
        let page = &layout.pages[layout.page_for_measure(system.first_measure).unwrap()];
        assert!(system.top >= page.top + 56.);
        assert!(system.bottom <= page.top + page.height - 56. + 0.01);
        for measure in layout
            .measures
            .iter()
            .filter(|m| (system.first_measure..=system.last_measure).contains(&m.index))
        {
            assert!(
                measure.origin.y + measure.content.ink_top >= system.top - 0.01,
                "ink {} top {}",
                measure.origin.y + measure.content.ink_top,
                system.top
            );
            assert!(measure.origin.y + measure.content.ink_bottom <= system.bottom + 0.01);
            assert!(measure.origin.x + measure.width <= page.width - 31.);
        }
    }
    let resized = engine.layout_view(&score, 1800., ViewMode::Page);
    assert_eq!(engine.recomputed_measures, 0);
    for (a, b) in layout.measures.iter().zip(&resized.measures) {
        assert_eq!(a.origin, b.origin);
        close(a.width, b.width);
    }
    for scale in [0.28, 0.7, 1.5] {
        layout.scale = scale;
        for id in &ids {
            let m = layout.measure(id.part, id.measure).unwrap();
            let event = &m.content.events[0];
            let p = Point {
                x: (m.origin.x + event.center.x) * scale,
                y: (m.origin.y + event.center.y) * scale,
            };
            assert_eq!(layout.hit_test(p), Some(*id));
            assert_eq!(layout.measure_at(p), Some((id.part, id.staff, id.measure)));
        }
    }
    let gap = &layout.pages[1];
    assert!(
        layout
            .measure_at(Point {
                x: 300. * layout.scale,
                y: (gap.top - 16.) * layout.scale
            })
            .is_none()
    );
}

#[test]
fn horizontal_view_keeps_all_bars_aligned_and_mode_switches_preserve_the_music() {
    let (mut score, ids) = piece(20);
    score
        .set_key_signature_at(8, KeySignature::new(-3, KeyMode::Major).unwrap())
        .unwrap();
    score
        .set_time_signature_at(
            9,
            TimeSignature {
                numerator: 6,
                denominator: 8,
            },
        )
        .unwrap();
    let before = score.clone();
    let mut engine = LayoutEngine::default();
    for mode in [
        ViewMode::SingleSystem,
        ViewMode::Page,
        ViewMode::Continuous,
        ViewMode::SingleSystem,
    ] {
        let layout = engine.layout_view(&score, 800., mode);
        assert_eq!(score, before);
        for id in &ids {
            let upper = layout.measure(id.part, id.measure).unwrap();
            let lower = layout
                .measure_on(id.part, StaffId::Lower, id.measure)
                .unwrap();
            close(upper.origin.x, lower.origin.x);
            close(upper.width, lower.width);
            if mode == ViewMode::SingleSystem {
                close(upper.origin.y, layout.measures[0].origin.y);
                if id.measure > 0 {
                    let previous = layout.measure(id.part, id.measure - 1).unwrap();
                    close(upper.origin.x, previous.origin.x + previous.width);
                }
            }
        }
        for system in &layout.systems {
            let upper = layout.measure(ids[0].part, system.first_measure).unwrap();
            let lower = layout
                .measure_on(ids[0].part, StaffId::Lower, system.first_measure)
                .unwrap();
            assert!(
                layout
                    .braces
                    .iter()
                    .any(|brace| (brace.top - upper.origin.y).abs() < 0.01
                        && (brace.bottom - lower.origin.y - 40.).abs() < 0.01)
            );
            assert!(
                layout
                    .labels
                    .iter()
                    .any(|label| label.staff == Some(StaffId::Upper)
                        && (label.position.y - upper.origin.y - 24.).abs() < 0.01)
            );
        }
        if mode == ViewMode::SingleSystem {
            assert_eq!(layout.systems.len(), 1);
            assert!(layout.width > 3000.);
        } else {
            assert!(layout.systems.len() > 1);
        }
        assert!(
            layout
                .measure(ids[0].part, 8)
                .unwrap()
                .content
                .glyphs
                .iter()
                .any(|g| g.text == "\u{e260}")
        );
        assert!(
            layout
                .measure(ids[0].part, 9)
                .unwrap()
                .content
                .glyphs
                .iter()
                .any(|g| g.text == "\u{e086}")
        );
    }
}

#[test]
fn connections_are_split_on_final_systems_and_never_cross_the_page_gap() {
    let (mut score, ids) = piece(40);
    for kind in [ConnectionKind::Slur, ConnectionKind::Crescendo] {
        score
            .add_connection(Connection {
                start: ids[0],
                end: ids[39],
                kind,
            })
            .unwrap();
    }
    let layout = LayoutEngine::default().layout_view(&score, 1000., ViewMode::Page);
    assert!(layout.pages.len() > 1);
    assert_eq!(layout.curves.len(), layout.systems.len());
    assert_eq!(layout.hairpins.len(), layout.systems.len());
    for curve in &layout.curves {
        assert!(
            layout
                .pages
                .iter()
                .any(|p| curve.from.y.min(curve.to.y) + curve.bend > p.top
                    && curve.from.y.max(curve.to.y) < p.top + p.height)
        );
    }
    for hairpin in &layout.hairpins {
        assert!(
            layout
                .pages
                .iter()
                .any(|p| hairpin.upper.from.y.min(hairpin.upper.to.y) > p.top
                    && hairpin.lower.from.y.max(hairpin.lower.to.y) < p.top + p.height)
        );
    }
}

#[test]
fn oversized_systems_get_larger_sheets_without_clipping_or_splitting_piano_hands() {
    let (mut score, _) = piece(4);
    while score.parts().len() < score_core::MAX_PARTS {
        score.add_part(Instrument::Piano).unwrap();
    }
    let layout = LayoutEngine::default().layout_view(&score, 1000., ViewMode::Page);
    assert!(layout.pages[0].height > 2000.);
    assert!(
        layout
            .measures
            .iter()
            .all(|m| m.origin.y + m.content.ink_bottom <= layout.pages[0].height - 56. + 0.01)
    );
    assert_eq!(layout.pages.len(), 1);
    let mut dense = Score::default();
    dense
        .set_time_signature_at(
            0,
            TimeSignature {
                numerator: 16,
                denominator: 1,
            },
        )
        .unwrap();
    for index in 0..64 {
        dense.insert(0, index, note()).unwrap();
    }
    let layout = LayoutEngine::default().layout_view(&dense, 1000., ViewMode::Page);
    assert!(layout.pages[0].width > 1000.);
    assert!(
        layout
            .measures
            .iter()
            .all(|m| m.origin.x + m.width <= layout.pages[0].width)
    );
}
