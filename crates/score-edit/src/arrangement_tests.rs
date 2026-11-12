use super::*;
use score_core::{Connection, ConnectionKind, Dynamic, Step};
#[test]
fn adding_and_removing_parts_keeps_score_order_selection_and_undo_data() {
    let mut editor = Editor::default();
    let piano = editor.cursor().part;
    editor.execute(Command::AddPart(Instrument::Cello)).unwrap();
    let cello = editor.cursor().part;
    editor.execute(Command::AddPart(Instrument::Flute)).unwrap();
    let flute = editor.cursor().part;
    assert_eq!(
        editor
            .score()
            .parts()
            .iter()
            .map(|p| p.id())
            .collect::<Vec<_>>(),
        [flute, piano, cello]
    );
    assert_eq!(editor.cursor().part, flute);
    let event = editor.note(Step::D);
    editor.execute(Command::Insert(event)).unwrap();
    let a = editor.selection().unwrap();
    editor.execute(Command::Insert(event)).unwrap();
    let b = editor.selection().unwrap();
    editor
        .execute(Command::SetDynamic(Some(Dynamic::F)))
        .unwrap();
    editor
        .execute(Command::AddConnection(Connection {
            start: a,
            end: b,
            kind: ConnectionKind::Tie,
        }))
        .unwrap();
    let before = editor.score().clone();
    editor.execute(Command::RemovePart).unwrap();
    assert_eq!(editor.cursor().part, piano);
    assert!(editor.score().connections().is_empty());
    assert!(editor.undo());
    assert_eq!(editor.score(), &before);
    assert_eq!(editor.selection(), Some(b));
    assert!(editor.redo());
    assert!(editor.score().part(flute).is_none());
    editor.select_part(cello);
    editor.execute(Command::RemovePart).unwrap();
    assert_eq!(editor.cursor().part, piano);
    let before = editor.score().clone();
    assert!(editor.execute(Command::RemovePart).is_err());
    assert_eq!(editor.score(), &before);
}

#[test]
fn a_song_keeps_the_voice_on_top_through_adding_removing_and_undo() {
    let mut editor = Editor::default();
    editor.execute(Command::AddPart(Instrument::Voice)).unwrap();
    let instruments = |editor: &Editor| -> Vec<Instrument> {
        editor
            .score()
            .parts()
            .iter()
            .map(|p| p.instrument())
            .collect()
    };
    assert_eq!(instruments(&editor), [Instrument::Voice, Instrument::Piano]);
    editor
        .execute(Command::AddPart(Instrument::Violin))
        .unwrap();
    let violin = editor.cursor().part;
    assert_eq!(
        instruments(&editor),
        [Instrument::Piano, Instrument::Voice, Instrument::Violin],
        "with strings, orchestral order"
    );
    editor.execute(Command::RemovePartById(violin)).unwrap();
    assert_eq!(instruments(&editor), [Instrument::Voice, Instrument::Piano]);
    editor.undo();
    assert_eq!(
        instruments(&editor),
        [Instrument::Piano, Instrument::Voice, Instrument::Violin]
    );
}
