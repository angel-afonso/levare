use super::*;
use score_core::{BarForm, RehearsalStyle};
fn score() -> Score {
    Score::new("Rehearsal".into(), &[Instrument::Violin], 64).unwrap()
}
#[test]
fn rehearsal_mark_is_boxed_above_the_first_staff() {
    let mut score = score();
    score.toggle_rehearsal_mark(0).unwrap();
    let layout = LayoutEngine::default().layout(&score, 1000.);
    assert_eq!(layout.rehearsals.len(), 1);
    let label = &layout.rehearsals[0];
    assert_eq!(label.text, "A");
    assert_eq!(label.frame.len(), 4);
    assert!(
        label
            .frame
            .iter()
            .all(|line| line.from.y < layout.measures[0].origin.y)
    );
}
#[test]
fn tempo_moves_right_of_a_rehearsal_mark_on_the_same_bar() {
    let mut score = score();
    score.toggle_rehearsal_mark(0).unwrap();
    score
        .set_bar_form(
            0,
            BarForm {
                segno: true,
                coda: true,
                ..Default::default()
            },
        )
        .unwrap();
    let layout = LayoutEngine::default().layout(&score, 1000.);
    let right = layout.rehearsals[0].frame[0].to.x;
    assert!(layout.tempos[0].position.x > right + 10.);
    assert!(layout.navigation[0].position.x > layout.tempos[0].position.x + layout.tempos[0].width);
}
#[test]
fn style_change_relabels_without_moving_bars() {
    let mut score = score();
    for bar in [0, 25, 48, 63] {
        score.toggle_rehearsal_mark(bar).unwrap();
    }
    let mut engine = LayoutEngine::default();
    for part in [false, true] {
        for mode in [ViewMode::Continuous, ViewMode::Page, ViewMode::SingleSystem] {
            score.set_rehearsal_style(RehearsalStyle::Letters);
            let base = if part {
                engine.layout_part(&score, 900., mode)
            } else {
                engine.layout_view(&score, 900., mode)
            };
            for style in [RehearsalStyle::Numbers, RehearsalStyle::BarNumbers] {
                score.set_rehearsal_style(style);
                let changed = if part {
                    engine.layout_part(&score, 900., mode)
                } else {
                    engine.layout_view(&score, 900., mode)
                };
                assert_eq!(
                    base.measures
                        .iter()
                        .map(|m| (m.index, m.origin.x, m.origin.y, m.width))
                        .collect::<Vec<_>>(),
                    changed
                        .measures
                        .iter()
                        .map(|m| (m.index, m.origin.x, m.origin.y, m.width))
                        .collect::<Vec<_>>()
                );
            }
        }
    }
}
#[test]
fn wide_custom_labels_keep_box_and_combined_headings_inside_every_view() {
    let mut score = score();
    for bar in [0, 25, 48, 63] {
        score
            .set_rehearsal_custom(bar, Some("WW音楽MMMM".into()))
            .unwrap();
        score
            .set_bar_form(
                bar,
                BarForm {
                    segno: true,
                    coda: true,
                    ..Default::default()
                },
            )
            .unwrap();
        score
            .set_tempo_at(
                bar,
                score_core::TempoMark {
                    text: "Allegro molto".into(),
                    bpm: 80 + bar as u16,
                    ..Default::default()
                },
            )
            .unwrap();
    }
    for part in [false, true] {
        for mode in [ViewMode::Continuous, ViewMode::Page, ViewMode::SingleSystem] {
            let mut engine = LayoutEngine::default();
            let layout = if part {
                engine.layout_part(&score, 1000., mode)
            } else {
                engine.layout_view(&score, 1000., mode)
            };
            for label in &layout.rehearsals {
                let system = layout
                    .systems
                    .iter()
                    .find(|s| (s.first_measure..=s.last_measure).contains(&label.measure))
                    .unwrap();
                assert!(label.frame[0].to.x - label.frame[0].from.x >= 6. * 18. + 2. * 18. + 10.);
                for line in &label.frame {
                    assert!(line.from.y - 0.81 >= system.top - 0.02);
                    assert!(line.from.y + 0.81 <= system.bottom + 0.02);
                    assert!(line.from.x >= 0. && line.from.x <= layout.width);
                }
                let tempo = layout
                    .tempos
                    .iter()
                    .find(|t| t.measure == label.measure)
                    .unwrap();
                assert!(tempo.position.x > label.frame[0].to.x + 10.);
                for nav in layout
                    .navigation
                    .iter()
                    .filter(|n| n.measure == label.measure)
                {
                    assert!(nav.position.x + 40. <= layout.width);
                }
                if mode == ViewMode::Page {
                    let page = layout
                        .pages
                        .iter()
                        .find(|p| (p.first_measure..=p.last_measure).contains(&label.measure))
                        .unwrap();
                    assert!(label.frame[0].from.y >= page.top);
                    assert!(label.frame[2].from.y <= page.top + page.height);
                }
            }
        }
    }
}
