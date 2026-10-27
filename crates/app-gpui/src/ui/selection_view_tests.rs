use super::selection_view::{
    EditorKind, MAX_MARK_ROWS, MarkRemoval, SelectionView, selection_view,
};
use score_core::{
    Articulation, ChordSymbol, Connection, ConnectionKind, Duration, Dynamic, Event, GraceKind,
    Instrument, Join, MarkRef, Notation, Pitch, Pitches, Score, Step, Syllable, TextKind,
};
use score_edit::Editor;

fn note(step: Step) -> Event {
    Event {
        pitches: Pitches::single(Pitch::new(step, 4)),
        duration: Duration::Quarter,
        notation: Notation::default(),
    }
}

fn rest() -> Event {
    Event {
        pitches: Pitches::default(),
        ..note(Step::C)
    }
}

#[test]
fn nothing_selected_summarises_the_score() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../examples/cuarteto-en-sol.partitura"
    );
    let score = score_io::load(std::path::Path::new(path)).unwrap();
    let editor = Editor::new(score).unwrap();
    let SelectionView::Score(summary) = selection_view(editor.score(), &editor, None) else {
        panic!("nothing selected shows the score");
    };
    assert_eq!(summary.title, "Cuarteto en Sol");
    assert_eq!(summary.key, "Sol mayor");
    assert_eq!(summary.time, "4/4");
    assert_eq!(summary.bars, 8);
    assert_eq!(summary.parts, 5);
    // Eight bars of 4/4 at ♩ = 120.
    assert_eq!(summary.duration, "0:16");
}

#[test]
fn a_note_lists_its_marks_with_their_removal() {
    let mut score = Score::default();
    let a = score.insert(0, 0, note(Step::C)).unwrap();
    let b = score.insert(0, 1, note(Step::D)).unwrap();
    score
        .set_articulation(a, Some(Articulation::Staccato))
        .unwrap();
    score.set_dynamic(a, Some(Dynamic::Mf)).unwrap();
    let slur = Connection {
        start: a,
        end: b,
        kind: ConnectionKind::Slur,
    };
    score.add_connection(slur).unwrap();
    let mut editor = Editor::new(score).unwrap();
    editor.select(a);

    let SelectionView::Note(summary) = selection_view(editor.score(), &editor, None) else {
        panic!("one note selected");
    };
    assert_eq!(summary.marks.len(), 3);
    let removals: Vec<_> = summary.marks.iter().map(|row| row.remove).collect();
    assert!(removals.contains(&MarkRemoval::Mark(MarkRef::Articulation(a))));
    assert!(removals.contains(&MarkRemoval::Mark(MarkRef::Dynamic(a))));
    assert!(removals.contains(&MarkRemoval::Connection(slur)));
    assert!(summary.marks.iter().all(|row| row.bar == 1));

    removes_each_and_undo_restores(&mut editor, removals);
}

fn written(editor: &Editor, removal: MarkRemoval) -> bool {
    match removal {
        MarkRemoval::Mark(mark) => editor.score().has_mark(mark),
        MarkRemoval::Connection(connection) => editor.score().connections().contains(&connection),
    }
}

fn removes_each_and_undo_restores(editor: &mut Editor, removals: Vec<MarkRemoval>) {
    for removal in removals {
        assert!(written(editor, removal), "{removal:?} written");
        assert!(editor.execute(removal.command()).unwrap());
        assert!(!written(editor, removal), "{removal:?} removed");
        assert!(editor.undo());
        assert!(written(editor, removal), "{removal:?} restored");
    }
}

#[test]
fn graces_words_and_an_ending_connection_are_removed_and_restored() {
    let mut score = Score::default();
    let a = score.insert(0, 0, note(Step::C)).unwrap();
    let b = score.insert(0, 1, note(Step::C)).unwrap();
    score.set_grace(b, Some(GraceKind::Acciaccatura)).unwrap();
    score
        .set_syllable(b, 0, Some(Syllable::new("la", Join::Word).unwrap()))
        .unwrap();
    score
        .set_text(b, TextKind::Expression, Some("dolce"))
        .unwrap();
    score
        .set_chord(b, Some(ChordSymbol::parse("C").unwrap()))
        .unwrap();
    let tie = Connection {
        start: a,
        end: b,
        kind: ConnectionKind::Tie,
    };
    score.add_connection(tie).unwrap();
    let mut editor = Editor::new(score).unwrap();
    editor.select(b);

    let SelectionView::Note(summary) = selection_view(editor.score(), &editor, None) else {
        panic!("one note selected");
    };
    let removals: Vec<_> = summary.marks.iter().map(|row| row.remove).collect();
    assert_eq!(
        removals,
        [
            MarkRemoval::Connection(tie),
            MarkRemoval::Mark(MarkRef::Grace(b, 0)),
            MarkRemoval::Mark(MarkRef::Text(b, TextKind::Expression)),
            MarkRemoval::Mark(MarkRef::Lyric(b, 0)),
            MarkRemoval::Mark(MarkRef::Chord(b)),
        ]
    );
    removes_each_and_undo_restores(&mut editor, removals);
}

#[test]
fn a_whole_staff_lists_a_capped_number_of_rows_and_counts_them_all() {
    let mut score = Score::default();
    for _ in 0..50 {
        score.append_measure().unwrap();
    }
    let mut ids = Vec::new();
    for measure in 0..50 {
        for index in 0..4 {
            ids.push(score.insert(measure, index, note(Step::C)).unwrap());
        }
    }
    for id in &ids {
        score
            .set_articulation(*id, Some(Articulation::Staccato))
            .unwrap();
    }
    for pair in ids.chunks(2) {
        score
            .add_connection(Connection {
                start: pair[0],
                end: pair[1],
                kind: ConnectionKind::Slur,
            })
            .unwrap();
    }
    let mut editor = Editor::new(score).unwrap();
    editor.select(ids[0]);
    editor.extend_selection(*ids.last().unwrap());

    let SelectionView::Passage(summary) = selection_view(editor.score(), &editor, None) else {
        panic!("a passage selected");
    };
    assert_eq!(summary.notes, 200);
    assert_eq!(
        summary.mark_count, 300,
        "200 staccatos and 100 slurs, each once"
    );
    assert_eq!(summary.marks.len(), MAX_MARK_ROWS);
    assert!(matches!(
        summary.marks[1].remove,
        MarkRemoval::Connection(slur) if slur.start == ids[0]
    ));
}

#[test]
fn transposing_part_shows_written_and_sounding() {
    let mut score = Score::default();
    let clarinet = score.add_part(Instrument::ClarinetBb).unwrap();
    let id = score.insert_in(clarinet, 0, 0, note(Step::D)).unwrap();
    let piano = score.insert(0, 0, note(Step::D)).unwrap();
    let mut editor = Editor::new(score).unwrap();
    editor.select(id);

    let SelectionView::Note(summary) = selection_view(editor.score(), &editor, None) else {
        panic!("one note selected");
    };
    assert_eq!(summary.part, "Clarinete en Si♭");
    assert_eq!(summary.pitch.as_deref(), Some("Re 4"));
    assert_eq!(summary.sounding.as_deref(), Some("Do 4"));

    editor.select(piano);
    let SelectionView::Note(summary) = selection_view(editor.score(), &editor, None) else {
        panic!("one note selected");
    };
    assert_eq!(summary.pitch.as_deref(), Some("Re 4"));
    assert_eq!(summary.sounding, None, "a concert part says nothing more");
}

#[test]
fn a_passage_counts_its_notes_bars_and_marks() {
    let mut score = Score::default();
    let a = score.insert(0, 0, note(Step::C)).unwrap();
    let b = score.insert(0, 1, note(Step::D)).unwrap();
    let c = score.insert(1, 0, note(Step::E)).unwrap();
    let r = score.insert(1, 1, rest()).unwrap();
    let after = score.insert(2, 0, note(Step::F)).unwrap();
    score
        .set_articulation(a, Some(Articulation::Staccato))
        .unwrap();
    score.set_dynamic(c, Some(Dynamic::P)).unwrap();
    score.set_dynamic(after, Some(Dynamic::F)).unwrap();
    score
        .add_connection(Connection {
            start: a,
            end: b,
            kind: ConnectionKind::Slur,
        })
        .unwrap();
    let mut editor = Editor::new(score).unwrap();
    editor.select(a);
    editor.extend_selection(r);

    let SelectionView::Passage(summary) = selection_view(editor.score(), &editor, None) else {
        panic!("a passage selected");
    };
    assert_eq!(summary.parts, ["Piano"]);
    assert_eq!((summary.first_bar, summary.last_bar), (1, 2));
    assert_eq!(summary.notes, 3, "the rest is not a note");
    // A slur inside the passage is listed once; the mark after it is not.
    assert_eq!(summary.marks.len(), 3);
    assert_eq!(summary.mark_count, 3);
    assert_eq!(
        summary.marks.iter().map(|row| row.bar).collect::<Vec<_>>(),
        [1, 1, 2]
    );
}

#[test]
fn an_open_editor_wins_over_the_selection() {
    let mut score = Score::default();
    let a = score.insert(0, 0, note(Step::C)).unwrap();
    let mut editor = Editor::new(score).unwrap();
    editor.select(a);
    assert!(matches!(
        selection_view(editor.score(), &editor, None),
        SelectionView::Note(_)
    ));
    for kind in [EditorKind::Key, EditorKind::Lyric, EditorKind::ScoreInfo] {
        assert_eq!(
            selection_view(editor.score(), &editor, Some(kind)),
            SelectionView::Editor(kind)
        );
    }
    editor.execute(score_edit::Command::Erase).unwrap();
    assert!(matches!(
        selection_view(editor.score(), &editor, None),
        SelectionView::Rest(_)
    ));
}

#[test]
fn moving_the_selection_keeps_only_the_editors_that_follow_the_bar() {
    use EditorKind::*;
    for kind in [Clef, Key, Time] {
        assert!(kind.survives_selection_move(), "{kind:?}");
    }
    for kind in [Text, Lyric, Chord, ScoreInfo, Tempo] {
        assert!(!kind.survives_selection_move(), "{kind:?}");
    }
}
