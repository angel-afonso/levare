use super::*;
use score_core::{Duration, Event, Notation, Pitch, Pitches, Step, TextKind};

fn note(step: Step) -> Event {
    Event {
        pitches: Pitches::single(Pitch::new(step, 5)),
        duration: Duration::Quarter,
        notation: Notation::default(),
    }
}

fn texts(score: &Score) -> Vec<(u64, TextKind, String)> {
    let part = &score.parts()[0];
    let mut found = Vec::new();
    for (bar, measure) in part.measures().iter().enumerate() {
        for (_, onset, event, _) in measure.timed_iter() {
            let id = score_core::ElementId {
                part: part.id(),
                staff: score_core::StaffId::Upper,
                measure: bar,
                event,
            };
            for kind in TextKind::ALL {
                if let Some(words) = score.text(id, kind) {
                    found.push((
                        score.measure_start_tick(bar) + u64::from(onset),
                        kind,
                        words.to_string(),
                    ));
                }
            }
        }
    }
    found
}

fn written() -> Score {
    let mut score = Score::default();
    let a = score.insert(0, 0, note(Step::C)).unwrap();
    let rest = score
        .insert(
            0,
            1,
            Event {
                pitches: Pitches::default(),
                ..note(Step::C)
            },
        )
        .unwrap();
    let b = score.insert(0, 2, note(Step::E)).unwrap();
    score
        .set_text(a, TextKind::Expression, Some("dolce"))
        .unwrap();
    score
        .set_text(a, TextKind::Technique, Some("con sord."))
        .unwrap();
    score
        .set_text(rest, TextKind::Technique, Some("senza sord."))
        .unwrap();
    score
        .set_text(b, TextKind::Expression, Some("cresc. «poco» & más"))
        .unwrap();
    score
}

#[test]
fn texts_round_trip_in_the_native_format() {
    let score = written();
    let bytes = encode(&score).unwrap();
    assert!(String::from_utf8_lossy(&bytes).contains("\"expression_text\": \"dolce\""));
    assert_eq!(decode(&bytes).unwrap(), score);
    let long = String::from_utf8(bytes).unwrap().replace(
        "\"dolce\"",
        &format!("\"{}\"", "x".repeat(score_core::MAX_TEXT_BYTES + 1)),
    );
    assert!(decode(long.as_bytes()).is_err());
}

#[test]
fn texts_round_trip_through_musicxml_with_their_placement() {
    let score = written();
    let xml = String::from_utf8(musicxml::encode(&score).unwrap()).unwrap();
    assert!(xml.contains(
        "<direction placement=\"below\"><direction-type><words font-style=\"italic\">dolce</words>"
    ));
    assert!(
        xml.contains("<direction placement=\"above\"><direction-type><words>con sord.</words>")
    );
    assert!(xml.contains("cresc. «poco» &amp; más"));
    let result = musicxml::decode(xml.as_bytes()).unwrap();
    assert!(
        result.warnings.iter().all(|w| !w.contains("texto")),
        "{:?}",
        result.warnings
    );
    assert_eq!(texts(&result.score), texts(&score));
}

#[test]
fn imported_words_take_their_kind_from_placement_style_and_meaning() {
    let direction = |attributes: &str, words: &str, tick: u16| {
        format!(
            "<direction{attributes}><direction-type>{words}</direction-type><offset>{tick}</offset></direction>"
        )
    };
    let body = [
        direction(" placement=\"below\"", "<words>espress.</words>", 0),
        direction(" placement=\"above\"", "<words>sul pont.</words>", 1),
        direction("", "<words font-style=\"italic\">cantabile</words>", 2),
        direction(" placement=\"above\"", "<words>cresc.</words>", 3),
        direction("", &format!("<words>{}</words>", "y".repeat(50)), 3),
    ]
    .concat();
    let xml = format!(
        r#"<?xml version="1.0"?><score-partwise version="4.0"><part-list><score-part id="P1"><part-name>Flauta</part-name></score-part></part-list><part id="P1"><measure number="1"><attributes><divisions>1</divisions><time><beats>4</beats><beat-type>4</beat-type></time><clef><sign>G</sign><line>2</line></clef></attributes>{body}{}</measure></part></score-partwise>"#,
        "<note><pitch><step>C</step><octave>5</octave></pitch><duration>1</duration><type>quarter</type></note>".repeat(4)
    );
    let result = musicxml::decode(xml.as_bytes()).unwrap();
    let found = texts(&result.score);
    let kinds: Vec<_> = found.iter().map(|(_, kind, _)| *kind).collect();
    assert_eq!(
        kinds,
        [
            TextKind::Expression,
            TextKind::Technique,
            TextKind::Expression,
            TextKind::Expression,
            TextKind::Technique,
        ]
    );
    assert_eq!(found[4].2.len(), score_core::MAX_TEXT_BYTES);
    assert!(result.warnings.iter().any(|w| w.contains("recortan")));
}
