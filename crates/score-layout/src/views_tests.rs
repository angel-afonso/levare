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
                assert!(mark.position.x + mark.width <= layout.width - 12.);
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
            assert!(layout.labels.iter().any(|label| label.part == ids[0].part
                && label.staff.is_none()
                && (label.position.y - (upper.origin.y + lower.origin.y + 40.) / 2.).abs() < 0.01));
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

#[test]
fn bar_numbers_follow_the_setting_on_the_top_staff_and_pages_number_their_corner() {
    use score_core::{BarNumbers, Numbering, PageNumbers};
    let (mut score, _) = piece(80);
    let numbered = |score: &Score| {
        let layout = LayoutEngine::default().layout_view(score, 1000., ViewMode::Page);
        let bars: Vec<_> = layout.numbered_bars().map(|m| m.index).collect();
        // Only the piano's upper staff carries numbers.
        assert!(layout.numbered_bars().all(|m| m.staff == StaffId::Upper));
        (layout, bars)
    };
    let (layout, bars) = numbered(&score);
    let starts: Vec<_> = layout.systems.iter().map(|s| s.first_measure).collect();
    assert!(layout.systems.len() > 2 && layout.pages.len() > 1);
    assert_eq!(bars, starts[1..]);
    assert_eq!(
        layout.page_number(1),
        Some((
            "2 / ".to_owned() + &layout.pages.len().to_string(),
            PageNumberPlace::Footer
        ))
    );
    for (setting, expected) in [
        (BarNumbers::EveryBar, (1..80).collect::<Vec<_>>()),
        (BarNumbers::Every(10), (1..=8).map(|n| n * 10 - 1).collect()),
        (BarNumbers::Hidden, Vec::new()),
    ] {
        score
            .set_numbering(Numbering {
                bars: setting,
                pages: PageNumbers::Corner,
            })
            .unwrap();
        let (layout, bars) = numbered(&score);
        assert_eq!(bars, expected, "{setting:?}");
        assert_eq!(layout.page_number(0), None);
        assert_eq!(
            layout.page_number(1),
            Some(("2".to_owned(), PageNumberPlace::TopLeft))
        );
        assert_eq!(
            layout.page_number(2).map(|(_, place)| place),
            (layout.pages.len() > 2).then_some(PageNumberPlace::TopRight)
        );
    }
    assert!(
        score
            .set_numbering(Numbering {
                bars: BarNumbers::Every(1),
                ..Numbering::default()
            })
            .is_err()
    );
}

#[test]
fn paper_and_staff_size_shape_the_page() {
    use score_core::{PageSetup, Paper};
    let (mut score, _) = piece(40);
    let page = |score: &Score| {
        let layout = LayoutEngine::default().layout_view(score, 1000., ViewMode::Page);
        let page = layout.pages[0].clone();
        (
            page.width,
            page.height,
            layout.systems.len(),
            layout.pages.len(),
        )
    };
    let (width, height, _, pages) = page(&score);
    assert_eq!(width, 1000.);
    assert!((height / width - std::f32::consts::SQRT_2).abs() < 0.001);
    // A 5 mm staff on A4: 210 mm hold 42 staff heights, so pages hold more.
    score
        .set_page_setup(PageSetup {
            staff: Some(50),
            ..PageSetup::default()
        })
        .unwrap();
    let (small, _, _, fewer) = page(&score);
    assert!((small - 210. * 40. / 5.).abs() < 0.01);
    assert!(fewer < pages);
    // Landscape Letter: the page turns and keeps the staff size.
    score
        .set_page_setup(PageSetup {
            paper: Paper::Letter,
            landscape: true,
            staff: Some(50),
            hide_empty_staves: false,
            margins: Default::default(),
            separate_empty_bars: false,
        })
        .unwrap();
    let (wide, tall, _, _) = page(&score);
    assert!((wide - 279.4 * 40. / 5.).abs() < 0.01);
    assert!((tall / wide - 215.9 / 279.4).abs() < 0.001);
    // Automatic size on A3 keeps the A4 staff and fits more music.
    score
        .set_page_setup(PageSetup {
            paper: Paper::A3,
            ..PageSetup::default()
        })
        .unwrap();
    let (a3, _, _, wider) = page(&score);
    assert!((a3 - 1000. * 297. / 210.).abs() < 0.01);
    assert!(wider < pages);
    assert!(
        score
            .set_page_setup(PageSetup {
                staff: Some(5),
                ..PageSetup::default()
            })
            .is_err()
    );
}

#[test]
fn empty_staves_leave_later_systems_in_page_view_only() {
    use score_core::{Instrument, PageSetup};
    let (mut score, _) = piece(40);
    let flute = score.add_part(Instrument::Flute).unwrap();
    let cello = score.add_part(Instrument::Cello).unwrap();
    // The flute plays only in the first and last bars; the cello never.
    for bar in [0, 39] {
        score.insert_in(flute, bar, 0, note()).unwrap();
    }
    score
        .set_page_setup(PageSetup {
            hide_empty_staves: true,
            ..PageSetup::default()
        })
        .unwrap();
    let mut engine = LayoutEngine::default();
    let page = engine.layout_view(&score, 1000., ViewMode::Page);
    let shows = |layout: &Layout, part, system: &System| {
        layout
            .measures
            .iter()
            .any(|m| m.part == part && m.index == system.first_measure)
    };
    let (first, last) = (&page.systems[0], page.systems.last().unwrap());
    assert!(shows(&page, flute, first) && shows(&page, cello, first));
    assert!(shows(&page, flute, last) && !shows(&page, cello, last));
    let middle = &page.systems[page.systems.len() / 2];
    assert!(!shows(&page, flute, middle) && !shows(&page, cello, middle));
    // The piano plays everywhere, so its staves stay.
    let piano = score.parts()[0].id();
    assert!(
        page.systems
            .iter()
            .all(|system| shows(&page, piano, system))
    );
    // A shorter system leaves room for more on each page.
    let mut all = score.clone();
    all.set_page_setup(PageSetup::default()).unwrap();
    let full = LayoutEngine::default().layout_view(&all, 1000., ViewMode::Page);
    assert!(page.pages.len() < full.pages.len());
    // The continuous view and plain layouts keep every staff.
    let continuous = engine.layout_view(&score, 1000., ViewMode::Continuous);
    let middle = &continuous.systems[continuous.systems.len() / 2];
    assert!(shows(&continuous, cello, middle));
    // With nothing playing in a system, every part stays.
    let (mut silent, _) = piece(40);
    let viola = silent.add_part(Instrument::Viola).unwrap();
    for bar in 0..40 {
        let id = silent.measures()[bar].id_at(0).unwrap();
        silent
            .remove(ElementId {
                part: silent.parts()[0].id(),
                staff: StaffId::Upper,
                measure: bar,
                event: id,
            })
            .unwrap();
    }
    silent
        .set_page_setup(PageSetup {
            hide_empty_staves: true,
            ..PageSetup::default()
        })
        .unwrap();
    let layout = LayoutEngine::default().layout_view(&silent, 1000., ViewMode::Page);
    assert!(
        layout
            .systems
            .iter()
            .all(|system| shows(&layout, viola, system))
    );
}

#[test]
fn every_example_lays_out_with_empty_staves_hidden() {
    use score_core::PageSetup;
    for bytes in [
        include_bytes!("../../../examples/orquesta-completa.partitura").as_slice(),
        include_bytes!("../../../examples/menuetto-y-trio-orquesta.partitura").as_slice(),
        include_bytes!("../../../examples/tres-luces-orquesta.partitura").as_slice(),
        include_bytes!("../../../examples/divisi-por-pasaje.partitura").as_slice(),
        include_bytes!("../../../examples/voces-y-divisi.partitura").as_slice(),
        include_bytes!("../../../examples/umbral-y-lumbre-concierto-viola.partitura").as_slice(),
    ] {
        let mut score = score_io::decode(bytes).unwrap();
        score
            .set_page_setup(PageSetup {
                hide_empty_staves: true,
                ..PageSetup::default()
            })
            .unwrap();
        let layout = LayoutEngine::default().layout_view(&score, 1000., ViewMode::Page);
        assert!(!layout.pages.is_empty());
    }
}

#[test]
fn a_bar_repeat_draws_percent_instead_of_a_rest() {
    let (mut score, _) = piece(3);
    let piano = score.parts()[0].id();
    score
        .set_bar_repeat(piano, StaffId::Upper, 1, true)
        .unwrap();
    let layout = LayoutEngine::default().layout(&score, 1000.);
    let bar = layout.measure_on(piano, StaffId::Upper, 1).unwrap();
    let percent = bar
        .content
        .glyphs
        .iter()
        .find(|g| g.text == "\u{e500}")
        .unwrap();
    assert_eq!(percent.position.y, 20.);
    assert!(!bar.content.glyphs.iter().any(|g| g.text == "\u{e4e3}"));
    // The lower staff keeps its whole-bar rest.
    let lower = layout.measure_on(piano, StaffId::Lower, 1).unwrap();
    assert!(!lower.content.glyphs.iter().any(|g| g.text == "\u{e500}"));
}

#[test]
fn wider_margins_keep_the_music_inside_the_same_page() {
    use score_core::{Margins, PageSetup};
    let (mut score, _) = piece(40);
    let layout = |score: &Score| LayoutEngine::default().layout_view(score, 1000., ViewMode::Page);
    let normal = layout(&score);
    assert_eq!(normal.pages[0].margin, 0.);
    score
        .set_page_setup(PageSetup {
            margins: Margins::ExtraWide,
            ..PageSetup::default()
        })
        .unwrap();
    let wide = layout(&score);
    let page = &wide.pages[0];
    // 12 mm of a 210 mm page that is 1000 units wide.
    let margin = 12. * 1000. / 210.;
    assert!((page.margin - margin).abs() < 0.01);
    assert!((page.width - normal.pages[0].width).abs() < 0.01);
    for measure in &wide.measures {
        assert!(measure.origin.x >= margin);
        assert!(measure.origin.x + measure.width <= page.width - margin + 0.5);
    }
    assert!(wide.labels.iter().all(|label| label.position.x >= margin));
    assert!(wide.systems[0].top >= normal.systems[0].top + margin - 0.01);
    assert!(wide.pages.len() >= normal.pages.len());
}
