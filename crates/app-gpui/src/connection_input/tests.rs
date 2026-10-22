use super::*;
use gpui::point;
use score_edit::{Command, Editor};
use score_layout::ViewMode;

/// C D rest E: four quarters in one bar, ids in order.
fn phrase() -> (Editor, Vec<ElementId>) {
    let mut editor = Editor::default();
    let mut ids = Vec::new();
    for step in [
        Some(score_core::Step::C),
        Some(score_core::Step::D),
        None,
        Some(score_core::Step::E),
    ] {
        let event = match step {
            Some(step) => editor.note(step),
            None => editor.rest(),
        };
        editor.execute(Command::Insert(event)).unwrap();
        ids.push(editor.selection().unwrap());
    }
    (editor, ids)
}

fn geometry(score: &Score) -> Layout {
    LayoutEngine::default().layout_view(score, 1000., ViewMode::Continuous)
}

#[test]
fn a_passage_slur_skips_rests_at_its_ends() {
    let (editor, ids) = phrase();
    let score = editor.score();
    assert_eq!(
        passage_endpoints(score, &ids[1..], ConnectionKind::Slur),
        Some((ids[1], ids[3]))
    );
    assert_eq!(
        passage_endpoints(score, &ids[2..], ConnectionKind::Crescendo),
        Some((ids[2], ids[3])),
        "a hairpin may start on a rest"
    );
    assert_eq!(
        passage_endpoints(score, &ids[2..], ConnectionKind::Slur),
        None
    );
}

#[test]
fn a_followed_slur_waits_on_rests() {
    let (editor, ids) = phrase();
    let score = editor.score();
    assert_eq!(
        follow(score, ids[0], ids[1], ConnectionKind::Slur).map(|c| c.end),
        Some(ids[1])
    );
    assert_eq!(follow(score, ids[0], ids[2], ConnectionKind::Slur), None);
    assert!(follow(score, ids[0], ids[2], ConnectionKind::Diminuendo).is_some());
    assert_eq!(follow(score, ids[1], ids[0], ConnectionKind::Slur), None);
}

#[test]
fn stretching_previews_and_replaces_the_line() {
    let (mut editor, ids) = phrase();
    let short = Connection {
        start: ids[0],
        end: ids[1],
        kind: ConnectionKind::Slur,
    };
    editor.execute(Command::AddConnection(short)).unwrap();
    let layout = geometry(editor.score());
    let mut drag = ConnectionDrag::stretch(
        editor.score(),
        short,
        End::End,
        point(px(0.), px(0.)),
        &layout,
    );
    assert!(drag.command().is_none(), "nothing before the pointer moves");
    assert!(drag.aim(ids[3]));
    let long = Connection {
        end: ids[3],
        ..short
    };
    assert_eq!(drag.candidate, Some(long));
    assert_eq!(drag.preview.as_ref().unwrap().connections().len(), 1);
    assert!(!drag.aim(ids[3]), "the same target keeps the preview");
    assert!(matches!(
        drag.command(),
        Some(Command::ReplaceConnection(old, new)) if old == short && new == long
    ));
    // Back over the original end: releasing there changes nothing.
    drag.aim(ids[1]);
    assert!(drag.command().is_none());
}

#[test]
fn a_dragged_end_never_crosses_the_fixed_one_or_lands_on_a_rest() {
    let (mut editor, ids) = phrase();
    let slur = Connection {
        start: ids[1],
        end: ids[3],
        kind: ConnectionKind::Slur,
    };
    editor.execute(Command::AddConnection(slur)).unwrap();
    let layout = geometry(editor.score());
    let mut drag = ConnectionDrag::stretch(
        editor.score(),
        slur,
        End::Start,
        point(px(0.), px(0.)),
        &layout,
    );
    drag.aim(ids[0]);
    assert_eq!(drag.candidate.map(|c| c.start), Some(ids[0]));
    drag.aim(ids[2]);
    assert_eq!(drag.candidate, None);
    assert!(drag.error.is_some());
    assert!(drag.command().is_none());
    drag.aim(ids[3]);
    assert_eq!(drag.candidate, None, "the start cannot pass the end");
}

#[test]
fn drawing_adds_a_new_line_and_rejects_overlaps() {
    let (mut editor, ids) = phrase();
    let layout = geometry(editor.score());
    let mut drag = ConnectionDrag::draw(
        editor.score(),
        ConnectionKind::Crescendo,
        ids[0],
        point(px(0.), px(0.)),
        &layout,
    );
    drag.aim(ids[2]);
    let command = drag.command().unwrap();
    editor.execute(command).unwrap();
    let mut overlap = ConnectionDrag::draw(
        editor.score(),
        ConnectionKind::Diminuendo,
        ids[1],
        point(px(0.), px(0.)),
        &editor_layout(&editor),
    );
    overlap.aim(ids[3]);
    assert!(overlap.command().is_none());
    assert!(overlap.error.is_some());
}

fn editor_layout(editor: &Editor) -> Layout {
    geometry(editor.score())
}

#[test]
fn handles_sit_on_both_ends_and_follow_the_zoom() {
    let (mut editor, ids) = phrase();
    let slur = Connection {
        start: ids[0],
        end: ids[3],
        kind: ConnectionKind::Slur,
    };
    editor.execute(Command::AddConnection(slur)).unwrap();
    for scale in [0.7, 1.5] {
        let mut layout = geometry(editor.score());
        layout.scale = scale;
        let (start, end) = handles(&layout, slur).unwrap();
        assert!(start.0 < end.0);
        assert_eq!(handle_at(&layout, slur, start), Some(End::Start));
        assert_eq!(
            handle_at(&layout, slur, (end.0 + 2., end.1)),
            Some(End::End)
        );
        let middle = ((start.0 + end.0) / 2., start.1);
        assert_eq!(handle_at(&layout, slur, middle), None);
    }
}

/// Typing C D rest E after S on C, the way the app follows each note.
#[test]
fn a_followed_slur_grows_with_each_note_and_one_undo_shrinks_it() {
    let mut editor = Editor::default();
    editor
        .execute(Command::Insert(editor.note(score_core::Step::C)))
        .unwrap();
    let start = editor.selection().unwrap();
    let mut current = None;
    let mut ends = Vec::new();
    for step in [Some(score_core::Step::D), None, Some(score_core::Step::E)] {
        let event = match step {
            Some(step) => editor.note(step),
            None => editor.rest(),
        };
        editor.execute(Command::Insert(event)).unwrap();
        let end = editor.selection().unwrap();
        if let Some((line, command)) =
            follow_command(editor.score(), start, end, ConnectionKind::Slur, current)
        {
            assert!(editor.execute_joined(command).unwrap());
            current = Some(line);
        }
        ends.push(current.map(|line| line.end));
    }
    assert_eq!(ends[0], ends[1], "the rest keeps the slur on D");
    assert_ne!(ends[1], ends[2]);
    assert_eq!(editor.score().connections().len(), 1);
    editor.undo();
    assert_eq!(
        editor
            .score()
            .connections()
            .iter()
            .next()
            .map(|line| line.end),
        ends[0],
        "undoing E gives the slur back its end on D"
    );
    assert_eq!(
        editor
            .score()
            .part(start.part)
            .unwrap()
            .staff(start.staff)
            .unwrap()
            .measures()[0]
            .voice_len(score_core::Voice::One),
        3
    );
}
