use super::*;
use score_core::{
    Accidental, Connection, ConnectionKind, Duration, Dynamic, Event, Instrument, Pitch, Pitches,
    Score, StaffId, Step, Voice,
};
use score_edit::{Command, Cursor, Editor};

fn pitch(step: Step, octave: u8) -> Pitch {
    Pitch::new(step, octave).unwrap()
}

#[test]
fn parses_notes_rests_chords() {
    let events = parse_voice("C4:4 R:8 Eb5:8 [C4 E4 G4]:2").unwrap();
    assert_eq!(events.len(), 4);
    assert_eq!(events[0].event.pitches.first(), Some(pitch(Step::C, 4)));
    assert!(events[1].event.pitches.is_empty());
    assert_eq!(events[1].event.duration, Duration::Eighth);
    let flat = events[2].event.pitches.first().unwrap();
    assert_eq!(flat.step, Step::E);
    assert_eq!(flat.octave(), 5);
    assert_eq!(flat.accidental(), Some(Accidental::Flat));
    assert_eq!(events[3].event.pitches.len(), 3);
    assert_eq!(events[3].event.duration, Duration::Half);
}

#[test]
fn parses_dots_triplets_ties() {
    let events = parse_voice("D4:4.~ D4:8 E4:8t").unwrap();
    assert!(events[0].event.duration.is_dotted());
    assert!(events[0].tie_to_next);
    assert!(!events[1].tie_to_next);
    assert!(events[2].event.duration.is_triplet());
    assert!(!events[2].event.duration.is_dotted());
}

#[test]
fn parses_sharps_naturals_and_every_value() {
    let events = parse_voice("F#3:1 Bn4:2 A0:16 G8:32.t").unwrap();
    let accidentals: Vec<_> = events
        .iter()
        .map(|e| e.event.pitches.first().unwrap().accidental())
        .collect();
    assert_eq!(
        accidentals,
        [
            Some(Accidental::Sharp),
            Some(Accidental::Natural),
            None,
            None
        ]
    );
    assert_eq!(events[0].event.duration, Duration::Whole);
    assert_eq!(events[2].event.duration, Duration::Sixteenth);
    assert_eq!(
        events[3].event.duration,
        Duration::ThirtySecond.with_dotted(true).with_triplet(true)
    );
}

#[test]
fn ignores_mark_blocks() {
    let events = parse_voice("C5:4{Trino, f} D5:4").unwrap();
    assert_eq!(events.len(), 2);
    let tied = parse_voice("C5:4~{Ligadura de unión (inicio)} D5:4").unwrap();
    assert!(tied[0].tie_to_next);
}

#[test]
fn empty_text_is_an_empty_voice() {
    assert!(parse_voice("").unwrap().is_empty());
    assert!(parse_voice("  \n ").unwrap().is_empty());
}

#[test]
fn rejects_bad_tokens() {
    for (text, token) in [
        ("H4:4", "H4:4"),
        ("C4:3", "C4:3"),
        ("C9:4", "C9:4"),
        ("C4", "C4"),
    ] {
        let error = parse_voice(text).unwrap_err();
        assert_eq!(error.offset, 0, "{text}");
        assert!(error.message.contains(token), "{text}: {}", error.message);
    }
    let error = parse_voice("C4:4 D4:4 X:4").unwrap_err();
    assert_eq!(error.offset, 10);
    assert!(error.message.contains("X:4"));
    assert!(error.to_string().contains("X:4"));
}

#[test]
fn rejects_malformed_structure() {
    for text in [
        "[C4]:4",                         // one-note chord
        "[C4 E4:4",                       // unclosed chord
        "[C4 C4]:4",                      // same chord position twice
        "[C4 R]:4",                       // rest inside a chord
        "C4:4{f",                         // unclosed marks
        "C4:4x",                          // junk after the event
        "C4:4t.",                         // dot after the triplet sign
        "R:4~",                           // tied rest
        "C4:4,D4:4",                      // wrong separator
        "[C4 E4 G4 B4 D5 F5 A5 C6 E6]:4", // more than eight notes
    ] {
        assert!(parse_voice(text).is_err(), "{text}");
    }
}

/// A 4/4 bar in voice one: dotted C4, a tied chord, an eighth rest and a
/// forte on the first note.
fn written_bar() -> (Editor, [Event; 4]) {
    let score = Score::new("".into(), &[Instrument::Violin], 1).unwrap();
    let mut editor = Editor::new(score).unwrap();
    let events = [
        Event {
            notation: Default::default(),
            pitches: Pitches::single(Some(pitch(Step::C, 4))),
            duration: Duration::Quarter.with_dotted(true),
        },
        Event {
            notation: Default::default(),
            pitches: Pitches::single(None),
            duration: Duration::Eighth,
        },
        Event {
            notation: Default::default(),
            pitches: Pitches::new(&[
                pitch(Step::E, 4).with_accidental(Some(Accidental::Flat)),
                pitch(Step::G, 4),
                pitch(Step::C, 5),
            ])
            .unwrap(),
            duration: Duration::Quarter,
        },
        Event {
            notation: Default::default(),
            pitches: Pitches::new(&[
                pitch(Step::E, 4).with_accidental(Some(Accidental::Flat)),
                pitch(Step::G, 4),
                pitch(Step::C, 5),
            ])
            .unwrap(),
            duration: Duration::Eighth.with_triplet(true),
        },
    ];
    for event in events {
        editor.execute(Command::Write(event)).unwrap();
    }
    (editor, events)
}

fn voice_ids(editor: &Editor) -> Vec<score_core::ElementId> {
    let part = editor.score().parts()[0].id();
    let measure = &editor.score().measures()[0];
    measure
        .voice_iter(Voice::One)
        .map(|(event, _)| score_core::ElementId {
            part,
            staff: StaffId::Upper,
            measure: 0,
            event,
        })
        .collect()
}

#[test]
fn format_round_trips() {
    let (mut editor, events) = written_bar();
    let ids = voice_ids(&editor);
    editor.select(ids[0]);
    editor
        .execute(Command::SetDynamic(Some(Dynamic::F)))
        .unwrap();
    editor
        .execute(Command::AddConnection(Connection {
            start: ids[2],
            end: ids[3],
            kind: ConnectionKind::Tie,
        }))
        .unwrap();
    let part = editor.score().parts()[0].id();
    let text = format_voice(editor.score(), part, StaffId::Upper, 0, Voice::One);
    assert_eq!(text, "C4:4.{f} R:8 [Eb4 G4 C5]:4~ [Eb4 G4 C5]:8t");
    let parsed = parse_voice(&text).unwrap();
    assert_eq!(parsed.len(), events.len());
    for (parsed, original) in parsed.iter().zip(events) {
        assert_eq!(parsed.event.pitches, original.pitches);
        assert_eq!(parsed.event.duration, original.duration);
    }
    let ties: Vec<_> = parsed.iter().map(|e| e.tie_to_next).collect();
    assert_eq!(ties, [false, false, true, false]);
    assert!(text.contains(Dynamic::F.label()));
}

#[test]
fn format_names_span_marks_at_both_ends() {
    let (mut editor, _) = written_bar();
    let ids = voice_ids(&editor);
    editor
        .execute(Command::AddConnection(Connection {
            start: ids[0],
            end: ids[2],
            kind: ConnectionKind::Slur,
        }))
        .unwrap();
    let part = editor.score().parts()[0].id();
    let text = format_voice(editor.score(), part, StaffId::Upper, 0, Voice::One);
    let slur = ConnectionKind::Slur.label();
    assert!(
        text.starts_with(&format!("C4:4.{{{slur} (inicio)}}")),
        "{text}"
    );
    assert!(text.contains(&format!("{{{slur} (fin)}}")), "{text}");
    assert!(!text.contains('~'));
    assert_eq!(parse_voice(&text).unwrap().len(), 4);
}

#[test]
fn format_of_a_missing_bar_or_empty_voice_is_empty() {
    let (editor, _) = written_bar();
    let part = editor.score().parts()[0].id();
    let score = editor.score();
    assert_eq!(format_voice(score, part, StaffId::Upper, 0, Voice::Two), "");
    assert_eq!(format_voice(score, part, StaffId::Upper, 9, Voice::One), "");
    assert_eq!(format_voice(score, part, StaffId::Lower, 0, Voice::One), "");
}

#[test]
fn formats_written_pitch_for_transposing_parts() {
    let score = Score::new("".into(), &[Instrument::ClarinetBb], 1).unwrap();
    let mut editor = Editor::new(score).unwrap();
    let part = editor.score().parts()[0].id();
    editor.set_cursor(Cursor {
        part,
        ..Cursor::default()
    });
    editor
        .execute(Command::Write(Event {
            notation: Default::default(),
            pitches: Pitches::single(Some(pitch(Step::D, 5))),
            duration: Duration::Quarter,
        }))
        .unwrap();
    let text = format_voice(editor.score(), part, StaffId::Upper, 0, Voice::One);
    assert_eq!(text, "D5:4");
}
