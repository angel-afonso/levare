use super::*;
use score_core::{
    ChordSymbol, Duration, ElementId, Event, Join, Notation, Pitch, Pitches, StaffId, Step,
    Syllable,
};

/// A quarter note.
const Q: u32 = score_core::TICKS_PER_QUARTER;

fn note(step: Step) -> Event {
    Event {
        pitches: Pitches::single(Pitch::new(step, 5)),
        duration: Duration::Quarter,
        notation: Notation::default(),
    }
}

/// A sung onset: its tick, its syllables by verse and its chord.
type Sung = (u64, Vec<(usize, String, Join)>, Option<String>);

/// Lyrics and chords of the first staff, in order.
fn sung(score: &Score) -> Vec<Sung> {
    let part = &score.parts()[0];
    let mut found = Vec::new();
    for (bar, measure) in part.measures().iter().enumerate() {
        for (_, onset, event, written) in measure.timed_iter() {
            let _ = ElementId {
                part: part.id(),
                staff: StaffId::Upper,
                measure: bar,
                event,
            };
            let lyrics: Vec<_> = written
                .notation
                .lyrics
                .verses()
                .map(|(verse, s)| (verse, s.text().to_owned(), s.join))
                .collect();
            let chord = written.notation.chord.map(|chord| chord.label());
            if !lyrics.is_empty() || chord.is_some() {
                found.push((
                    score.measure_start_tick(bar) + u64::from(onset),
                    lyrics,
                    chord,
                ));
            }
        }
    }
    found
}

fn song() -> Score {
    let mut score = Score::default();
    let steps = [Step::C, Step::D, Step::E, Step::F];
    let ids: Vec<_> = steps
        .iter()
        .enumerate()
        .map(|(i, step)| score.insert(0, i, note(*step)).unwrap())
        .collect();
    let sing = |text, join| Some(Syllable::new(text, join).unwrap());
    score
        .set_syllable(ids[0], 0, sing("Can", Join::Hyphen))
        .unwrap();
    score
        .set_syllable(ids[1], 0, sing("ta", Join::Hyphen))
        .unwrap();
    score
        .set_syllable(ids[2], 0, sing("re", Join::Melisma))
        .unwrap();
    score
        .set_syllable(ids[0], 1, sing("Sol", Join::Word))
        .unwrap();
    score
        .set_syllable(ids[1], 1, sing("y", Join::Word))
        .unwrap();
    for (id, chord) in [(ids[0], "C"), (ids[2], "Bb7sus4/D"), (ids[3], "Fadd9")] {
        score
            .set_chord(id, Some(ChordSymbol::parse(chord).unwrap()))
            .unwrap();
    }
    score
}

#[test]
fn lyrics_and_chords_round_trip_in_the_native_format() {
    let score = song();
    assert_eq!(decode(&encode(&score).unwrap()).unwrap(), score);
}

#[test]
fn lyrics_and_chords_round_trip_through_musicxml() {
    let score = song();
    let xml = String::from_utf8(musicxml::encode(&score).unwrap()).unwrap();
    for tag in [
        "<lyric number=\"1\"><syllabic>begin</syllabic><text>Can</text></lyric>",
        "<lyric number=\"1\"><syllabic>middle</syllabic><text>ta</text></lyric>",
        "<syllabic>end</syllabic><text>re</text><extend/>",
        "<lyric number=\"2\"><syllabic>single</syllabic><text>Sol</text>",
        "<kind text=\"7sus4\">suspended-fourth</kind>",
        "<bass><bass-step>D</bass-step>",
    ] {
        assert!(xml.contains(tag), "{tag}");
    }
    let result = musicxml::decode(xml.as_bytes()).unwrap();
    assert!(
        result
            .warnings
            .iter()
            .all(|w| !w.contains("letra") && !w.contains("cifrado") && !w.contains("sílaba")),
        "{:?}",
        result.warnings
    );
    assert_eq!(sung(&result.score), sung(&score));
}

#[test]
fn other_programs_lyrics_and_harmonies_import_with_their_meaning() {
    let xml = r#"<?xml version="1.0"?><score-partwise version="4.0"><part-list><score-part id="P1"><part-name>Soprano</part-name></score-part></part-list><part id="P1"><measure number="1"><attributes><divisions>2</divisions><time><beats>2</beats><beat-type>4</beat-type></time><clef><sign>G</sign><line>2</line></clef></attributes>
<harmony><root><root-step>A</root-step></root><kind text="m7">minor-seventh</kind></harmony>
<note><pitch><step>A</step><octave>4</octave></pitch><duration>2</duration><type>quarter</type><lyric number="1"><syllabic>begin</syllabic><text>Ale</text></lyric><lyric number="part1verse2"><syllabic>single</syllabic><text>mar</text></lyric></note>
<harmony><root><root-step>D</root-step></root><kind>dominant</kind><degree><degree-value>9</degree-value><degree-alter>-1</degree-alter><degree-type>add</degree-type></degree><offset>1</offset></harmony>
<note><pitch><step>B</step><octave>4</octave></pitch><duration>1</duration><type>eighth</type><lyric><syllabic>end</syllabic><text>lu</text><elision/><text>ya</text></lyric></note>
<note><rest/><duration>1</duration><type>eighth</type><lyric><text>no</text></lyric></note>
</measure></part></score-partwise>"#;
    let result = musicxml::decode(xml.as_bytes()).unwrap();
    assert_eq!(
        result.score.parts()[0].instrument(),
        score_core::Instrument::Soprano
    );
    let found = sung(&result.score);
    assert_eq!(found[0].2.as_deref(), Some("Am7"));
    assert_eq!(
        found[0].1,
        [
            (0, "Ale".to_owned(), Join::Hyphen),
            (1, "mar".to_owned(), Join::Word)
        ]
    );
    assert_eq!(found[1].1, [(0, "lu ya".to_owned(), Join::Word)]);
    // The harmony's offset puts it on the rest after the eighth.
    assert_eq!(
        (found[2].0, found[2].2.as_deref()),
        (u64::from(3 * Q / 2), Some("D7(♭9)"))
    );
    assert!(result.warnings.iter().any(|w| w.contains("silencios")));
}

/// A soprano part, as other programs write it.
fn foreign(measures: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><score-partwise version="3.1"><part-list><score-part id="P1"><part-name>Soprano</part-name></score-part></part-list><part id="P1">{measures}</part></score-partwise>"#
    )
}
const FOREIGN_ATTRIBUTES: &str = "<attributes><divisions>1</divisions><key><fifths>0</fifths></key><time><beats>4</beats><beat-type>4</beat-type></time><clef><sign>G</sign><line>2</line></clef></attributes>";
const QUARTER: &str = "<note><pitch><step>C</step><octave>5</octave></pitch><duration>1</duration><voice>1</voice><type>quarter</type></note>";

fn labels(chords: &[&str]) -> Vec<Option<String>> {
    chords
        .iter()
        .map(|chord| Some(ChordSymbol::parse(chord).unwrap().label()))
        .collect()
}

#[test]
fn chord_alterations_and_added_notes_import_from_other_programs() {
    let harmony = |root: &str, kind: &str, rest: &str| {
        format!(
            "<harmony print-frame=\"no\"><root><root-step>{root}</root-step></root>{kind}{rest}</harmony>{QUARTER}"
        )
    };
    let degree = |value: u8, alter: i8, kind: &str| {
        format!(
            "<degree><degree-value>{value}</degree-value><degree-alter>{alter}</degree-alter><degree-type>{kind}</degree-type></degree>"
        )
    };
    let first = [
        // MuseScore: the kind's text and the added flat nine apart.
        harmony("G", "<kind text=\"7\">dominant</kind>", &degree(9, -1, "add")),
        // Sibelius: everything printed in the text, and the degree too.
        harmony(
            "E",
            "<kind use-symbols=\"no\" text=\"7(#9)\">dominant</kind>",
            &degree(9, 1, "add"),
        ),
        // Dorico: a printed suffix and a bass.
        harmony(
            "C",
            "<kind text=\"maj7\">major-seventh</kind>",
            "<bass><bass-step>B</bass-step></bass>",
        ),
        // Finale: no printed text, an added ninth.
        harmony("F", "<kind>major</kind>", &degree(9, 0, "add")),
    ]
    .concat();
    let second = [
        harmony(
            "A",
            "<kind text=\"m7\">minor-seventh</kind>",
            &degree(5, -1, "alter"),
        ),
        harmony(
            "D",
            "<kind text=\"7sus4\">suspended-fourth</kind>",
            &degree(7, -1, "add"),
        ),
        // A sharp eleventh Levare cannot write: the dominant seventh stays.
        harmony("G", "<kind>dominant</kind>", &degree(11, 1, "add")),
        // Hidden, for playback only.
        format!(
            "<harmony print-object=\"no\"><root><root-step>C</root-step></root><kind>major</kind></harmony>{QUARTER}"
        ),
    ]
    .concat();
    let xml = foreign(&format!(
        "<measure number=\"1\">{FOREIGN_ATTRIBUTES}{first}</measure><measure number=\"2\">{second}</measure>"
    ));
    let result = musicxml::decode(xml.as_bytes()).unwrap();
    let chords: Vec<_> = sung(&result.score).into_iter().map(|s| s.2).collect();
    assert_eq!(
        chords,
        labels(&["G7b9", "E7#9", "Cmaj7/B", "Fadd9", "Am7b5", "D7sus4", "G7"])
    );
    assert!(
        result.warnings.iter().any(|w| w.contains("notas añadidas")),
        "{:?}",
        result.warnings
    );
    assert!(result.warnings.iter().any(|w| w.contains("ocultos")));
    assert!(!result.warnings.iter().any(|w| w.contains("no reconocidos")));
}

#[test]
fn every_chord_quality_with_roots_and_basses_round_trips_through_musicxml() {
    for (index, quality) in score_core::ChordQuality::ALL.into_iter().enumerate() {
        let mut score = Score::default();
        let id = score.insert(0, 0, note(Step::C)).unwrap();
        let root = ["C", "F#", "Bb", "E", "Ab", "D"][index % 6];
        let bass = ["", "/G", "/Eb"][index % 3];
        let chord = ChordSymbol::parse(&format!("{root}{}{bass}", quality.suffix())).unwrap();
        assert_eq!(chord.quality, quality);
        score.set_chord(id, Some(chord)).unwrap();
        let xml = musicxml::encode(&score).unwrap();
        let result = musicxml::decode(&xml).unwrap();
        assert!(
            result.warnings.iter().all(|w| !w.contains("cifrado")),
            "{quality:?}: {:?}",
            result.warnings
        );
        assert_eq!(sung(&result.score), sung(&score), "{quality:?}");
    }
}

#[test]
fn lyrics_on_any_chord_note_and_foreign_spacing_import() {
    let note = |step: &str, extra: &str| {
        format!(
            "<note>{extra}<pitch><step>{step}</step><octave>5</octave></pitch><duration>1</duration><voice>1</voice><type>quarter</type>"
        )
    };
    let measure = [
        "<note><grace slash=\"yes\"/><pitch><step>D</step><octave>5</octave></pitch><voice>1</voice><type>eighth</type><lyric number=\"1\"><text>ah</text></lyric></note>".to_owned(),
        note("C", "") + "</note>",
        // Finale may write the syllable on another note of the chord.
        note("E", "<chord/>")
            + "<lyric number=\"1\" name=\"verse\" default-y=\"-80\"><syllabic>begin</syllabic><text>A\u{a0}la</text></lyric></note>",
        note("D", "")
            + "<lyric number=\"1\"><syllabic>end</syllabic><text>ba</text><extend type=\"start\"/></lyric></note>",
        note("E", "") + "<lyric number=\"1\"><extend type=\"stop\"/></lyric></note>",
        note("F", "")
            + "<lyric number=\"1\"><syllabic>single</syllabic><text>sí</text><extend type=\"stop\"/></lyric></note>",
    ]
    .concat();
    let xml = foreign(&format!(
        "<measure number=\"1\">{FOREIGN_ATTRIBUTES}{measure}</measure>"
    ));
    let result = musicxml::decode(xml.as_bytes()).unwrap();
    let lyrics: Vec<_> = sung(&result.score).into_iter().map(|s| s.1).collect();
    assert_eq!(
        lyrics,
        [
            vec![(0, "A la".to_owned(), Join::Hyphen)],
            vec![(0, "ba".to_owned(), Join::Melisma)],
            vec![(0, "sí".to_owned(), Join::Word)],
        ]
    );
    assert!(
        result.warnings.iter().any(|w| w.contains("adorno")),
        "{:?}",
        result.warnings
    );
    assert!(!result.warnings.iter().any(|w| w.contains("sílaba")));
}
