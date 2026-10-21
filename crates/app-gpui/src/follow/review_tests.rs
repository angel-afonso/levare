use super::*;
use score_core::{BarForm, BarMark, BarMarkRef, Instrument, Jump, Offset, Score};
use score_layout::{LayoutEngine, ViewMode};

#[test]
fn form_issues_reveal_the_header_geometry_and_offsets_in_tall_scores_at_any_zoom() {
    let mut score = Score::new("Orquesta".into(), &Instrument::ORCHESTRA, 4).unwrap();
    score
        .set_bar_form(
            2,
            BarForm {
                jump: Some(Jump::DalSegno),
                ..Default::default()
            },
        )
        .unwrap();
    for mode in [ViewMode::Page, ViewMode::Continuous, ViewMode::SingleSystem] {
        let mut engine = LayoutEngine::default();
        for scale in [0.5, 1., 1.5] {
            let mut layout = engine.layout_view(&score, 1000., mode);
            layout.scale = scale;
            let before = review_bar_bounds(&layout, 2, Some(BarMark::Jump)).unwrap();
            let label = layout
                .navigation
                .iter()
                .find(|label| label.measure == 2 && label.mark == BarMark::Jump)
                .unwrap();
            assert!(before.contains(&point(
                px(label.position.x * scale),
                px(label.position.y * scale)
            )));
            let lowest = layout
                .measures
                .iter()
                .filter(|measure| measure.index == 2)
                .map(|measure| measure.origin.y)
                .fold(0., f32::max);
            assert!(before.bottom() < px(lowest * scale));
            layout.translate_bar_mark(
                BarMarkRef {
                    measure: 2,
                    mark: BarMark::Jump,
                },
                65.,
                35.,
            );
            let moved = review_bar_bounds(&layout, 2, Some(BarMark::Jump)).unwrap();
            assert!((moved.origin.x - before.origin.x - px(65. * scale)).abs() < px(0.01));
            assert!((moved.origin.y - before.origin.y - px(35. * scale)).abs() < px(0.01));
            let repeat = review_bar_bounds(&layout, 2, None).unwrap();
            assert!(repeat.bottom() < px(lowest * scale));
        }
    }
    score
        .set_bar_mark_offset(
            BarMarkRef {
                measure: 2,
                mark: BarMark::Jump,
            },
            Offset { dx: 65, dy: 35 },
        )
        .unwrap();
    let layout = LayoutEngine::default().layout_view(&score, 1000., ViewMode::Continuous);
    let target = review_bar_bounds(&layout, 2, Some(BarMark::Jump)).unwrap();
    let label = layout
        .navigation
        .iter()
        .find(|label| label.measure == 2 && label.mark == BarMark::Jump)
        .unwrap();
    assert!(target.contains(&point(px(label.position.x), px(label.position.y))));
}
