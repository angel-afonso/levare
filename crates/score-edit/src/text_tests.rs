use super::*;
use score_core::{MarkKind, MarkRef, Offset, Pitches, Step, TextKind};

fn note(step: Step) -> Event {
    Event {
        pitches: Pitches::single(Pitch::new(step, 5)),
        duration: Duration::Quarter,
        notation: Default::default(),
    }
}

#[test]
fn texts_are_written_moved_removed_and_undone_one_step_each() {
    let mut score = Score::default();
    let a = score.insert(0, 0, note(Step::C)).unwrap();
    let b = score.insert(0, 1, note(Step::D)).unwrap();
    let mut editor = Editor::new(score).unwrap();
    editor.select(a);
    let write = |text: &str| Command::SetText(a, TextKind::Expression, Some(text.into()));
    assert!(editor.execute(write("dolce")).unwrap());
    assert!(
        !editor.execute(write(" dolce ")).unwrap(),
        "the same words change nothing"
    );
    assert!(editor.execute(write(&"x".repeat(60))).is_err());
    let mark = MarkRef::Text(a, TextKind::Expression);
    assert_eq!(
        editor.score().mark_kind(mark),
        Some(MarkKind::Text(TextKind::Expression))
    );
    assert_eq!(
        editor.score().mark_label(mark).as_deref(),
        Some("Texto de expresión «dolce»")
    );
    let moved = MarkRef::Text(b, TextKind::Expression);
    assert!(
        editor
            .execute(Command::MoveMark(mark, moved, Offset::ZERO))
            .unwrap()
    );
    assert_eq!(editor.score().text(a, TextKind::Expression), None);
    assert_eq!(
        editor.score().text(b, TextKind::Expression).unwrap(),
        "dolce"
    );
    assert!(editor.execute(Command::RemoveMark(moved)).unwrap());
    assert!(!editor.score().has_mark(moved));
    assert!(editor.undo());
    assert!(editor.undo());
    assert_eq!(
        editor.score().text(a, TextKind::Expression).unwrap(),
        "dolce"
    );
    assert!(editor.redo());
    assert!(editor.score().has_mark(moved));
}

#[test]
fn copied_notes_bring_their_words() {
    let mut score = Score::default();
    let a = score.insert(0, 0, note(Step::E)).unwrap();
    score.append_measure().unwrap();
    let target = score.insert(1, 0, note(Step::G)).unwrap();
    score
        .set_text(a, TextKind::Technique, Some("con sord."))
        .unwrap();
    let mut editor = Editor::new(score).unwrap();
    editor.select(a);
    let clip = editor.copy_selection().unwrap();
    editor.select(target);
    editor.paste(&clip).unwrap();
    let pasted = editor.selection().unwrap();
    assert_eq!(pasted.measure, 1);
    assert_eq!(
        editor.score().text(pasted, TextKind::Technique).unwrap(),
        "con sord."
    );
}

#[test]
fn syllables_and_chords_are_written_cleared_and_undone() {
    let mut score = Score::default();
    let a = score.insert(0, 0, note(Step::C)).unwrap();
    let mut editor = Editor::new(score).unwrap();
    let syllable = |text: &str| Some((text.to_owned(), score_core::Join::Hyphen));
    assert!(
        editor
            .execute(Command::SetSyllable(a, 1, syllable("Glo")))
            .unwrap()
    );
    assert!(
        !editor
            .execute(Command::SetSyllable(a, 1, syllable("Glo")))
            .unwrap()
    );
    assert!(
        editor
            .execute(Command::SetSyllable(a, 9, syllable("No")))
            .is_err()
    );
    let chord = score_core::ChordSymbol::parse("F#m7").unwrap();
    assert!(editor.execute(Command::SetChord(a, Some(chord))).unwrap());
    assert_eq!(
        editor.score().mark_label(MarkRef::Chord(a)).as_deref(),
        Some("Cifrado F♯m7")
    );
    assert!(
        editor
            .execute(Command::RemoveMark(MarkRef::Lyric(a, 1)))
            .unwrap()
    );
    assert!(editor.score().event(a).unwrap().notation.lyrics.is_empty());
    assert!(editor.undo());
    assert_eq!(
        editor
            .score()
            .event(a)
            .unwrap()
            .notation
            .lyrics
            .verse(1)
            .unwrap()
            .text(),
        "Glo"
    );
    assert!(editor.execute(Command::SetChordsSound(false)).unwrap());
    assert!(!editor.score().chords_sound());
    assert!(editor.undo());
    assert!(editor.score().chords_sound());
}
