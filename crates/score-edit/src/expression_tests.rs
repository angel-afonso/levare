use super::*;
use score_core::{Articulation, Connection, ConnectionKind, Dynamic, Notation, Pitches, Step};
#[test]
fn expression_edits_undo_and_invalid_edits_never_enter_history() {
    let mut editor = Editor::default();
    let event = Event {
        pitches: Pitches::single(Pitch::new(Step::C, 4)),
        duration: Duration::Quarter,
        notation: Notation::default(),
    };
    editor.execute(Command::Insert(event)).unwrap();
    let a = editor.selection().unwrap();
    editor.execute(Command::Insert(event)).unwrap();
    let b = editor.selection().unwrap();
    editor.select(a);
    let plain = editor.score().clone();
    editor
        .execute(Command::SetDynamic(Some(Dynamic::P)))
        .unwrap();
    editor
        .execute(Command::SetArticulation(Some(Articulation::Accent)))
        .unwrap();
    editor
        .execute(Command::AddConnection(Connection {
            start: a,
            end: b,
            kind: ConnectionKind::Tie,
        }))
        .unwrap();
    let decorated = editor.score().clone();
    assert!(
        !editor
            .execute(Command::SetArticulation(Some(Articulation::Accent)))
            .unwrap()
    );
    assert!(
        editor
            .execute(Command::AddConnection(Connection {
                start: b,
                end: a,
                kind: ConnectionKind::Slur
            }))
            .is_err()
    );
    editor.execute(Command::Transpose(1)).unwrap();
    assert!(editor.score().connections().is_empty());
    assert!(editor.undo());
    assert_eq!(editor.score(), &decorated);
    for _ in 0..3 {
        assert!(editor.undo());
    }
    assert_eq!(editor.score(), &plain);
    for _ in 0..3 {
        assert!(editor.redo());
    }
    assert_eq!(editor.score(), &decorated);
    editor.select(b);
    editor.execute(Command::Delete).unwrap();
    assert!(editor.score().connections().is_empty());
    editor.undo();
    assert_eq!(editor.score(), &decorated);
}

#[test]
fn removing_one_outgoing_tie_keeps_the_incoming_link_in_a_chain() {
    let mut editor = Editor::default();
    let event = editor.note(Step::C);
    let mut ids = Vec::new();
    for _ in 0..3 {
        editor.execute(Command::Insert(event)).unwrap();
        ids.push(editor.selection().unwrap());
    }
    let incoming = Connection {
        start: ids[0],
        end: ids[1],
        kind: ConnectionKind::Tie,
    };
    let outgoing = Connection {
        start: ids[1],
        end: ids[2],
        kind: ConnectionKind::Tie,
    };
    editor.execute(Command::AddConnection(incoming)).unwrap();
    editor.execute(Command::AddConnection(outgoing)).unwrap();
    editor.select(ids[1]);
    editor.execute(Command::RemoveConnection(outgoing)).unwrap();
    assert_eq!(
        editor
            .score()
            .connections()
            .iter()
            .copied()
            .collect::<Vec<_>>(),
        vec![incoming]
    );
    editor.undo();
    assert_eq!(editor.score().connections().len(), 2);
}

#[test]
fn string_techniques_are_persistent_undoable_and_reject_other_instruments() {
    use score_core::{Instrument, PlayingTechnique as T};
    let mut s = Score::default();
    let piano = s
        .insert(
            0,
            0,
            Event {
                pitches: Pitches::default(),
                duration: Duration::Quarter,
                notation: Notation::default(),
            },
        )
        .unwrap();
    assert!(s.set_technique(piano, Some(T::Pizzicato)).is_err());
    for instrument in [
        Instrument::Violin,
        Instrument::Viola,
        Instrument::Cello,
        Instrument::DoubleBass,
    ] {
        let part = s.add_part(instrument).unwrap();
        let a = s
            .insert_in(
                part,
                0,
                0,
                Event {
                    pitches: Pitches::default(),
                    duration: Duration::Quarter,
                    notation: Notation::default(),
                },
            )
            .unwrap();
        let b = s
            .insert_in(
                part,
                1,
                0,
                Event {
                    pitches: Pitches::single(Pitch::new(Step::C, 4)),
                    duration: Duration::Whole,
                    notation: Notation::default(),
                },
            )
            .unwrap();
        let mut e = Editor::new(s.clone()).unwrap();
        e.select(a);
        assert!(
            e.execute(Command::SetTechnique(Some(T::Pizzicato)))
                .unwrap()
        );
        assert!(
            !e.execute(Command::SetTechnique(Some(T::Pizzicato)))
                .unwrap()
        );
        assert_eq!(e.score().technique_at(b), Some(T::Pizzicato));
        e.select(b);
        e.execute(Command::SetTechnique(Some(T::Arco))).unwrap();
        assert_eq!(e.score().technique_at(b), Some(T::Arco));
        e.undo();
        assert_eq!(e.score().technique_at(b), Some(T::Pizzicato));
        e.redo();
        e.execute(Command::SetTechnique(None)).unwrap();
        assert_eq!(e.score().technique_at(b), Some(T::Pizzicato));
        e.score().validate().unwrap();
    }
}

#[test]
fn hairpins_undo_delete_and_remove_from_the_middle_as_atomic_edits() {
    let mut e = Editor::default();
    let mut ids = Vec::new();
    for _ in 0..3 {
        e.execute(Command::Insert(e.note(Step::C))).unwrap();
        ids.push(e.selection().unwrap());
    }
    let before = e.score().clone();
    let c = Connection {
        start: ids[0],
        end: ids[2],
        kind: ConnectionKind::Crescendo,
    };
    e.execute(Command::AddConnection(c)).unwrap();
    assert!(!e.execute(Command::AddConnection(c)).unwrap());
    let marked = e.score().clone();
    assert!(
        e.execute(Command::AddConnection(Connection {
            start: ids[1],
            end: ids[2],
            kind: ConnectionKind::Diminuendo
        }))
        .is_err()
    );
    assert_eq!(e.score(), &marked);
    e.select(ids[1]);
    e.execute(Command::RemoveConnections(ConnectionKind::Crescendo))
        .unwrap();
    assert_eq!(e.score(), &before);
    e.undo();
    assert_eq!(e.score(), &marked);
    e.select(ids[2]);
    e.execute(Command::Delete).unwrap();
    assert!(e.score().connections().is_empty());
    e.undo();
    assert_eq!(e.score(), &marked);
}

#[test]
fn rhythmic_edits_preserve_chords_ids_links_and_atomic_history_and_capacity() {
    let mut e = Editor::default();
    let mut ids = Vec::new();
    for _ in 0..3 {
        e.execute(Command::Insert(e.note(Step::C))).unwrap();
        ids.push(e.selection().unwrap());
    }
    let c = Connection {
        start: ids[0],
        end: ids[1],
        kind: ConnectionKind::Tie,
    };
    e.execute(Command::AddConnection(c)).unwrap();
    e.select(ids[0]);
    e.extend_selection(ids[2]);
    let original = e.score().clone();
    e.execute(Command::SetTriplet(true)).unwrap();
    assert!(
        ids.iter()
            .all(|id| e.score().event(*id).unwrap().duration.is_triplet())
    );
    assert_eq!(
        e.score().measures()[0].used_ticks(),
        score_core::Duration::Half.ticks()
    );
    assert!(e.score().connections().contains(&c));
    let triplets = e.score().clone();
    e.undo();
    assert_eq!(e.score(), &original);
    assert_eq!(e.selected_ids(), ids);
    e.redo();
    assert_eq!(e.score(), &triplets);
    e.execute(Command::SetDotted(true)).unwrap();
    assert_eq!(e.score().measures()[0].used_ticks(), 144);
    assert!(e.score().connections().contains(&c));
    e.undo();
    assert_eq!(e.score(), &triplets);
    // On a full bar, a failed range edit cannot leave the first notes modified.
    e.execute(Command::SetTriplet(false)).unwrap();
    e.navigate(true);
    e.execute(Command::Insert(e.note(Step::D))).unwrap();
    e.select_all();
    let full = e.score().clone();
    let selected = e.selected_ids();
    assert!(e.execute(Command::SetDotted(true)).is_err());
    assert_eq!(e.score(), &full);
    assert_eq!(e.selected_ids(), selected);
    // A chord/rest share one duration, never one duration per pitch.
    let mut s = Score::default();
    let mut chord = e.note(Step::E);
    chord.pitches = chord.pitches.with(Pitch::new(Step::G, 4).unwrap()).unwrap();
    let id = s.insert(0, 0, chord).unwrap();
    s.set_duration(id, Duration::Quarter.with_dotted(true))
        .unwrap();
    assert_eq!(s.event(id).unwrap().pitches.len(), 2);
    assert_eq!(s.measures()[0].used_ticks(), 72);
    let rest = s
        .insert(
            0,
            1,
            Event {
                pitches: Pitches::default(),
                duration: Duration::Quarter,
                notation: Notation::default(),
            },
        )
        .unwrap();
    s.set_duration(rest, Duration::Quarter.with_triplet(true))
        .unwrap();
    assert_eq!(s.event_tick(rest), Some(72));
    s.validate().unwrap();
}

#[test]
fn a_selected_mark_is_removed_or_swapped_in_one_undoable_step() {
    let mut editor = Editor::default();
    let event = Event {
        pitches: Pitches::single(Pitch::new(Step::C, 4)),
        duration: Duration::Quarter,
        notation: Notation::default(),
    };
    editor.execute(Command::Insert(event)).unwrap();
    let a = editor.selection().unwrap();
    editor.execute(Command::Insert(event)).unwrap();
    let b = editor.selection().unwrap();
    editor.select(a);
    editor
        .execute(Command::SetArticulation(Some(Articulation::Tenuto)))
        .unwrap();
    let crescendo = Connection {
        start: a,
        end: b,
        kind: ConnectionKind::Crescendo,
    };
    editor.execute(Command::AddConnection(crescendo)).unwrap();
    let decorated = editor.score().clone();

    let diminuendo = Connection {
        kind: ConnectionKind::Diminuendo,
        ..crescendo
    };
    editor
        .execute(Command::ReplaceConnection(crescendo, diminuendo))
        .unwrap();
    assert_eq!(editor.score().connections().len(), 1);
    assert!(editor.score().connections().contains(&diminuendo));
    // An invalid replacement keeps the original mark and history.
    let backwards = Connection {
        start: b,
        end: a,
        ..diminuendo
    };
    assert!(
        editor
            .execute(Command::ReplaceConnection(diminuendo, backwards))
            .is_err()
    );
    assert!(editor.score().connections().contains(&diminuendo));

    editor
        .execute(Command::RemoveMark(score_core::MarkRef::Articulation(a)))
        .unwrap();
    assert_eq!(editor.score().event(a).unwrap().notation.articulation, None);
    assert_eq!(editor.score().event(a).unwrap().pitches, event.pitches);
    assert!(
        editor
            .execute(Command::RemoveMark(score_core::MarkRef::Articulation(a)))
            .is_err()
    );
    editor.undo();
    editor.undo();
    assert_eq!(editor.score(), &decorated);
}
