use super::*;
use score_core::{Pitches, Step};

fn quarter(step: Step) -> Event {
    Event {
        pitches: Pitches::single(Pitch::new(step, 4)),
        duration: Duration::Quarter,
        notation: Default::default(),
    }
}

#[test]
fn bar_navigation_goes_to_bar_starts_and_lands_on_empty_bars() {
    let mut score = Score::default();
    let a = score.insert(0, 0, quarter(Step::C)).unwrap();
    let b = score.insert(0, 1, quarter(Step::D)).unwrap();
    let c = score.insert(2, 0, quarter(Step::E)).unwrap();
    let mut editor = Editor::new(score).unwrap();
    editor.select(b);

    // From inside a bar, ⌘← goes to its start; from its start, to the previous bar.
    assert!(editor.navigate_bar(false));
    assert_eq!(editor.selection(), Some(a));
    assert!(!editor.navigate_bar(false));
    assert_eq!(editor.selection(), Some(a));

    // An empty bar takes the cursor; the next one with music selects its first note.
    assert!(editor.navigate_bar(true));
    assert_eq!(editor.selection(), None);
    assert_eq!((editor.cursor().measure, editor.cursor().index), (1, 0));
    assert!(editor.navigate_bar(true));
    assert_eq!(editor.selection(), Some(c));

    // Back from the bar's start reaches the empty bar before it.
    assert!(editor.navigate_bar(false));
    assert_eq!(editor.selection(), None);
    assert_eq!(editor.cursor().measure, 1);
    assert!(editor.navigate_bar(false));
    assert_eq!(editor.selection(), Some(a));

    let last = editor.score().measures().len() - 1;
    for _ in 0..last {
        editor.navigate_bar(true);
    }
    assert_eq!(editor.cursor().measure, last);
    assert!(!editor.navigate_bar(true));
}
