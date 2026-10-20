use super::*;

fn menuetto() -> Score {
    let native: serde_json::Value = serde_json::from_slice(include_bytes!(
        "../../../examples/menuetto-y-trio-cuerdas.partitura"
    ))
    .unwrap();
    serde_json::from_value(native["score"].clone()).unwrap()
}

#[test]
fn parts_fit_longer_systems_without_repeated_instrument_names_or_changing_music() {
    let score = menuetto();
    for part in score.parts() {
        let extracted = score.extract_part(part.id()).unwrap();
        let before = extracted.clone();
        let name = score.part_name(part.id());
        let mut engine = LayoutEngine::default();
        let general = engine.layout_view(&extracted, 1000., ViewMode::Page);
        let layout = engine.layout_part(&extracted, 1000., ViewMode::Page);
        assert!(
            layout.systems[0].last_measure >= 3,
            "{name}: first system must fit at least four bars"
        );
        assert!(
            layout.systems.len() < general.systems.len(),
            "{name}: parts should fit more bars per system"
        );
        assert!(
            layout.pages[0].width > general.pages[0].width,
            "smaller printed staff size"
        );
        assert!(layout.labels.iter().all(|label| label.text != name));
        assert_eq!(
            layout.systems.last().unwrap().last_measure,
            score.measures().len() - 1
        );
        assert!(
            layout
                .systems
                .iter()
                .all(|s| s.last_measure - s.first_measure < 6)
        );
        for bar in &layout.measures {
            assert!(bar.origin.x >= 56.);
            assert!(bar.origin.x + bar.width <= layout.width + 0.01);
        }
        if part.has_passage_divisi() {
            assert!(layout.labels.iter().any(|label| label.text == "div. I"));
            assert!(layout.labels.iter().any(|label| label.text == "div. II"));
            assert!(!layout.brackets.is_empty());
        }
        let repeated = engine.layout_part(&extracted, 1000., ViewMode::Page);
        assert_eq!(engine.recomputed_measures, 0);
        assert_eq!(repeated.systems.len(), layout.systems.len());
        assert_eq!(before, extracted);
    }
}

#[test]
fn piano_part_keeps_hand_labels_and_full_score_keeps_instrument_labels() {
    let score = Score::default();
    let mut engine = LayoutEngine::default();
    let part = engine.layout_part(&score, 1000., ViewMode::Page);
    assert!(part.labels.iter().any(|label| label.text == "M. derecha"));
    assert!(part.labels.iter().any(|label| label.text == "M. izquierda"));
    assert!(!part.labels.iter().any(|label| label.text == "Piano"));
    assert!(!part.braces.is_empty());
    let full = engine.layout_view(&score, 1000., ViewMode::Page);
    assert!(full.labels.iter().any(|label| label.text == "Piano"));
    let ensemble = menuetto();
    let layout = engine.layout_part(&ensemble, 1000., ViewMode::Page);
    for instrument in ensemble.parts() {
        assert!(
            layout
                .labels
                .iter()
                .any(|label| label.text == ensemble.part_name(instrument.id()))
        );
    }
}

#[test]
fn adjacent_tempo_headings_in_parts_have_room_and_do_not_leave_the_system() {
    let mut score = Score::default();
    let mark = score_core::TempoMark {
        text: "Allegro molto espressivo".into(),
        ..score.tempo_at(0).clone()
    };
    for bar in 0..score.measures().len() {
        let mut change = mark.clone();
        change.bpm += bar as u16;
        score.set_tempo_at(bar, change).unwrap();
    }
    let layout = LayoutEngine::default().layout_part(&score, 1000., ViewMode::Page);
    for system in &layout.systems {
        let headings: Vec<_> = layout
            .tempos
            .iter()
            .filter(|t| (system.first_measure..=system.last_measure).contains(&t.measure))
            .collect();
        for heading in &headings {
            assert!(heading.position.x + heading.width <= layout.width - 12.);
        }
        for pair in headings.windows(2) {
            assert!(pair[0].position.x + pair[0].width + 10. <= pair[1].position.x);
        }
    }
}

#[test]
fn late_tempo_heading_stays_on_the_page_after_system_justification() {
    let mut score = Score::default();
    let mark = score_core::TempoMark {
        text: "Andante molto espressivo e cantabile".into(),
        ..score.tempo_at(0).clone()
    };
    score.set_tempo_at(3, mark).unwrap();
    let layout = LayoutEngine::default().layout_part(&score, 1000., ViewMode::Page);
    let heading = layout.tempos.iter().find(|t| t.measure == 3).unwrap();
    assert!(
        heading.position.x + heading.width + 12. <= layout.width,
        "heading {:?}, page {}, measures {:?}",
        heading,
        layout.width,
        layout
            .measures
            .iter()
            .map(|m| (m.index, m.origin.x, m.width, m.content.x_at_tick(0.)))
            .collect::<Vec<_>>()
    );
    for system in &layout.systems {
        let right = layout
            .measures
            .iter()
            .filter(|m| m.system_start == system.first_measure)
            .map(|m| m.origin.x + m.width)
            .fold(0f32, f32::max);
        assert!((right - (layout.width - 32.)).abs() < 0.01);
    }
}

#[test]
fn part_systems_and_the_trio_indications_use_compact_vertical_spacing() {
    let score = menuetto();
    let part = score.extract_part(score.parts()[0].id()).unwrap();
    let layout = LayoutEngine::default().layout_part(&part, 1000., ViewMode::Page);
    for systems in layout.systems.windows(2) {
        if layout.page_for_measure(systems[0].first_measure)
            == layout.page_for_measure(systems[1].first_measure)
        {
            let gap = systems[1].top - systems[0].bottom;
            assert!(
                (12. ..=20.).contains(&gap),
                "system gap overlaps or wastes space: {gap}"
            );
        }
    }
    let bar = layout.measures.iter().find(|m| m.index == 48).unwrap();
    let arco = bar
        .content
        .techniques
        .iter()
        .find(|t| t.text == "arco")
        .unwrap();
    let tempo = layout.tempos.iter().find(|t| t.measure == 48).unwrap();
    assert!(
        -arco.position.y <= 60.,
        "arco too far from staff: {}",
        arco.position.y
    );
    assert!(
        bar.origin.y - tempo.position.y <= 90.,
        "tempo too far from staff"
    );
    assert!(bar.origin.y + arco.position.y - 16. - (tempo.position.y + 6.) >= 5.);
}

#[test]
fn compact_part_system_bounds_contain_the_final_painted_ink() {
    let mut scores = vec![menuetto()];
    let piano: serde_json::Value =
        serde_json::from_slice(include_bytes!("../../../examples/piano-arpegios.partitura"))
            .unwrap();
    scores.push(serde_json::from_value(piano["score"].clone()).unwrap());
    let metrics = Metrics::bravura();
    for score in scores {
        for part in score.parts() {
            let extracted = score.extract_part(part.id()).unwrap();
            let layout = LayoutEngine::default().layout_part(&extracted, 1000., ViewMode::Page);
            for bar in &layout.measures {
                let system = layout
                    .systems
                    .iter()
                    .find(|s| (s.first_measure..=s.last_measure).contains(&bar.index))
                    .unwrap();
                for glyph in &bar.content.glyphs {
                    let (_, top, _, bottom) = metrics.bounds(&glyph.text, glyph.rotated);
                    assert!(bar.origin.y + glyph.position.y + top >= system.top - 0.01);
                    assert!(bar.origin.y + glyph.position.y + bottom <= system.bottom + 0.01);
                }
                for label in &bar.content.techniques {
                    assert!(bar.origin.y + label.position.y - 16. >= system.top - 0.01);
                    assert!(bar.origin.y + label.position.y + 4. <= system.bottom + 0.01);
                }
                let page = &layout.pages[layout.page_for_measure(bar.index).unwrap()];
                assert!(
                    system.top
                        >= page.top + if page.first_measure == 0 { 160. } else { 72. } - 0.01
                );
                assert!(system.bottom <= page.top + page.height - 55.);
            }
            for curve in &layout.curves {
                let system = layout
                    .systems
                    .iter()
                    .find(|s| (s.top..=s.bottom).contains(&curve.from.y))
                    .expect("curve must belong to a visible system");
                for i in 0..=8 {
                    let t = i as f32 / 8.;
                    let u = 1. - t;
                    let y = u * u * u * curve.from.y
                        + 3. * u * u * t * (curve.from.y + curve.bend)
                        + 3. * u * t * t * (curve.to.y + curve.bend)
                        + t * t * t * curve.to.y;
                    assert!(
                        y >= system.top && y <= system.bottom,
                        "curve outside system bounds"
                    );
                }
            }
            for hairpin in &layout.hairpins {
                let system = layout
                    .systems
                    .iter()
                    .find(|s| (s.top..=s.bottom).contains(&hairpin.upper.from.y))
                    .expect("hairpin must belong to a visible system");
                for y in [
                    hairpin.upper.from.y,
                    hairpin.upper.to.y,
                    hairpin.lower.from.y,
                    hairpin.lower.to.y,
                ] {
                    assert!(y >= system.top && y <= system.bottom);
                }
            }
            for tempo in &layout.tempos {
                let system = layout
                    .systems
                    .iter()
                    .find(|s| (s.first_measure..=s.last_measure).contains(&tempo.measure))
                    .unwrap();
                assert!(tempo.position.y - 30. >= system.top - 0.01);
                assert!(tempo.position.y + 6. <= system.bottom);
            }
        }
    }
}
