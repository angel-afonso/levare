use super::*;
use score_core::{
    Articulation, Connection, ConnectionKind, Dynamic, KeyMode, KeySignature, Notation, Pitches,
    Step,
};
fn note(step: Step, duration: Duration) -> Event {
    Event {
        pitches: Pitches::single(Pitch::new(step, 4)),
        duration,
        notation: Notation::default(),
    }
}
#[test]
fn range_selection_expands_in_both_directions_and_delete_undo_restores_it() {
    let mut e = Editor::default();
    let mut ids = Vec::new();
    for step in [Step::C, Step::D, Step::E] {
        e.execute(Command::Insert(note(step, Duration::Quarter)))
            .unwrap();
        ids.push(e.selection().unwrap());
    }
    e.select(ids[2]);
    e.extend_navigation(false);
    e.extend_navigation(false);
    assert_eq!(e.selected_ids(), ids);
    let before = e.score().clone();
    e.execute(Command::Delete).unwrap();
    assert!(e.score().measures()[0].is_empty());
    assert_eq!(e.cursor().index, 0);
    assert!(e.selected_ids().is_empty());
    assert!(e.undo());
    assert_eq!(e.score(), &before);
    assert_eq!(e.selected_ids(), ids);
    e.navigate(true);
    assert_eq!(e.selected_ids(), vec![ids[1]]);
    e.select_all();
    assert_eq!(e.selected_ids(), ids);
    e.select_staff(StaffId::Lower);
    assert!(e.selected_ids().is_empty());
    assert!(!e.select_all());
}
#[test]
fn musical_copy_preserves_chords_marks_internal_links_and_implicit_bar_gaps() {
    let mut s = Score::default();
    let mut chord = note(Step::C, Duration::Quarter);
    chord.pitches = chord
        .pitches
        .with(
            Pitch::new(Step::E, 4)
                .unwrap()
                .with_accidental(Some(Accidental::Flat)),
        )
        .unwrap();
    let a = s.insert(0, 0, chord).unwrap();
    let b = s.insert(0, 1, chord).unwrap();
    let c = s.insert(1, 0, note(Step::G, Duration::Half)).unwrap();
    s.set_dynamic(a, Some(Dynamic::P)).unwrap();
    s.set_articulation(b, Some(Articulation::Tenuto)).unwrap();
    s.add_connection(Connection {
        start: a,
        end: b,
        kind: ConnectionKind::Tie,
    })
    .unwrap();
    s.add_connection(Connection {
        start: a,
        end: c,
        kind: ConnectionKind::Slur,
    })
    .unwrap();
    let mut e = Editor::new(s).unwrap();
    e.select(a);
    e.extend_selection(c);
    let before = e.score().clone();
    let clip = e.copy_selection().unwrap();
    assert_eq!(e.score(), &before);
    assert_eq!(
        clip.events.iter().map(|e| e.offset).collect::<Vec<_>>(),
        [0, 48, 192]
    );
    assert_eq!(clip.links.len(), 2);
    e.select_staff(StaffId::Lower);
    e.set_cursor(Cursor {
        measure: 0,
        index: 0,
        ..e.cursor()
    });
    e.paste(&clip).unwrap();
    let pasted = e.selected_ids();
    assert_ne!(pasted[0].staff, a.staff);
    assert_eq!(e.score().event(pasted[0]).unwrap().pitches, chord.pitches);
    assert_eq!(
        e.score().event(pasted[0]).unwrap().notation.dynamic,
        Some(Dynamic::P)
    );
    assert_eq!(
        e.score()
            .connections()
            .iter()
            .filter(|c| c.start.staff == StaffId::Lower)
            .count(),
        2
    );
    let lower = e
        .score()
        .part(a.part)
        .unwrap()
        .staff(StaffId::Lower)
        .unwrap();
    assert_eq!(lower.measures()[0].used_ticks(), 192);
    assert!(
        lower.measures()[0]
            .iter()
            .any(|(_, event)| event.pitches.is_empty())
    );
    assert!(e.undo());
    assert_eq!(e.score(), &before);
    assert!(e.redo());
    e.score().validate().unwrap();
}
#[test]
fn paste_spanning_bar_splits_long_notes_and_ties_them_as_one_transaction() {
    let mut source = Editor::default();
    let mut event = note(Step::C, Duration::Whole);
    event.notation = Notation {
        harmonic: None,
        division: None,
        arpeggio: None,
        technique: None,
        dynamic: Some(Dynamic::Ff),
        articulation: Some(Articulation::Staccato),
        fermata: false,
    };
    source.execute(Command::Insert(event)).unwrap();
    let clip = source.copy_selection().unwrap();
    let mut e = Editor::default();
    e.execute(Command::Insert(note(Step::D, Duration::Quarter)))
        .unwrap();
    let before = e.score().clone();
    e.paste(&clip).unwrap();
    assert_eq!(e.selected_ids().len(), 3);
    assert_eq!(e.score().connections().len(), 2);
    let ids = e.selected_ids();
    assert_eq!(
        e.score().event(ids[0]).unwrap().notation.dynamic,
        Some(Dynamic::Ff)
    );
    assert_eq!(
        e.score().event(ids[2]).unwrap().notation.articulation,
        Some(Articulation::Staccato)
    );
    assert!(
        e.score()
            .event(ids[0])
            .unwrap()
            .notation
            .articulation
            .is_none()
    );
    e.undo();
    assert_eq!(e.score(), &before);
}
#[test]
fn paste_creates_aligned_measures_and_failure_never_leaves_half_a_passage() {
    let mut source = Editor::default();
    for _ in 0..2 {
        source
            .execute(Command::Insert(note(Step::C, Duration::Whole)))
            .unwrap();
    }
    source.select_all();
    let clip = source.copy_selection().unwrap();
    let mut e = Editor::default();
    e.set_cursor(Cursor {
        measure: 3,
        ..e.cursor()
    });
    let before = e.score().clone();
    e.paste(&clip).unwrap();
    assert_eq!(e.score().measures().len(), 5);
    assert!(
        e.score().parts()[0]
            .staves()
            .iter()
            .all(|staff| staff.measures().len() == 5)
    );
    e.undo();
    assert_eq!(e.score(), &before);
    e.redo();
    assert_eq!(e.score().measures().len(), 5);
    let mut s = Score::default();
    s.insert(0, 0, note(Step::D, Duration::Quarter)).unwrap();
    s.insert(1, 0, note(Step::E, Duration::Whole)).unwrap();
    let mut e = Editor::new(s).unwrap();
    let id = ElementId {
        part: e.cursor().part,
        staff: StaffId::Upper,
        measure: 0,
        event: e.score().measures()[0].id_at(0).unwrap(),
    };
    e.select(id);
    let before = e.score().clone();
    let undo = e.undo.len();
    let selection = e.selected_ids();
    assert!(e.paste(&clip).is_err());
    assert_eq!(e.score(), &before);
    assert_eq!(e.undo.len(), undo);
    assert_eq!(e.selected_ids(), selection);
}
#[test]
fn cross_key_paste_keeps_sounding_pitch_and_inherited_dynamic() {
    let mut s = Score::default();
    s.set_key_signature(KeySignature::new(1, KeyMode::Major).unwrap());
    let a = s.insert(0, 0, note(Step::G, Duration::Quarter)).unwrap();
    s.set_dynamic(a, Some(Dynamic::Pp)).unwrap();
    let b = s.insert(0, 1, note(Step::F, Duration::Quarter)).unwrap();
    let mut e = Editor::new(s).unwrap();
    e.select(b);
    let clip = e.copy_selection().unwrap();
    assert_eq!(clip.initial_dynamic, Dynamic::Pp);
    let mut dest = Editor::default();
    dest.paste(&clip).unwrap();
    let event = dest.score().event(dest.selection().unwrap()).unwrap();
    assert_eq!(
        event.pitches.first().unwrap().accidental(),
        Some(Accidental::Sharp)
    );
    assert_eq!(event.notation.dynamic, Some(Dynamic::Pp));
    let mut s = Score::default();
    s.set_key_signature(KeySignature::new(1, KeyMode::Major).unwrap());
    let mut dest = Editor::new(s).unwrap();
    dest.paste(&clip).unwrap();
    assert!(dest.selected_pitch().unwrap().accidental().is_none());
}
#[test]
fn invalid_clipboard_cannot_mutate_score_or_history() {
    let mut e = Editor::default();
    e.execute(Command::Insert(note(Step::C, Duration::Quarter)))
        .unwrap();
    let mut clip = e.copy_selection().unwrap();
    let before = e.score().clone();
    let undo = e.undo.len();
    clip.events[0].offset = u64::MAX;
    assert!(e.paste(&clip).is_err());
    assert_eq!(e.score(), &before);
    assert_eq!(e.undo.len(), undo);
    clip.events[0].offset = 0;
    clip.links.push(clipboard::ClipLink {
        start: 0,
        end: 3,
        kind: ConnectionKind::Tie,
    });
    assert!(e.paste(&clip).is_err());
    assert_eq!(e.score(), &before);
}

#[test]
fn clipboard_carries_inherited_technique_splits_once_and_rejects_incompatible_destinations_atomically()
 {
    use score_core::{Instrument, PlayingTechnique as T};
    let mut s = Score::default();
    let part = s.add_part(Instrument::Violin).unwrap();
    let a = s
        .insert_in(part, 0, 0, note(Step::C, Duration::Quarter))
        .unwrap();
    let b = s
        .insert_in(part, 1, 0, note(Step::D, Duration::Whole))
        .unwrap();
    s.set_technique(a, Some(T::Pizzicato)).unwrap();
    let mut e = Editor::new(s).unwrap();
    e.select(b);
    let clip = e.copy_selection().unwrap();
    assert_eq!(clip.initial_technique, T::Pizzicato);
    assert_eq!(clip.version, 6);
    assert_eq!(clip.events[0].event.notation.technique, None);
    let mut target = Score::default();
    let violin = target.add_part(Instrument::Violin).unwrap();
    let before = target
        .insert_in(violin, 0, 0, note(Step::E, Duration::Quarter))
        .unwrap();
    let mut dst = Editor::new(target).unwrap();
    dst.select(before);
    let original = dst.score().clone();
    dst.paste(&clip).unwrap();
    let ids = dst.selected_ids();
    assert_eq!(ids.len(), 3);
    assert_eq!(
        dst.score().event(ids[0]).unwrap().notation.technique,
        Some(T::Pizzicato)
    );
    assert_eq!(dst.score().event(ids[1]).unwrap().notation.technique, None);
    assert_eq!(dst.score().technique_at(ids[1]), Some(T::Pizzicato));
    dst.undo();
    assert_eq!(dst.score(), &original);
    dst.select_part(original.parts()[0].id());
    let original = dst.score().clone();
    assert!(dst.paste(&clip).is_err());
    assert_eq!(dst.score(), &original);
    // Old clipboard payloads still decode without a new field.
    let mut old = serde_json::to_value(&clip).unwrap();
    old["version"] = 1.into();
    old.as_object_mut().unwrap().remove("initial_technique");
    let old: clipboard::MusicalClipboard = serde_json::from_value(old).unwrap();
    old.validate().unwrap();
    assert_eq!(old.initial_technique, T::Arco);
}

#[test]
fn hairpin_paste_remaps_end_to_first_split_note_and_carries_completed_levels() {
    let mut s = Score::default();
    let a = s.insert(0, 0, note(Step::C, Duration::Quarter)).unwrap();
    let b = s.insert(0, 1, note(Step::D, Duration::Half)).unwrap();
    s.set_dynamic(a, Some(Dynamic::P)).unwrap();
    s.add_connection(Connection {
        start: a,
        end: b,
        kind: ConnectionKind::Crescendo,
    })
    .unwrap();
    let c = s.insert(1, 0, note(Step::E, Duration::Quarter)).unwrap();
    let mut e = Editor::new(s).unwrap();
    e.select(a);
    e.extend_selection(b);
    let clip = e.copy_selection().unwrap();
    assert_eq!(clip.links.len(), 1);
    assert_eq!(clip.version, 6);
    let mut dst = Editor::default();
    dst.execute(Command::Insert(note(Step::G, Duration::Half)))
        .unwrap();
    dst.paste(&clip).unwrap();
    let copied = dst
        .score()
        .connections()
        .iter()
        .find(|c| c.kind.is_hairpin())
        .unwrap();
    let tick = dst.score().event_tick(copied.end).unwrap();
    assert_eq!(tick, 144);
    assert_eq!(copied.end.measure, 0); // The half note is split here and into bar 2.
    dst.score().validate().unwrap();
    e.select(c);
    let after = e.copy_selection().unwrap();
    assert_eq!(after.initial_dynamic, Dynamic::Mf); // p + two steps, retained beyond the regulator.
    assert!(after.links.is_empty());
}

#[test]
fn musical_clipboard_keeps_modified_figures_and_upgrades_legacy_tick_units() {
    let mut e = Editor::default();
    e.execute(Command::Insert(note(
        Step::C,
        Duration::Quarter.with_dotted(true),
    )))
    .unwrap();
    let a = e.selection().unwrap();
    for _ in 0..3 {
        e.execute(Command::Insert(note(
            Step::E,
            Duration::Eighth.with_triplet(true),
        )))
        .unwrap();
    }
    let b = e.selection().unwrap();
    e.select(a);
    e.extend_selection(b);
    let clip = e.copy_selection().unwrap();
    assert_eq!(clip.version, 6);
    let mut dst = Editor::default();
    dst.paste(&clip).unwrap();
    let events: Vec<_> = dst.score().measures()[0]
        .iter()
        .map(|(_, e)| e.duration)
        .collect();
    assert_eq!(
        events,
        [
            Duration::Quarter.with_dotted(true),
            Duration::Eighth.with_triplet(true),
            Duration::Eighth.with_triplet(true),
            Duration::Eighth.with_triplet(true)
        ]
    );
    // Ordinary old payload offsets use 64 ticks/whole; migration is explicit.
    let mut old = clip.clone();
    old.version = 3;
    old.events.truncate(2);
    old.events[0].event.duration = Duration::Quarter;
    old.events[1].event.duration = Duration::Quarter;
    old.events[1].offset = 16;
    let upgraded = old.clone().upgraded().unwrap();
    assert_eq!(upgraded.events[1].offset, 48);
    assert_eq!(upgraded.version, 6);
    let mut dst = Editor::default();
    dst.paste(&old).unwrap();
    assert_eq!(dst.score().measures()[0].used_ticks(), 96);
    // A duration crossing a bar is split with ties without losing dotted length.
    let mut src = Editor::default();
    src.execute(Command::Insert(note(
        Step::C,
        Duration::Quarter.with_dotted(true),
    )))
    .unwrap();
    let clip = src.copy_selection().unwrap();
    let mut dst = Editor::default();
    for _ in 0..3 {
        dst.execute(Command::Insert(note(Step::D, Duration::Quarter)))
            .unwrap();
    }
    dst.paste(&clip).unwrap();
    let ids = dst.selected_ids();
    assert_eq!(ids.len(), 2);
    assert_eq!(
        ids.iter()
            .map(|id| u32::from(dst.score().event(*id).unwrap().duration.ticks()))
            .sum::<u32>(),
        72
    );
    assert_eq!(
        dst.score()
            .connections()
            .iter()
            .filter(|c| c.kind == ConnectionKind::Tie)
            .count(),
        1
    );
}
#[test]
fn unrepresentable_paste_boundaries_fail_without_partial_music_or_panics() {
    let mut src = Editor::default();
    src.execute(Command::Insert(note(Step::C, Duration::Quarter)))
        .unwrap();
    let clip = src.copy_selection().unwrap();
    let mut dst = Editor::default();
    for _ in 0..23 {
        dst.execute(Command::Insert(note(
            Step::E,
            Duration::Sixteenth.with_triplet(true),
        )))
        .unwrap();
    }
    // 184 ticks leaves eight: a straight quarter cannot be divided there using
    // the supported straight note values. No new bars or events may escape.
    let before = dst.score().clone();
    let selected = dst.selected_ids();
    assert!(dst.paste(&clip).is_err());
    assert_eq!(dst.score(), &before);
    assert_eq!(dst.selected_ids(), selected);
}

#[test]
fn arpeggio_edits_undo_and_split_paste_keep_only_the_original_attack() {
    use score_core::{Arpeggio, MarkKind, MarkRef};
    let mut source = Editor::default();
    let mut chord = note(Step::C, Duration::Whole);
    chord.pitches = chord.pitches.with(Pitch::new(Step::G, 4).unwrap()).unwrap();
    source.execute(Command::Insert(chord)).unwrap();
    let id = source.selection().unwrap();
    source
        .execute(Command::ToggleMark(MarkKind::Arpeggio(Arpeggio::Down)))
        .unwrap();
    let clip = source.copy_selection().unwrap();
    source.execute(Command::RemoveHighestPitch).unwrap();
    assert!(!source.score().has_mark(MarkRef::Arpeggio(id)));
    source.undo();
    assert!(source.score().has_mark(MarkRef::Arpeggio(id)));
    source
        .execute(Command::RemoveMark(MarkRef::Arpeggio(id)))
        .unwrap();
    assert_eq!(source.score().event(id).unwrap().pitches.len(), 2);
    source.undo();
    source
        .execute(Command::ToggleMark(MarkKind::Arpeggio(Arpeggio::Down)))
        .unwrap();
    assert!(!source.score().has_mark(MarkRef::Arpeggio(id)));
    source.undo();
    let mut target = Editor::default();
    target
        .execute(Command::Insert(note(Step::D, Duration::Quarter)))
        .unwrap();
    let before = target.score().clone();
    target.paste(&clip).unwrap();
    let pasted = target.selected_ids();
    assert!(pasted.len() > 1);
    for (index, id) in pasted.iter().enumerate() {
        assert_eq!(
            target.score().event(*id).unwrap().notation.arpeggio,
            (index == 0).then_some(Arpeggio::Down)
        );
    }
    target.score().validate().unwrap();
    target.undo();
    assert_eq!(target.score(), &before);
    target.redo();
    assert_eq!(
        target.score().event(pasted[0]).unwrap().notation.arpeggio,
        Some(Arpeggio::Down)
    );
}

#[test]
fn shared_arpeggio_edits_are_atomic_undoable_and_single_staff_copy_has_no_dangling_links() {
    use score_core::{Arpeggio, MarkRef};
    let mut score = Score::default();
    let piano = score.parts()[0].id();
    let mut chord = note(Step::C, Duration::Quarter);
    chord.pitches = chord.pitches.with(Pitch::new(Step::G, 4).unwrap()).unwrap();
    let upper = score.insert(0, 0, chord).unwrap();
    let lower = score.insert_on(piano, StaffId::Lower, 0, 0, chord).unwrap();
    let mut editor = Editor::new(score).unwrap();
    editor.select(lower);
    editor.execute(Command::SetArpeggioBothHands(true)).unwrap();
    assert_eq!(editor.score().piano_arpeggios().len(), 1);
    editor.undo();
    assert!(editor.score().piano_arpeggios().is_empty());
    editor.redo();
    let clip = editor.copy_selection().unwrap();
    assert_eq!(
        clip.events[0].event.notation.arpeggio,
        Some(Arpeggio::Normal)
    );
    let mut target = Editor::default();
    target.paste(&clip).unwrap();
    target.score().validate().unwrap();
    assert!(target.score().piano_arpeggios().is_empty());
    editor
        .execute(Command::RemoveMark(MarkRef::Arpeggio(upper)))
        .unwrap();
    assert!(editor.score().piano_arpeggios().is_empty());
    assert_eq!(editor.score().event(lower).unwrap().pitches.len(), 2);
    editor.undo();
    editor.execute(Command::Delete).unwrap();
    assert!(editor.score().piano_arpeggios().is_empty());
    editor.undo();
    assert_eq!(editor.score().piano_arpeggios().len(), 1);
}

#[test]
fn copying_between_concert_and_transposing_instruments_keeps_sounding_pitch() {
    for source in [Instrument::Flute, Instrument::ClarinetBb, Instrument::HornF] {
        let mut score = Score::new("Origen".into(), &[source], 1).unwrap();
        let part = score.parts()[0].id();
        let id = score
            .insert_on(part, StaffId::Upper, 0, 0, note(Step::C, Duration::Quarter))
            .unwrap();
        let sounding = score.sounding_pitches(id).unwrap();
        let mut editor = Editor::new(score).unwrap();
        editor.select(id);
        let clip = editor.copy_selection().unwrap();
        for target in [Instrument::Flute, Instrument::ClarinetBb, Instrument::HornF] {
            let mut dest =
                Editor::new(Score::new("Destino".into(), &[target], 1).unwrap()).unwrap();
            dest.paste(&clip).unwrap();
            let absolute = |ps: Pitches| {
                ps.iter()
                    .map(|p| {
                        i16::from(p.octave()) * 12
                            + [0, 2, 4, 5, 7, 9, 11][p.step.index() as usize]
                            + i16::from(p.alteration())
                    })
                    .collect::<Vec<_>>()
            };
            assert_eq!(
                absolute(
                    dest.score()
                        .sounding_pitches(dest.selected_ids()[0])
                        .unwrap()
                ),
                absolute(sounding),
                "{source:?} → {target:?}"
            );
            assert!(dest.undo());
            assert!(dest.score().measures()[0].is_empty());
        }
    }
}
