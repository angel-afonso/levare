use super::*;
use score_core::{Event, LayoutBreak, Notation, Pitch, Pitches, Step};

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

#[test]
fn system_break_ends_the_system_after_its_bar() {
    let (mut score, _) = piece(12);
    let mut engine = LayoutEngine::default();
    let free = engine.layout_view(&score, 1000., ViewMode::Page);
    assert!(free.systems[0].last_measure > 0);
    score.toggle_layout_break(0, LayoutBreak::System).unwrap();
    let layout = engine.layout_view(&score, 1000., ViewMode::Page);
    assert_eq!(layout.systems[0].last_measure, 0);
    assert_eq!(layout.systems[1].first_measure, 1);
}

#[test]
fn parts_ignore_breaks() {
    let (mut score, _) = piece(12);
    let part = score.parts()[0].id();
    score.toggle_layout_break(0, LayoutBreak::System).unwrap();
    let extracted = score.extract_part(part).unwrap();
    let layout = LayoutEngine::default().layout_part(&extracted, 1000., ViewMode::Page);
    assert!(layout.systems[0].last_measure > 0);
}

#[test]
fn single_system_view_ignores_breaks() {
    let (mut score, _) = piece(12);
    score.toggle_layout_break(0, LayoutBreak::System).unwrap();
    let layout = LayoutEngine::default().layout_view(&score, 1000., ViewMode::SingleSystem);
    assert_eq!(layout.systems.len(), 1);
}

#[test]
fn page_break_starts_a_new_page_only_in_page_view() {
    let (mut score, _) = piece(12);
    score.toggle_layout_break(0, LayoutBreak::Page).unwrap();
    let mut engine = LayoutEngine::default();
    let layout = engine.layout_view(&score, 1000., ViewMode::Page);
    assert!(layout.pages.len() >= 2);
    assert_eq!(layout.pages[0].last_measure, 0);
    assert_eq!(layout.pages[1].first_measure, 1);
    let continuous = engine.layout_view(&score, 1000., ViewMode::Continuous);
    assert_eq!(continuous.pages.len(), 1);
}
