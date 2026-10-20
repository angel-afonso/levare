use super::*;

fn menuetto() -> Score {
    let native: serde_json::Value = serde_json::from_slice(include_bytes!(
        "../../../examples/menuetto-y-trio-cuerdas.partitura"
    ))
    .unwrap();
    serde_json::from_value(native["score"].clone()).unwrap()
}

#[test]
fn full_score_uses_compact_spacing_on_screen_and_on_printed_pages() {
    let score = menuetto();
    let before = score.clone();
    let mut engine = LayoutEngine::default();
    for mode in [ViewMode::Page, ViewMode::Continuous, ViewMode::SingleSystem] {
        let layout = engine.layout_view(&score, 1000., mode);
        for pair in layout.systems.windows(2) {
            if layout.page_for_measure(pair[0].first_measure)
                == layout.page_for_measure(pair[1].first_measure)
            {
                let gap = pair[1].top - pair[0].bottom;
                assert!((12. ..=20.).contains(&gap), "{mode:?}: gap {gap}");
            }
        }
        let bar = layout
            .measures
            .iter()
            .find(|m| m.part == score.parts()[0].id() && m.index == 48 && m.staff == StaffId::Upper)
            .unwrap();
        let arco = bar
            .content
            .techniques
            .iter()
            .find(|t| t.text == "arco")
            .unwrap();
        assert!(
            -arco.position.y <= 60.,
            "{mode:?}: arco too far from its own staff: {}",
            arco.position.y
        );
        let tempo = layout.tempos.iter().find(|t| t.measure == 48).unwrap();
        assert!(
            bar.origin.y - tempo.position.y <= 90.,
            "{mode:?}: tempo too far from top staff"
        );
        for part in score.parts() {
            assert!(
                layout
                    .labels
                    .iter()
                    .any(|l| l.text == score.part_name(part.id()))
            );
        }
    }
    assert_eq!(score, before);
}

#[test]
fn high_note_drag_preview_keeps_the_displayed_frame_and_clears_its_raised_slur() {
    let mut score = menuetto();
    let connection = *score
        .connections()
        .iter()
        .find(|c| c.kind == score_core::ConnectionKind::Slur && c.start.measure == 0)
        .unwrap();
    let mut engine = LayoutEngine::default();
    let base = engine.layout_view(&score, 1600., ViewMode::Page);
    score.transpose(connection.start, 21).unwrap();
    let preview = engine.layout_pitch_preview(&score, &base);
    let curve = preview
        .curves
        .iter()
        .find(|c| c.connection == connection)
        .unwrap();
    assert!(
        curve.from.y < base.systems[0].top,
        "repro must raise the slur beyond the old system frame"
    );
    let bar = preview.event_measure(connection.start).unwrap();
    let original = base.event_measure(connection.start).unwrap();
    assert_eq!(bar.origin, original.origin);
    assert_eq!(preview.height, base.height);
    assert_eq!(preview.systems[0].top, base.systems[0].top);
    let text = bar
        .content
        .techniques
        .iter()
        .find(|t| t.text == "arco")
        .unwrap();
    let left = (bar.origin.x + text.position.x).max(curve.from.x);
    let right = (bar.origin.x + text.position.x + 28.).min(curve.to.x);
    assert!(right > left);
    for i in 0..=16 {
        let t =
            (left + (right - left) * i as f32 / 16. - curve.from.x) / (curve.to.x - curve.from.x);
        let u = 1. - t;
        let y = u * u * u * curve.from.y
            + 3. * u * u * t * (curve.from.y + curve.bend)
            + 3. * u * t * t * (curve.to.y + curve.bend)
            + t * t * t * curve.to.y;
        assert!(
            bar.origin.y + text.position.y + 10. <= y + 0.01,
            "preview text overlaps its slur"
        );
    }
}
