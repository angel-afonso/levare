use super::*;
use score_core::{
    ChordSymbol, Duration, ElementId, Event, Join, Notation, Pitch, Pitches, StaffId, Step,
    Syllable,
};

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
    assert_eq!((found[2].0, found[2].2.as_deref()), (72, Some("D7(♭9)")));
    assert!(result.warnings.iter().any(|w| w.contains("silencios")));
}
