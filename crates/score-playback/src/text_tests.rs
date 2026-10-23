use super::*;
use score_core::{
    Connection, ConnectionKind, Duration, Dynamic, Event, Instrument, Notation, Pitch, Pitches,
};

fn note(step: Step) -> Event {
    Event {
        pitches: Pitches::single(Pitch::new(step, 5)),
        duration: Duration::Quarter,
        notation: Notation::default(),
    }
}

fn plan(score: &Score) -> PlaybackPlan {
    PlaybackPlan::new(score, Tempo::default(), 48_000).unwrap()
}

fn phrase(instrument: Instrument) -> (Score, Vec<ElementId>) {
    let mut score = Score::new("".into(), &[instrument], 1).unwrap();
    let steps = [Step::C, Step::D, Step::E, Step::F];
    let ids = steps
        .iter()
        .enumerate()
        .map(|(i, step)| score.insert(0, i, note(*step)).unwrap())
        .collect();
    (score, ids)
}

fn levels(plan: &PlaybackPlan) -> Vec<u8> {
    plan.messages
        .iter()
        .filter_map(|m| match m.message {
            Message::NoteOn(note) => Some(note.level),
            _ => None,
        })
        .collect()
}

fn expression(plan: &PlaybackPlan) -> Vec<(u64, u8)> {
    plan.messages
        .iter()
        .filter_map(|m| match m.message {
            Message::Expression(value) => Some((m.frame, value)),
            _ => None,
        })
        .collect()
}

#[test]
fn a_written_crescendo_plays_like_the_hairpin_to_the_next_dynamic() {
    let (mut words, ids) = phrase(Instrument::Flute);
    words.set_dynamic(ids[0], Some(Dynamic::P)).unwrap();
    words.set_dynamic(ids[3], Some(Dynamic::F)).unwrap();
    let mut hairpin = words.clone();
    words
        .set_text(ids[0], TextKind::Expression, Some("cresc."))
        .unwrap();
    hairpin
        .add_connection(Connection {
            start: ids[0],
            end: ids[3],
            kind: ConnectionKind::Crescendo,
        })
        .unwrap();
    let written = expression(&plan(&words));
    assert!(written.len() > 2);
    assert_eq!(written, expression(&plan(&hairpin)));
    assert_eq!(levels(&plan(&words)), levels(&plan(&hairpin)));
}

#[test]
fn a_mute_softens_the_notes_until_it_is_taken_off() {
    let (mut score, ids) = phrase(Instrument::Violin);
    let open = levels(&plan(&score));
    score
        .set_text(ids[1], TextKind::Technique, Some("con sord."))
        .unwrap();
    score
        .set_text(ids[3], TextKind::Technique, Some("senza sord."))
        .unwrap();
    let muted = levels(&plan(&score));
    assert_eq!(muted[0], open[0]);
    assert!(muted[1] < open[1] && muted[2] < open[2]);
    assert_eq!(muted[3], open[3]);
}

#[test]
fn legato_in_words_binds_the_notes_like_a_slur() {
    let (mut score, ids) = phrase(Instrument::Flute);
    let release = |plan: &PlaybackPlan| {
        let off = plan
            .messages
            .iter()
            .find(|m| matches!(m.message, Message::NoteOff(_)))
            .unwrap()
            .frame;
        let second = plan
            .messages
            .iter()
            .filter(|m| matches!(m.message, Message::NoteOn(_)))
            .nth(1)
            .unwrap()
            .frame;
        (off, second)
    };
    let (off, next) = release(&plan(&score));
    assert!(off < next, "detached by default");
    score
        .set_text(ids[0], TextKind::Expression, Some("legato"))
        .unwrap();
    let bound = plan(&score);
    let (off, next) = release(&bound);
    assert!(off >= next, "legato reaches the next note");
    score
        .set_text(ids[2], TextKind::Technique, Some("ord."))
        .unwrap();
    let ordinary = plan(&score);
    let attacks: Vec<_> = ordinary
        .messages
        .iter()
        .filter(|m| matches!(m.message, Message::NoteOn(_)))
        .map(|m| m.frame)
        .collect();
    let last_off = ordinary
        .messages
        .iter()
        .filter(|m| matches!(m.message, Message::NoteOff(_)))
        .map(|m| m.frame)
        .filter(|frame| *frame > attacks[2])
        .min()
        .unwrap();
    assert!(last_off < attacks[3], "«ord.» ends the legato");
}
