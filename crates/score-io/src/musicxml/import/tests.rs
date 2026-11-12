use super::*;
use std::io::Write;

/// A quarter note.
const Q: u32 = score_core::TICKS_PER_QUARTER;

fn wrap(instrument: &str, measures: &str) -> String {
    format!(
        "<score-partwise version=\"4.0\"><work><work-title>Prueba externa</work-title></work><part-list><score-part id=\"P1\"><part-name>{instrument}</part-name></score-part></part-list><part id=\"P1\">{measures}</part></score-partwise>"
    )
}
fn import(xml: &str) -> ImportResult {
    decode(xml.as_bytes()).unwrap()
}
fn n(step: &str, duration: u16, voice: u8, staff: u8) -> String {
    format!(
        "<note><pitch><step>{step}</step><octave>4</octave></pitch><duration>{duration}</duration><voice>{voice}</voice><staff>{staff}</staff></note>"
    )
}
fn midi(p: Pitch) -> i16 {
    i16::from(p.octave()) * 12
        + [0, 2, 4, 5, 7, 9, 11][p.step.index() as usize]
        + i16::from(p.alteration())
}
type Attack = (
    usize,
    StaffId,
    Voice,
    u64,
    u64,
    Vec<i16>,
    Option<Harmonic>,
    PlayingTechnique,
);
fn attacks(score: &Score) -> Vec<Attack> {
    let mut result = Vec::new();
    for (pi, part) in score.parts().iter().enumerate() {
        for staff in part.staves() {
            for (bar, measure) in staff.measures().iter().enumerate() {
                for (voice, onset, event, n) in measure.timed_iter() {
                    if n.pitches.is_empty() {
                        continue;
                    }
                    let id = ElementId {
                        part: part.id(),
                        staff: staff.id(),
                        measure: bar,
                        event,
                    };
                    if score
                        .connections()
                        .iter()
                        .any(|c| c.kind == ConnectionKind::Tie && c.end == id)
                    {
                        continue;
                    }
                    let mut end =
                        score.measure_start_tick(bar) + u64::from(onset + n.duration.ticks());
                    let mut at = id;
                    while let Some(c) = score
                        .connections()
                        .iter()
                        .find(|c| c.kind == ConnectionKind::Tie && c.start == at)
                    {
                        at = c.end;
                        end = score.event_tick(at).unwrap()
                            + u64::from(score.event(at).unwrap().duration.ticks());
                    }
                    let (staff, voice) = if part.is_divisi()
                        && staff.id() == StaffId::Lower
                        && voice == Voice::One
                    {
                        (StaffId::Upper, Voice::Two)
                    } else {
                        (staff.id(), voice)
                    };
                    result.push((
                        pi,
                        staff,
                        voice,
                        score.measure_start_tick(bar) + u64::from(onset),
                        end,
                        score
                            .sounding_pitches(id)
                            .unwrap()
                            .iter()
                            .map(midi)
                            .collect(),
                        n.notation.harmonic,
                        score.technique_at(id).unwrap_or_default(),
                    ));
                }
            }
        }
    }
    result.sort_by_key(|(p, s, v, t, _, _, _, _)| {
        (*p, if *s == StaffId::Upper { 0 } else { 1 }, v.number(), *t)
    });
    result
}

#[test]
fn every_native_example_can_be_exported_imported_edited_and_saved_without_changing_attacks() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples");
    for entry in std::fs::read_dir(root).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().and_then(|s| s.to_str()) != Some("partitura") {
            continue;
        }
        let original = crate::load(&path).unwrap();
        let xml = super::super::encode(&original).unwrap();
        let result = decode(&xml).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        assert_eq!(
            attacks(&original),
            attacks(&result.score),
            "{}: {:?}",
            path.display(),
            result.warnings
        );
        assert_eq!(
            original.piano_arpeggios().len(),
            result.score.piano_arpeggios().len(),
            "{}",
            path.display()
        );
        let encoded = crate::encode(&result.score).unwrap();
        assert_eq!(crate::decode(&encoded).unwrap(), result.score);
        result.score.validate().unwrap();
    }
}

#[test]
fn foreign_piano_with_backups_chords_and_two_voices_keeps_exact_positions() {
    let xml = wrap(
        "Piano",
        &format!(
            "<measure number=\"1\"><attributes><divisions>2</divisions><staves>2</staves></attributes>{}<note><chord/><pitch><step>E</step><octave>4</octave></pitch><duration>4</duration><voice>1</voice><staff>1</staff></note>{}<backup><duration>8</duration></backup>{}<backup><duration>8</duration></backup><forward><duration>2</duration></forward>{}</measure>",
            n("C", 4, 1, 1),
            n("G", 4, 1, 1),
            n("C", 8, 2, 1),
            n("C", 6, 5, 2)
        ),
    );
    let result = import(&xml);
    let p = &result.score.parts()[0];
    assert_eq!(p.instrument(), Instrument::Piano);
    assert_eq!(p.measures()[0].voice_len(Voice::One), 2);
    assert_eq!(p.measures()[0].voice_len(Voice::Two), 1);
    assert_eq!(
        p.measures()[0]
            .voice_iter(Voice::One)
            .next()
            .unwrap()
            .1
            .pitches
            .len(),
        2
    );
    let lower = &p.staff(StaffId::Lower).unwrap().measures()[0];
    assert!(
        lower
            .voice_iter(Voice::One)
            .next()
            .unwrap()
            .1
            .pitches
            .is_empty()
    );
    assert_eq!(
        lower
            .timed_iter()
            .find(|(_, _, _, e)| !e.pitches.is_empty())
            .unwrap()
            .1,
        Q
    );
}

#[test]
fn foreign_transposed_clarinet_and_bass_keep_written_pitch_and_sound_in_concert_pitch() {
    for (instrument, transpose, step, octave) in [
        ("Clarinet", "<chromatic>-2</chromatic>", Step::B, 3),
        (
            "Double Bass",
            "<chromatic>0</chromatic><octave-change>-1</octave-change>",
            Step::C,
            3,
        ),
    ] {
        let xml = wrap(
            instrument,
            &format!(
                "<measure number=\"1\"><attributes><divisions>1</divisions><key><fifths>0</fifths></key><transpose>{transpose}</transpose></attributes>{}</measure>",
                n("C", 4, 1, 1)
            ),
        );
        let result = import(&xml);
        let part = &result.score.parts()[0];
        let (event, note) = part.measures()[0].iter().next().unwrap();
        let written = note.pitches.first().unwrap();
        assert_eq!((written.step, written.octave()), (Step::C, 4));
        let pitch = result
            .score
            .sounding_pitches(ElementId {
                part: part.id(),
                staff: StaffId::Upper,
                measure: 0,
                event,
            })
            .unwrap()
            .first()
            .unwrap();
        assert_eq!((pitch.step, pitch.octave()), (step, octave));
        if instrument == "Clarinet" {
            assert_eq!(pitch.in_key(result.score.key_signature()).alteration(), -1);
            assert_eq!(result.score.key_signature().fifths(), -2);
        }
    }
}

#[test]
fn compressed_mxl_and_timewise_files_load_and_source_is_unchanged() {
    let xml = wrap(
        "Violin",
        &format!(
            "<measure number=\"1\"><attributes><divisions>1</divisions></attributes>{}</measure>",
            n("C", 4, 1, 1)
        ),
    );
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    zip.start_file("META-INF/container.xml", options).unwrap();
    zip.write_all(
        b"<container><rootfiles><rootfile full-path=\"score.xml\"/></rootfiles></container>",
    )
    .unwrap();
    zip.start_file("score.xml", options).unwrap();
    zip.write_all(xml.as_bytes()).unwrap();
    let bytes = zip.finish().unwrap().into_inner();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("from-other-program.mxl");
    std::fs::write(&path, &bytes).unwrap();
    let opened = crate::open(&path).unwrap();
    assert!(opened.imported);
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
    let timewise = xml
        .replace("<score-partwise", "<score-timewise")
        .replace("</score-partwise>", "</score-timewise>")
        .replace(
            "<part id=\"P1\"><measure number=\"1\">",
            "<measure number=\"1\"><part id=\"P1\">",
        )
        .replace("</measure></part>", "</part></measure>");
    assert_eq!(decode(timewise.as_bytes()).unwrap().score, opened.score);
}

#[test]
fn unsupported_content_warns_and_unrepresentable_music_is_rejected() {
    let xml = wrap(
        "Violin",
        &format!(
            "<measure number=\"1\"><attributes><divisions>1</divisions></attributes><direction><direction-type><bracket type=\"start\" line-end=\"down\"/></direction-type></direction>{}<barline><repeat direction=\"backward\"/></barline></measure>",
            n("C", 4, 1, 1)
        ),
    );
    let result = import(&xml);
    assert_eq!(result.warnings.len(), 1);
    assert_eq!(result.score.bar_form(0).repeat_end, Some(2));
    let invalid = wrap(
        "Piano",
        &format!(
            "<measure number=\"1\"><attributes><divisions>1</divisions></attributes>{}</measure>",
            ["C", "E", "G", "B", "D"]
                .iter()
                .enumerate()
                .map(|(voice, step)| n(step, 4, voice as u8 + 1, 1))
                .collect::<Vec<_>>()
                .join("<backup><duration>4</duration></backup>")
        ),
    );
    assert!(
        decode(invalid.as_bytes())
            .unwrap_err()
            .to_string()
            .contains("cuatro voces")
    );
    // A 128th is shorter than any value Levare writes.
    assert!(decode(wrap("Violin","<measure><attributes><divisions>32</divisions></attributes><note><pitch><step>C</step><octave>4</octave></pitch><duration>1</duration><type>128th</type></note></measure>").as_bytes()).is_err());
    assert!(decode(b"<score-partwise>broken").is_err());
}

#[test]
fn external_doctype_is_not_fetched_and_expanded_or_large_archives_are_bounded() {
    let xml = format!(
        "<!DOCTYPE score-partwise PUBLIC \"-//Recordare//DTD MusicXML 4.0 Partwise//EN\" \"https://invalid.example/dtd\">{}",
        wrap(
            "Violin",
            &format!(
                "<measure><attributes><divisions>1</divisions></attributes>{}</measure>",
                n("C", 4, 1, 1)
            )
        )
    );
    assert!(decode(xml.as_bytes()).is_ok());
    assert!(decode(&vec![b' '; MAX_MUSICXML_BYTES + 1]).is_err());
    assert!(decode(b"PK not a zip").is_err());
    let xml = "<!DOCTYPE score-partwise [<!ENTITY leak SYSTEM 'file:///etc/passwd'>]><score-partwise><work><work-title>&leak;</work-title></work></score-partwise>";
    assert!(decode(xml.as_bytes()).is_err());
}

#[test]
fn entities_that_multiply_are_refused_before_they_expand() {
    // Each level repeats the one below ten times: five levels already turn
    // a few hundred bytes into a title of 400 000 characters.
    let mut subset = String::from("<!ENTITY e0 \"jaja\">");
    for level in 1..=5 {
        let previous = format!("&e{};", level - 1).repeat(10);
        subset.push_str(&format!("<!ENTITY e{level} \"{previous}\">"));
    }
    let score = wrap(
        "Violin",
        &format!(
            "<measure><attributes><divisions>1</divisions></attributes>{}</measure>",
            n("C", 4, 1, 1)
        ),
    )
    .replace("Prueba externa", "&e5;");
    let xml = format!("<!DOCTYPE score-partwise [{subset}]>{score}");
    assert!(xml.len() < 2_000);
    let failure = decode(xml.as_bytes()).unwrap_err().to_string();
    assert!(failure.contains("entidades"), "{failure}");
}

#[test]
fn pickups_and_directions_inside_sustained_notes_keep_their_lengths_and_onsets() {
    let xml = wrap(
        "Violin",
        &format!(
            "<measure number=\"0\" implicit=\"yes\"><attributes><divisions>1</divisions><time><beats>3</beats><beat-type>4</beat-type></time></attributes>{}</measure><measure number=\"1\"><direction><direction-type><dynamics><f/></dynamics></direction-type><offset>2</offset></direction>{}</measure>",
            n("D", 1, 1, 1),
            n("C", 3, 1, 1)
        ),
    );
    let result = import(&xml);
    assert_eq!(result.score.time_at(0).capacity(), Q);
    assert_eq!(result.score.time_at(1).capacity(), 3 * Q);
    let bar = &result.score.parts()[0].measures()[1];
    let (_, tick, _, _) = bar
        .timed_iter()
        .find(|(_, _, _, e)| e.notation.dynamic == Some(Dynamic::F))
        .unwrap();
    assert_eq!(tick, 2 * Q);
    assert_eq!(result.score.connections().len(), 1);
}

#[test]
fn partial_chord_ties_do_not_suppress_untied_notes() {
    let chord = |stop: bool| {
        format!(
            "<note><pitch><step>C</step><octave>4</octave></pitch><duration>1</duration><tie type=\"{}\"/></note><note><chord/><pitch><step>E</step><octave>4</octave></pitch><duration>1</duration></note>",
            if stop { "stop" } else { "start" }
        )
    };
    let xml = wrap(
        "Piano",
        &format!(
            "<measure><attributes><divisions>1</divisions></attributes>{}{}</measure>",
            chord(false),
            chord(true)
        ),
    );
    let result = import(&xml);
    assert!(result.score.connections().is_empty());
    assert_eq!(attacks(&result.score).len(), 2);
    assert!(
        result
            .warnings
            .iter()
            .any(|s| s.contains("solo a algunas notas"))
    );
}

#[test]
fn cross_voice_shared_roll_does_not_roll_an_unmarked_partner() {
    let a = n("C", 4, 1, 1).replace(
        "</note>",
        "<notations><arpeggiate number=\"1\"/></notations></note>",
    );
    let c = n("E", 4, 6, 2).replace(
        "</note>",
        "<notations><arpeggiate number=\"1\"/></notations></note>",
    );
    let xml = wrap(
        "Piano",
        &format!(
            "<measure><attributes><divisions>1</divisions><staves>2</staves></attributes>{a}<backup><duration>4</duration></backup>{}<backup><duration>4</duration></backup>{c}</measure>",
            n("C", 4, 5, 2)
        ),
    );
    let result = import(&xml);
    assert!(result.score.piano_arpeggios().is_empty());
    assert!(result.warnings.iter().any(|s| s.contains("arpegio")));
}

#[test]
fn local_pizzicato_returns_to_arco_and_sound_directions_persist() {
    let local = n("C", 1, 1, 1).replace("<note>", "<note pizzicato=\"yes\">");
    let xml = wrap(
        "Violin",
        &format!(
            "<measure><attributes><divisions>1</divisions></attributes>{local}{}<direction><direction-type><words>pizz.</words></direction-type><sound pizzicato=\"yes\"/></direction>{}<sound pizzicato=\"no\"/>{}</measure>",
            n("D", 1, 1, 1),
            n("E", 1, 1, 1),
            n("F", 1, 1, 1)
        ),
    );
    let result = import(&xml);
    let techniques: Vec<_> = attacks(&result.score).iter().map(|a| a.7).collect();
    assert_eq!(
        techniques,
        [
            PlayingTechnique::Pizzicato,
            PlayingTechnique::Arco,
            PlayingTechnique::Pizzicato,
            PlayingTechnique::Arco
        ]
    );
    let xml = wrap(
        "Violin",
        &format!(
            "<measure><attributes><divisions>1</divisions></attributes><sound pizzicato=\"yes\"/>{}{}</measure>",
            n("C", 1, 1, 1),
            n("D", 1, 1, 1)
        ),
    );
    let result = import(&xml);
    assert!(
        attacks(&result.score)
            .iter()
            .all(|a| a.7 == PlayingTechnique::Pizzicato)
    );
    assert_eq!(
        result.score.parts()[0].measures()[0]
            .iter()
            .filter(|(_, e)| e.notation.technique.is_some())
            .count(),
        1
    );
}

#[test]
fn invalid_transpose_returns_an_error_instead_of_panicking() {
    let xml = wrap(
        "Clarinet",
        &format!(
            "<measure><attributes><divisions>1</divisions><transpose><chromatic>-8</chromatic><octave-change>-178956970</octave-change></transpose></attributes>{}</measure>",
            n("C", 4, 1, 1)
        ),
    );
    assert!(
        decode(xml.as_bytes())
            .unwrap_err()
            .to_string()
            .contains("transposición")
    );
}

#[test]
fn natural_harmonics_require_sounding_pitch_for_explicit_auxiliary_roles() {
    for role in ["touching-pitch", "base-pitch"] {
        let note=n("G",4,1,1).replace("</note>",&format!("<notations><technical><harmonic><natural/><{role}/></harmonic></technical></notations></note>"));
        let xml = wrap(
            "Violin",
            &format!("<measure><attributes><divisions>1</divisions></attributes>{note}</measure>"),
        );
        assert!(
            decode(xml.as_bytes())
                .unwrap_err()
                .to_string()
                .contains("altura sonora")
        );
    }
    let xml = wrap(
        "Violin",
        "<measure><attributes><divisions>1</divisions></attributes><note><pitch><step>G</step><octave>3</octave></pitch><duration>4</duration><notations><technical><harmonic><natural/><base-pitch/></harmonic></technical></notations></note><note><chord/><pitch><step>G</step><octave>5</octave></pitch><duration>4</duration><notations><technical><harmonic><natural/><sounding-pitch/></harmonic></technical></notations></note></measure>",
    );
    let result = import(&xml);
    let notes = attacks(&result.score);
    assert_eq!(notes.len(), 1);
    assert_eq!(notes[0].5, [67]);
    assert_eq!(notes[0].6, Some(Harmonic::Natural));
}

#[test]
fn chord_tones_without_voice_or_staff_inherit_the_previous_note_like_sibelius_exports() {
    let first = n("C", 1, 5, 2);
    let second = "<note><chord/><pitch><step>G</step><octave>4</octave></pitch><duration>1</duration></note>";
    let xml = wrap(
        "Piano",
        &format!(
            "<measure><attributes><divisions>1</divisions><staves>2</staves></attributes>{first}{second}</measure>"
        ),
    );
    let result = import(&xml);
    let p = &result.score.parts()[0];
    assert!(p.measures()[0].is_empty());
    let event = p.staff(StaffId::Lower).unwrap().measures()[0]
        .iter()
        .next()
        .unwrap()
        .1;
    assert_eq!(event.pitches.len(), 2);
}

#[test]
fn secondary_chord_tone_marks_are_imported_with_an_explicit_adaptation_warning() {
    let second = "<note pizzicato=\"yes\"><chord/><pitch><step>E</step><octave>4</octave></pitch><duration>1</duration><notations><articulations><staccato/></articulations></notations></note>";
    let xml = wrap(
        "Violin",
        &format!(
            "<measure><attributes><divisions>1</divisions></attributes>{}{second}</measure>",
            n("C", 1, 1, 1)
        ),
    );
    let result = import(&xml);
    let event = result.score.parts()[0].measures()[0]
        .iter()
        .next()
        .unwrap()
        .1;
    assert_eq!(event.notation.technique, Some(PlayingTechnique::Pizzicato));
    assert_eq!(event.notation.articulation, Some(Articulation::Staccato));
    assert_eq!(result.warnings.len(), 2);
}

#[test]
fn every_supported_tempo_beat_and_boundary_round_trips() {
    for beat in TempoBeat::ALL {
        for bpm in [20, 21, 400] {
            let mut score = Score::new("Tempo".into(), &[Instrument::Violin], 1).unwrap();
            let tempo = TempoMark {
                bpm,
                beat,
                text: "Tempo de prueba".into(),
            };
            score.set_tempo_at(0, tempo.clone()).unwrap();
            let xml = super::super::encode(&score).unwrap();
            let result = decode(&xml).unwrap();
            assert_eq!(result.score.tempo_at(0), &tempo);
            assert!(result.warnings.is_empty(), "{:?}", result.warnings);
        }
    }
}

#[test]
fn musescore_style_volta_and_words_import() {
    let data = wrap("Violin",&format!(
        "<measure number=\"1\"><attributes><divisions>1</divisions></attributes><barline location=\"left\"><ending number=\"1\" type=\"start\"/></barline>{}<barline location=\"right\"><ending number=\"1\" type=\"stop\"/><repeat direction=\"backward\"/></barline></measure>
        <measure number=\"2\"><barline location=\"left\"><ending number=\"2\" type=\"start\"/></barline><direction><direction-type><words>  D.C.  al Fine </words></direction-type><sound dacapo=\"yes\"/></direction>{}<barline location=\"right\"><ending number=\"2\" type=\"discontinue\"/></barline></measure>
        <measure number=\"3\"><direction><direction-type><words>Fine</words></direction-type><sound fine=\"yes\"/></direction>{}</measure>",n("C",4,1,1),n("D",4,1,1),n("E",4,1,1)));
    let result = import(&data);
    assert!(result.warnings.is_empty(), "{:?}", result.warnings);
    assert_eq!(result.score.bar_form(0).repeat_end, Some(2));
    assert_eq!(result.score.bar_form(1).jump, Some(Jump::DaCapoAlFine));
    assert!(result.score.bar_form(2).fine);
    assert_eq!(
        result.score.endings().cloned().collect::<Vec<_>>(),
        [
            Ending {
                start: 0,
                end: 0,
                numbers: vec![1]
            },
            Ending {
                start: 1,
                end: 1,
                numbers: vec![2]
            },
        ]
    );
}

#[test]
fn bad_ending_numbers_warn_and_keep_the_music() {
    let data = wrap(
        "Violin",
        &format!(
            "<measure number=\"1\"><attributes><divisions>1</divisions></attributes><barline location=\"left\"><ending number=\"9\" type=\"start\"/></barline>{}</measure>",
            n("C", 4, 1, 1)
        ),
    );
    let result = import(&data);
    assert_eq!(result.warnings, ["Casilla con números no admitidos"]);
    assert_eq!(attacks(&result.score).len(), 1);
    assert_eq!(result.score.endings().count(), 0);
}

#[test]
fn form_merges_all_parts_in_partwise_and_timewise_scores() {
    let list = "<part-list><score-part id=\"P1\"><part-name>Violin</part-name></score-part><score-part id=\"P2\"><part-name>Flute</part-name></score-part></part-list>";
    let attributes = "<attributes><divisions>1</divisions></attributes>";
    let bars = [
        [
            format!(
                "{attributes}<direction><direction-type><words>Fine</words></direction-type></direction>{}",
                n("C", 4, 1, 1)
            ),
            n("D", 4, 1, 1),
        ],
        [
            format!(
                "{attributes}<barline location=\"left\"><ending number=\"2, 1 2, 9\" type=\"start\"/><repeat direction=\"forward\"/></barline><sound dacapo=\"yes\"/>{}",
                n("E", 4, 1, 1)
            ),
            format!(
                "<direction><direction-type><coda/></direction-type><sound tocoda=\"coda\"/></direction><sound segno=\"s\"/>{}<barline location=\"right\"><bar-style>light-heavy</bar-style><ending number=\"1, 2\" type=\"discontinue\"/><repeat direction=\"backward\" times=\"99\"/></barline>",
                n("F", 4, 1, 1)
            ),
        ],
    ];
    for timewise in [false, true] {
        let mut data = format!(
            "<score-{} version=\"4.0\">{list}",
            if timewise { "timewise" } else { "partwise" }
        );
        if timewise {
            for (bar, (first, second)) in bars[0].iter().zip(&bars[1]).enumerate() {
                data.push_str(&format!("<measure number=\"{}\"><part id=\"P1\">{first}</part><part id=\"P2\">{second}</part></measure>",bar+1));
            }
        } else {
            for (part, bars) in bars.iter().enumerate() {
                data.push_str(&format!("<part id=\"P{}\">", part + 1));
                for (bar, body) in bars.iter().enumerate() {
                    data.push_str(&format!("<measure number=\"{}\">{body}</measure>", bar + 1));
                }
                data.push_str("</part>");
            }
        }
        data.push_str(if timewise {
            "</score-timewise>"
        } else {
            "</score-partwise>"
        });
        let result = import(&data);
        assert!(result.warnings.is_empty(), "{:?}", result.warnings);
        let first = result.score.bar_form(0);
        assert!(first.repeat_start && first.fine);
        assert_eq!(first.jump, Some(Jump::DaCapoAlFine));
        let last = result.score.bar_form(1);
        assert!(last.segno && last.to_coda && !last.coda);
        assert_eq!(last.repeat_end, Some(MAX_REPEAT_TIMES));
        assert_eq!(last.barline, Barline::Single);
        assert_eq!(
            result.score.endings().next(),
            Some(&Ending {
                start: 0,
                end: 1,
                numbers: vec![1, 2]
            })
        );
    }
}

#[test]
fn repeat_counts_and_unclosed_endings_are_bounded_without_losing_notes() {
    let mut measures = String::new();
    for (index, times) in ["0", "3", "99"].into_iter().enumerate() {
        measures.push_str(&format!("<measure number=\"{}\"><attributes><divisions>1</divisions></attributes>{}<barline location=\"right\"><bar-style>dashed</bar-style><repeat direction=\"backward\" times=\"{times}\"/></barline></measure>",index+1,n("C",4,1,1)));
    }
    measures = measures.replacen(
        "<note>",
        "<barline location=\"left\"><ending number=\"1\" type=\"start\"/></barline><note>",
        1,
    );
    let result = import(&wrap("Violin", &measures));
    assert!(result.warnings.is_empty(), "{:?}", result.warnings);
    assert_eq!(
        (0..3)
            .map(|bar| result.score.bar_form(bar).repeat_end)
            .collect::<Vec<_>>(),
        [Some(2), Some(3), Some(MAX_REPEAT_TIMES)]
    );
    assert_eq!(
        result.score.endings().next(),
        Some(&Ending {
            start: 0,
            end: 0,
            numbers: vec![1]
        })
    );
    assert_eq!(attacks(&result.score).len(), 3);
}

#[test]
fn two_note_tremolo_warns_and_keeps_both_notes() {
    let notes = ["start","stop"].into_iter().map(|kind|n("C",1,1,1).replace("</note>",&format!("<notations><ornaments><tremolo type=\"{kind}\">2</tremolo></ornaments></notations></note>"))).collect::<String>();
    let result = import(&wrap(
        "Violin",
        &format!(
            "<measure number=\"1\"><attributes><divisions>1</divisions></attributes>{notes}</measure>"
        ),
    ));
    assert_eq!(
        result.warnings,
        ["El trémolo entre dos notas no se importa"]
    );
    assert_eq!(attacks(&result.score).len(), 2);
    assert!(
        result.score.measures()[0]
            .iter()
            .all(|(_, event)| event.notation.tremolo.is_none())
    );
}

#[test]
fn mixed_rehearsal_sequence_keeps_only_exception_custom() {
    let bars=["A","B","Trío"].iter().map(|text|format!("<measure><direction><direction-type><rehearsal>{text}</rehearsal></direction-type></direction></measure>")).collect::<String>();
    let result = import(&wrap("Violin", &bars));
    assert!(result.warnings.is_empty(), "{:?}", result.warnings);
    assert_eq!(
        result
            .score
            .rehearsal_marks()
            .map(|(_, m)| m.custom.as_deref())
            .collect::<Vec<_>>(),
        [None, None, Some("Trío")]
    );
}
#[test]
fn rehearsal_from_later_part_and_timewise_and_long_labels() {
    let direction = "<direction><direction-type><rehearsal>WWWWabcdefghijklmnop</rehearsal></direction-type></direction>";
    let list = "<part-list><score-part id=\"P1\"><part-name>Violin</part-name></score-part><score-part id=\"P2\"><part-name>Cello</part-name></score-part></part-list>";
    for xml in [
        format!(
            "<score-partwise>{list}<part id=\"P1\"><measure/></part><part id=\"P2\"><measure>{direction}</measure></part></score-partwise>"
        ),
        format!(
            "<score-timewise>{list}<measure><part id=\"P1\"/><part id=\"P2\">{direction}</part></measure></score-timewise>"
        ),
    ] {
        let result = import(&xml);
        assert_eq!(result.score.rehearsal_label(0).unwrap().chars().count(), 16);
        assert!(!result.warnings.is_empty());
    }
}

/// One bar of 4/4 on a violin, with `divisions` per quarter.
fn violin_bar(divisions: u32, notes: &str) -> String {
    wrap(
        "Violin",
        &format!(
            "<measure number=\"1\"><attributes><divisions>{divisions}</divisions><time><beats>4</beats><beat-type>4</beat-type></time></attributes>{notes}</measure>"
        ),
    )
}

fn tuplet_note(duration: u32, kind: &str, ratio: (u8, u8), extra: &str, bracket: &str) -> String {
    format!(
        "<note><pitch><step>C</step><octave>5</octave></pitch><duration>{duration}</duration><voice>1</voice><type>{kind}</type><time-modification><actual-notes>{}</actual-notes><normal-notes>{}</normal-notes>{extra}</time-modification><notations>{bracket}</notations></note>",
        ratio.0, ratio.1
    )
}

#[test]
fn tuplets_of_any_supported_ratio_import_with_their_unit() {
    // A quarter and an eighth in a triplet of eighths: one group. With 15
    // divisions to the quarter, they last 10 and 5.
    let notes = [
        tuplet_note(
            10,
            "quarter",
            (3, 2),
            "<normal-type>eighth</normal-type>",
            "<tuplet type=\"start\"/>",
        ),
        tuplet_note(5, "eighth", (3, 2), "", "<tuplet type=\"stop\"/>"),
        // Five sixteenths in the time of four.
        tuplet_note(3, "16th", (5, 4), "", ""),
        tuplet_note(3, "16th", (5, 4), "", ""),
        tuplet_note(3, "16th", (5, 4), "", ""),
        tuplet_note(3, "16th", (5, 4), "", ""),
        tuplet_note(3, "16th", (5, 4), "", ""),
    ]
    .concat();
    let half = "<note><rest/><duration>30</duration><voice>1</voice><type>half</type></note>";
    let score = import(&violin_bar(15, &format!("{notes}{half}"))).score;
    let bar = &score.parts()[0].measures()[0];
    let groups = bar.tuplet_groups(Voice::One);
    assert_eq!(groups.len(), 2);
    assert_eq!(
        groups[0].tuplet,
        Tuplet::new(3, 2, NoteValue::Eighth).unwrap()
    );
    assert_eq!(groups[0].events.len(), 2);
    assert!(groups.iter().all(|g| g.is_complete()));
    assert_eq!(
        groups[1].tuplet,
        Tuplet::new(5, 4, NoteValue::Sixteenth).unwrap()
    );
    assert_eq!(bar.voice_ticks(Voice::One), score.time().capacity());
}

#[test]
fn nested_and_unsupported_tuplets_are_refused_with_their_bar() {
    let two_starts = "<tuplet type=\"start\" number=\"1\"/><tuplet type=\"start\" number=\"2\"/>";
    let nested = violin_bar(15, &tuplet_note(2, "16th", (15, 8), "", two_starts));
    let error = decode(nested.as_bytes()).unwrap_err().to_string();
    assert!(
        error.contains("grupillos anidados en el compás 1"),
        "{error}"
    );
    let eleven = violin_bar(11, &tuplet_note(4, "16th", (11, 8), "", ""));
    let error = decode(eleven.as_bytes()).unwrap_err().to_string();
    assert!(
        error.contains("un grupillo que Levare no admite en el compás 1"),
        "{error}"
    );
}
#[test]
fn a_crossing_arpeggio_round_trips_through_musicxml() {
    let mut score = Score::default();
    let piano = score.parts()[0].id();
    let eighth = |step, octave| Event {
        pitches: Pitches::single(Pitch::new(step, octave)),
        duration: Duration::Eighth,
        notation: Notation::default(),
    };
    let ids: Vec<_> = [(Step::C, 3), (Step::G, 3), (Step::C, 4), (Step::E, 4)]
        .into_iter()
        .enumerate()
        .map(|(i, (step, octave))| {
            score
                .insert_on(piano, StaffId::Lower, 0, i, eighth(step, octave))
                .unwrap()
        })
        .collect();
    for &id in &ids[2..] {
        score.set_cross(id, true).unwrap();
    }
    // Written on the staff where it is drawn, in its own voice.
    let xml = String::from_utf8(crate::musicxml::encode(&score).unwrap()).unwrap();
    let document = roxmltree::Document::parse(&xml).unwrap();
    let written: Vec<_> = document
        .descendants()
        .filter(|n| n.has_tag_name("note") && child(*n, "pitch").is_some())
        .map(|n| (text(n, "voice").unwrap(), text(n, "staff").unwrap()))
        .collect();
    assert_eq!(written, [("5", "2"), ("5", "2"), ("5", "1"), ("5", "1")]);
    let imported = import(&xml).score;
    let lower = &imported.parts()[0].staves()[1].measures()[0];
    let crossing: Vec<_> = lower
        .voice_iter(Voice::One)
        .map(|(_, event)| event.notation.cross)
        .collect();
    // The rest that fills the bar stays on its own staff.
    assert_eq!(crossing, [false, false, true, true, false]);
    // Nothing moved into the right hand's own voices.
    assert!(
        imported.parts()[0].staves()[0].measures()[0]
            .iter()
            .all(|(_, event)| event.pitches.is_empty())
    );
}
#[test]
fn one_voice_name_on_both_staves_at_once_is_two_voices_not_a_crossing() {
    // Finale numbers each staff's first voice 1.
    let result = import(&wrap(
        "Piano",
        &format!(
            "<measure number=\"1\"><attributes><divisions>1</divisions><staves>2</staves></attributes>{}<backup><duration>4</duration></backup>{}</measure>",
            n("E", 4, 1, 1),
            n("C", 4, 1, 2)
        ),
    ));
    let part = &result.score.parts()[0];
    for staff in part.staves() {
        let measure = &staff.measures()[0];
        assert_eq!(measure.voice_len(Voice::One), 1);
        assert!(measure.iter().all(|(_, event)| !event.notation.cross));
    }
}

/// A quarter note with its own notations, on the first staff.
fn quarter(step: &str, inside: &str, notations: &str) -> String {
    format!(
        "<note>{inside}<pitch><step>{step}</step><octave>4</octave></pitch><duration>1</duration><voice>1</voice><type>quarter</type><notations>{notations}</notations></note>"
    )
}
/// The events of the first staff's first bar.
fn first_bar(result: &ImportResult) -> Vec<Event> {
    result.score.parts()[0].staves()[0].measures()[0]
        .timed_iter()
        .map(|(_, _, _, event)| event.clone())
        .collect()
}

/// What a musician reads in a one-staff score: every mark at its position,
/// connections between positions, and the bars' form, clefs and changes.
fn readable(score: &Score) -> Vec<String> {
    let part = &score.parts()[0];
    let mut at = HashMap::new();
    let mut found = Vec::new();
    for (bar, measure) in part.staves()[0].measures().iter().enumerate() {
        for (_, onset, event, e) in measure.timed_iter() {
            let id = ElementId {
                part: part.id(),
                staff: StaffId::Upper,
                measure: bar,
                event,
            };
            at.insert(id, (bar, onset));
            let n = &e.notation;
            let lyrics: Vec<_> = n
                .lyrics
                .verses()
                .map(|(verse, s)| format!("{verse}:{}:{:?}", s.text(), s.join))
                .collect();
            found.push(format!(
                "{bar}@{onset} {:?} {lyrics:?} {:?} {:?} {:?} {:?} {:?} {:?} {:?} {:?} {} {:?} {:?}",
                // Accidentals as they sound in the bar's key.
                e.pitches
                    .in_key(score.key_signature_at(bar))
                    .iter()
                    .collect::<Vec<_>>(),
                n.chord.map(|c| c.label()),
                n.dynamic,
                n.expression_text.map(|w| w.to_string()),
                n.technique_text.map(|w| w.to_string()),
                n.articulation,
                n.ornament,
                n.tremolo,
                n.notehead,
                n.fermata,
                n.pause,
                n.grace,
            ));
        }
        found.push(format!(
            "bar {bar}: {:?} {:?} {:?} {:?} {:?}",
            score.bar_form(bar),
            part.clef_at(StaffId::Upper, bar),
            score.tempo_change_at(bar),
            score.key_change_at(bar),
            score.rehearsal_label(bar),
        ));
    }
    found.extend(score.endings().map(|e| format!("{e:?}")));
    let mut links: Vec<_> = score
        .connections()
        .iter()
        .map(|c| format!("{:?} {:?}-{:?}", c.kind, at[&c.start], at[&c.end]))
        .collect();
    links.sort();
    found.extend(links);
    found
}

#[test]
fn a_score_with_every_mark_levare_writes_round_trips_through_musicxml() {
    let mut score = Score::new("Todo".into(), &[Instrument::Viola], 4).unwrap();
    let part = score.parts()[0].id();
    let steps = [Step::C, Step::D, Step::E, Step::F, Step::G, Step::A];
    let mut ids = Vec::new();
    for bar in 0..4 {
        for index in 0..4 {
            let event = Event {
                pitches: Pitches::single(Pitch::new(steps[(bar + index) % 6], 4)),
                duration: Duration::Quarter,
                notation: Notation::default(),
            };
            ids.push(score.insert_in(part, bar, index, event).unwrap());
        }
    }
    let sing = |text, join| Some(Syllable::new(text, join).unwrap());
    score
        .set_syllable(ids[0], 0, sing("Can", Join::Hyphen))
        .unwrap();
    score
        .set_syllable(ids[1], 0, sing("ta", Join::Melisma))
        .unwrap();
    score
        .set_syllable(ids[0], 1, sing("Sol", Join::Word))
        .unwrap();
    for (id, chord) in [(0, "Dm7"), (2, "G7b9/B"), (4, "Cmaj7"), (8, "F#ø")] {
        score
            .set_chord(ids[id], Some(ChordSymbol::parse(chord).unwrap()))
            .unwrap();
    }
    score.set_dynamic(ids[0], Some(Dynamic::P)).unwrap();
    score.set_dynamic(ids[5], Some(Dynamic::Sfz)).unwrap();
    score
        .set_text(ids[0], TextKind::Expression, Some("dolce"))
        .unwrap();
    score
        .set_text(ids[1], TextKind::Technique, Some("con sord."))
        .unwrap();
    score
        .set_articulation(ids[1], Some(Articulation::Staccato))
        .unwrap();
    score
        .set_articulation(ids[6], Some(Articulation::Marcato))
        .unwrap();
    score.set_ornament(ids[2], Some(Ornament::Turn)).unwrap();
    score
        .set_ornament(ids[7], Some(Ornament::InvertedMordent))
        .unwrap();
    score.set_tremolo(ids[3], Some(Tremolo::Two)).unwrap();
    score.set_notehead(ids[9], Some(Notehead::Cross)).unwrap();
    score.set_fermata(ids[15], true).unwrap();
    score.set_pause(ids[11], Some(Pause::Caesura)).unwrap();
    score
        .set_grace(ids[13], Some(GraceKind::Acciaccatura))
        .unwrap();
    for (start, end, kind) in [
        (0, 1, ConnectionKind::Slur),
        (1, 3, ConnectionKind::Crescendo),
        (4, 6, ConnectionKind::Trill),
        (6, 7, ConnectionKind::Glissando),
        (8, 10, ConnectionKind::OttavaAlta),
        (12, 14, ConnectionKind::Diminuendo),
    ] {
        score
            .add_connection(Connection {
                start: ids[start],
                end: ids[end],
                kind,
            })
            .unwrap();
    }
    score.toggle_rehearsal_mark(1).unwrap();
    score
        .set_bar_form(
            1,
            BarForm {
                repeat_start: true,
                ..Default::default()
            },
        )
        .unwrap();
    score
        .set_bar_form(
            2,
            BarForm {
                repeat_end: Some(2),
                ..Default::default()
            },
        )
        .unwrap();
    for (bar, number) in [(2, 1), (3, 2)] {
        score
            .set_ending(Ending {
                start: bar,
                end: bar,
                numbers: vec![number],
            })
            .unwrap();
    }
    score
        .set_clef_at(part, StaffId::Upper, 2, Clef::Treble)
        .unwrap();
    score
        .set_tempo_at(
            2,
            TempoMark {
                bpm: 92,
                beat: TempoBeat::Quarter,
                text: "Più mosso".into(),
            },
        )
        .unwrap();
    score
        .set_key_signature_at(3, KeySignature::new(2, KeyMode::Major).unwrap())
        .unwrap();
    let xml = super::super::encode(&score).unwrap();
    let opened = decode(&xml).unwrap();
    assert!(opened.warnings.is_empty(), "{:?}", opened.warnings);
    assert_eq!(readable(&opened.score), readable(&score));
}

#[test]
fn clef_changes_inside_a_bar_move_to_the_next_barline() {
    let clef = |sign: &str, line: u8| {
        format!("<attributes><clef><sign>{sign}</sign><line>{line}</line></clef></attributes>")
    };
    let notes = |count: usize| quarter("D", "", "").repeat(count);
    let open = |middle: &str, end: &str| {
        import(&wrap(
            "Violoncello",
            &format!(
                "<measure number=\"1\"><attributes><divisions>1</divisions><time><beats>4</beats><beat-type>4</beat-type></time>{}</attributes>{}{middle}{}</measure><measure number=\"2\">{}{end}</measure><measure number=\"3\">{}</measure>",
                "<clef><sign>F</sign><line>4</line></clef>",
                notes(2),
                notes(2),
                notes(4),
                notes(4)
            ),
        ))
    };
    // Sibelius writes the clef of the next bar before the barline.
    let before_barline = open("", &clef("C", 4));
    let part = &before_barline.score.parts()[0];
    assert_eq!(part.clef_at(StaffId::Upper, 1), Clef::Bass);
    assert_eq!(part.clef_at(StaffId::Upper, 2), Clef::Tenor);
    assert!(
        !before_barline.warnings.iter().any(|w| w.contains("clave")),
        "{:?}",
        before_barline.warnings
    );
    // A change on the third beat was refused before; now it waits for the barline.
    let inside = open(&clef("C", 4), "");
    let part = &inside.score.parts()[0];
    assert_eq!(part.clef_at(StaffId::Upper, 0), Clef::Bass);
    assert_eq!(part.clef_at(StaffId::Upper, 1), Clef::Tenor);
    assert!(
        inside
            .warnings
            .iter()
            .any(|w| w.contains("compás siguiente")),
        "{:?}",
        inside.warnings
    );
    assert_eq!(attacks(&inside.score), attacks(&before_barline.score));
}

#[test]
fn words_written_in_runs_or_over_lines_import_as_one_text() {
    let direction = |placement: &str, words: &str| {
        format!(
            "<direction placement=\"{placement}\"><direction-type>{words}</direction-type></direction>"
        )
    };
    let measure = [
        direction(
            "below",
            "<words>poco a poco </words><words font-style=\"italic\">cresc.</words>",
        ),
        quarter("C", "", ""),
        direction("above", "<words>con\n      sord.</words>"),
        quarter("D", "", ""),
        direction(
            "above",
            "<words font-weight=\"bold\">molto</words><words default-x=\"40\">rit.</words>",
        ),
        quarter("E", "", ""),
        direction(
            "above",
            "<words>D.S. </words><words font-size=\"9\">al Coda</words>",
        ),
        quarter("F", "", ""),
    ]
    .concat();
    let result = import(&wrap(
        "Violin",
        &format!(
            "<measure number=\"1\"><attributes><divisions>1</divisions></attributes>{measure}</measure>"
        ),
    ));
    let texts: Vec<_> = first_bar(&result)
        .iter()
        .map(|e| {
            (
                e.notation.expression_text.map(|w| w.to_string()),
                e.notation.technique_text.map(|w| w.to_string()),
            )
        })
        .collect();
    assert_eq!(
        texts,
        [
            (Some("poco a poco cresc.".to_owned()), None),
            (None, Some("con sord.".to_owned())),
            (None, Some("molto rit.".to_owned())),
            (None, None),
        ]
    );
    assert_eq!(result.score.bar_form(0).jump, Some(Jump::DalSegnoAlCoda));
    assert!(result.warnings.is_empty(), "{:?}", result.warnings);
}

#[test]
fn marks_levare_lacks_warn_by_name_and_a_stroke_reads_as_staccatissimo() {
    let measure = [
        quarter("C", "", "<articulations><spiccato/></articulations>"),
        // Finale: a trill with its accidental.
        quarter(
            "D",
            "",
            "<ornaments><trill-mark/><accidental-mark>sharp</accidental-mark></ornaments>",
        ),
        quarter("E", "", "<ornaments><inverted-turn/></ornaments>"),
        quarter("F", "", "<ornaments><mordent/></ornaments>"),
        "<barline location=\"right\"><bar-style>light-heavy</bar-style><fermata type=\"upright\"/></barline>".to_owned(),
    ]
    .concat();
    let result = import(&wrap(
        "Violin",
        &format!(
            "<measure number=\"1\"><attributes><divisions>1</divisions></attributes>{measure}</measure>"
        ),
    ));
    let events = first_bar(&result);
    assert_eq!(
        events[0].notation.articulation,
        Some(Articulation::Staccatissimo)
    );
    assert_eq!(events[3].notation.ornament, Some(Ornament::Mordent));
    assert!(
        result
            .score
            .connections()
            .iter()
            .any(|c| c.kind == ConnectionKind::Trill)
    );
    for warning in [
        "spiccato",
        "alteraciones de los ornamentos",
        "«inverted-turn»",
        "barra de compás",
    ] {
        assert!(
            result.warnings.iter().any(|w| w.contains(warning)),
            "{warning}: {:?}",
            result.warnings
        );
    }
}

#[test]
fn dynamics_on_notes_and_beyond_levares_range_import_at_the_nearest_mark() {
    let direction = |dynamic: &str| {
        format!(
            "<direction placement=\"below\"><direction-type><dynamics>{dynamic}</dynamics></direction-type></direction>"
        )
    };
    let measure = [
        direction("<pppp/>"),
        quarter("C", "", ""),
        // A dynamic attached to the note, as Dorico and Finale may write it.
        quarter("D", "", "<dynamics><sfz/></dynamics>"),
        direction("<other-dynamics>sfzp</other-dynamics>"),
        quarter("E", "", ""),
        quarter("F", "", ""),
        quarter("A", "<chord/>", "<dynamics><ffff/></dynamics>"),
    ]
    .concat();
    let result = import(&wrap(
        "Violin",
        &format!(
            "<measure number=\"1\"><attributes><divisions>1</divisions></attributes>{measure}</measure>"
        ),
    ));
    let dynamics: Vec<_> = first_bar(&result)
        .iter()
        .map(|e| e.notation.dynamic)
        .collect();
    assert_eq!(
        dynamics,
        [
            Some(Dynamic::Ppp),
            Some(Dynamic::Sfz),
            Some(Dynamic::Sfp),
            Some(Dynamic::Fff)
        ]
    );
    assert!(
        result.warnings.iter().any(|w| w.contains("pppp")),
        "{:?}",
        result.warnings
    );
    assert!(
        !result.warnings.iter().any(|w| w.contains("dynamics")),
        "{:?}",
        result.warnings
    );
}
