//! Orchestrate the existing string Menuetto, preserving its expressive passages.
use score_core::*;
use std::{error::Error, path::Path};

fn main() -> Result<(), Box<dyn Error>> {
    let mut args = std::env::args_os().skip(1);
    let source = args
        .next()
        .expect("usage: menuetto_orquesta strings.partitura output.partitura");
    let output = args.next().expect("output.partitura");
    let original = score_io::load(Path::new(&source))?;
    let mut data = serde_json::to_value(&original)?;
    data["title"] = "Menuetto y Trio en Re — Orquesta completa".into();
    // The earlier string document stores the bass in concert pitch. Raise its
    // notation an octave while adding the normal written-to-sounding interval.
    for part in data["parts"].as_array_mut().unwrap() {
        if part["instrument"] == "DoubleBass" {
            part["transposition"] =
                serde_json::to_value(Instrument::DoubleBass.standard_transposition())?;
            for staff in part["staves"].as_array_mut().unwrap() {
                for bar in staff["measures"].as_array_mut().unwrap() {
                    for entry in bar["events"].as_array_mut().unwrap() {
                        if let Some(pitches) = entry
                            .get_mut("value")
                            .and_then(|event| event.get_mut("pitches"))
                            .and_then(serde_json::Value::as_array_mut)
                        {
                            for pitch in pitches {
                                pitch["octave"] = (pitch["octave"].as_u64().unwrap() + 1).into();
                            }
                        }
                    }
                }
            }
        }
    }
    let mut score: Score = serde_json::from_value(data)?;
    let source_ids: Vec<_> = original.parts().iter().map(Part::id).collect();
    let mut desks = Vec::new();
    for instrument in Instrument::ORCHESTRA {
        // Two trumpets and orchestral bass drum/cymbals; no military snare part.
        if instrument.family() == InstrumentFamily::Strings
            || instrument == Instrument::SnareDrum
            || (instrument == Instrument::TrumpetBb
                && desks.iter().filter(|(i, _, _)| *i == instrument).count() == 2)
        {
            continue;
        }
        let desk = desks.iter().filter(|(i, _, _)| *i == instrument).count();
        let part = score.add_part(instrument)?;
        desks.push((instrument, part, desk));
    }
    for (instrument, part, desk) in desks {
        let mut was_playing = false;
        let mut last_dynamic = None;
        for bar in 0..original.measures().len() {
            if instrument == Instrument::Harp {
                harp(&original, &mut score, &source_ids, part, bar)?;
                continue;
            }
            let mut events = orchestrate(&original, &source_ids, instrument, desk, bar);
            let playing = events.iter().any(|event| !event.pitches.is_empty());
            for event in &mut events {
                if let Some(dynamic) = event.notation.dynamic {
                    if was_playing && bar % 8 != 0 && last_dynamic == Some(dynamic) {
                        event.notation.dynamic = None;
                    }
                    last_dynamic = Some(dynamic);
                }
            }
            was_playing = playing;
            let mut pitched = Vec::new();
            for (index, mut event) in events.into_iter().enumerate() {
                let concert = event.pitches.in_key(original.key_signature_at(bar));
                if !event.pitches.is_empty() {
                    event.pitches = instrument
                        .standard_transposition()
                        .inverse()
                        .pitches(concert, KeySignature::default())?;
                    // Prefer the signature over redundant naturals/sharps.
                    let key = score.written_key_at(part, bar);
                    event.pitches = Pitches::new(
                        &event
                            .pitches
                            .iter()
                            .map(|p| {
                                if p.alteration()
                                    == Pitch::new(p.step, p.octave())
                                        .unwrap()
                                        .in_key(key)
                                        .alteration()
                                {
                                    p.with_accidental(None)
                                } else {
                                    p
                                }
                            })
                            .collect::<Vec<_>>(),
                    )?;
                }
                let id = score.insert_on(part, StaffId::Upper, bar, index, event)?;
                if instrument.percussion_key().is_none() {
                    assert_eq!(heights(concert), heights(score.sounding_pitches(id)?));
                }
                if !event.pitches.is_empty() {
                    pitched.push(id);
                }
            }
            if instrument.family() == InstrumentFamily::Woodwinds && pitched.len() > 1 {
                score.add_connection(Connection {
                    start: pitched[0],
                    end: *pitched.last().unwrap(),
                    kind: ConnectionKind::Slur,
                })?;
            }
        }
    }
    score.arrange_parts();
    score.validate()?;
    // Preserve every sounding string note, including harmonics and divisi.
    for old_part in original.parts() {
        for staff in old_part.staves() {
            for (bar, measure) in staff.measures().iter().enumerate() {
                for (event, _) in measure.iter() {
                    let id = ElementId {
                        part: old_part.id(),
                        staff: staff.id(),
                        measure: bar,
                        event,
                    };
                    assert_eq!(
                        heights(original.sounding_pitches(id)?),
                        heights(score.sounding_pitches(id)?)
                    );
                }
            }
        }
    }
    score_io::save(Path::new(&output), &score)?;
    assert_eq!(score_io::load(Path::new(&output))?, score);
    println!(
        "{} partes · {} compases · cuerdas conservadas en altura real",
        score.parts().len(),
        score.measures().len()
    );
    for p in score
        .parts()
        .iter()
        .filter(|p| !p.transposition().is_concert())
    {
        println!(
            "{}: armadura inicial {:?}",
            score.part_name(p.id()),
            score.written_key_at(p.id(), 0)
        );
    }
    Ok(())
}

fn heights(pitches: Pitches) -> Vec<i16> {
    pitches
        .iter()
        .map(|p| {
            i16::from(p.octave()) * 12
                + [0, 2, 4, 5, 7, 9, 11][p.step.index() as usize]
                + i16::from(p.alteration())
        })
        .collect()
}

fn rest() -> Event {
    Event {
        pitches: Pitches::default(),
        duration: Duration::Half.with_dotted(true),
        notation: Notation::default(),
    }
}

fn line(
    source: &Score,
    part: PartId,
    bar: usize,
    octave: i16,
    low: i16,
    high: i16,
    dynamic: Dynamic,
) -> Vec<Event> {
    source.part(part).unwrap().measures()[bar]
        .voice_iter(Voice::One)
        .enumerate()
        .map(|(index, (_, e))| {
            let pitch = e.pitches.last().map(|p| {
                let mut p = p.shift(octave * 7).unwrap();
                while p.diatonic() < low {
                    p = p.shift(7).unwrap();
                }
                while p.diatonic() > high {
                    p = p.shift(-7).unwrap();
                }
                p
            });
            Event {
                pitches: Pitches::single(pitch),
                duration: e.duration,
                notation: Notation {
                    dynamic: (index == 0).then_some(dynamic),
                    articulation: e.notation.articulation,
                    fermata: e.notation.fermata,
                    ..Default::default()
                },
            }
        })
        .collect()
}

fn sustain(
    source: &Score,
    part: PartId,
    bar: usize,
    octave: i16,
    low: i16,
    high: i16,
    dynamic: Dynamic,
) -> Vec<Event> {
    let mut events = line(source, part, bar, octave, low, high, dynamic);
    let pitch = events.iter().find_map(|e| e.pitches.first());
    let mut event = rest();
    event.pitches = Pitches::single(pitch);
    event.notation.dynamic = Some(dynamic);
    event.notation.fermata = bar == 83;
    events.clear();
    vec![event]
}

fn orchestrate(
    source: &Score,
    ids: &[PartId],
    instrument: Instrument,
    desk: usize,
    bar: usize,
) -> Vec<Event> {
    let trio = (32..64).contains(&bar);
    let tutti = (16..32).contains(&bar) || (64..80).contains(&bar);
    let cadence = [7, 15, 23, 31, 71, 79].contains(&bar);
    let dynamic = if trio {
        Dynamic::P
    } else if tutti {
        Dynamic::Mf
    } else {
        Dynamic::Mp
    };
    match instrument {
        Instrument::Piccolo if tutti && (bar % 8 >= 4) => {
            line(source, ids[0], bar, 1, 38, 48, Dynamic::P)
        }
        Instrument::Flute
            if (desk == 0 && (!trio && bar % 8 >= 4 || (48..56).contains(&bar)))
                || (desk == 1 && ((40..48).contains(&bar) || tutti)) =>
        {
            line(
                source,
                ids[if desk == 0 { 0 } else { 1 }],
                bar,
                0,
                30,
                44,
                dynamic,
            )
        }
        Instrument::Oboe
            if (desk == 0 && (!trio && bar % 8 < 4 || (56..64).contains(&bar)))
                || (desk == 1 && (tutti || (48..56).contains(&bar))) =>
        {
            line(
                source,
                ids[if desk == 0 { 0 } else { 2 }],
                bar,
                if desk == 0 { -1 } else { 0 },
                27,
                40,
                dynamic,
            )
        }
        Instrument::EnglishHorn if (32..40).contains(&bar) || (56..64).contains(&bar) => {
            line(source, ids[0], bar, -1, 23, 36, Dynamic::Mp)
        }
        Instrument::EnglishHorn if tutti && bar % 4 < 2 => {
            sustain(source, ids[2], bar, 0, 23, 35, Dynamic::P)
        }
        Instrument::ClarinetBb
            if desk == 0 && ((40..48).contains(&bar) || !trio)
                || desk == 1 && (tutti || (48..64).contains(&bar) || bar < 8) =>
        {
            line(
                source,
                ids[if desk == 0 && trio {
                    0
                } else if desk == 0 {
                    1
                } else {
                    2
                }],
                bar,
                0,
                23,
                40,
                dynamic,
            )
        }
        Instrument::BassClarinet if trio || tutti => {
            line(source, ids[3], bar, 0, 16, 30, Dynamic::P)
        }
        Instrument::Bassoon if desk == 0 || tutti || (56..64).contains(&bar) => line(
            source,
            ids[if desk == 0 { 3 } else { 2 }],
            bar,
            if desk == 0 { 0 } else { -1 },
            13,
            29,
            dynamic,
        ),
        Instrument::Contrabassoon if tutti => line(source, ids[4], bar, 0, 6, 19, Dynamic::P),
        Instrument::HornF if tutti || desk < 2 && (bar % 4 < 2 || (48..56).contains(&bar)) => {
            sustain(
                source,
                ids[[1, 2, 3, 4][desk]],
                bar,
                if desk == 0 { -1 } else { 0 },
                18,
                32,
                if trio { Dynamic::Pp } else { Dynamic::P },
            )
        }
        Instrument::TrumpetBb if tutti && bar % 8 >= 4 || cadence && bar < 32 => line(
            source,
            ids[if desk == 0 { 1 } else { 2 }],
            bar,
            0,
            25,
            37,
            Dynamic::Mp,
        ),
        Instrument::Trombone if tutti && (bar.is_multiple_of(4) || bar % 4 == 3) => sustain(
            source,
            ids[if desk == 0 { 2 } else { 3 }],
            bar,
            if desk == 0 { -1 } else { 0 },
            15,
            28,
            Dynamic::Mp,
        ),
        Instrument::BassTrombone | Instrument::Tuba if tutti && bar % 4 >= 2 => sustain(
            source,
            ids[3],
            bar,
            if instrument == Instrument::Tuba {
                -1
            } else {
                0
            },
            9,
            23,
            Dynamic::P,
        ),
        Instrument::Timpani if cadence || tutti && bar.is_multiple_of(4) => {
            let key = source.key_signature_at(bar);
            let (root, fifth) = if key.fifths() == 2 {
                (Step::D, Step::A)
            } else if key.mode() == KeyMode::Minor {
                (Step::E, Step::B)
            } else {
                (Step::G, Step::D)
            };
            vec![
                Event {
                    pitches: Pitches::single(Pitch::new(root, 2)),
                    duration: Duration::Half,
                    notation: Notation {
                        dynamic: Some(Dynamic::P),
                        ..Default::default()
                    },
                },
                Event {
                    pitches: Pitches::single(Pitch::new(fifth, 2)),
                    duration: Duration::Quarter,
                    notation: Notation::default(),
                },
            ]
        }
        Instrument::BassDrum | Instrument::Cymbals if [23, 31, 71, 79].contains(&bar) => {
            vec![
                Event {
                    pitches: Pitches::single(Pitch::new(Step::B, 4)),
                    duration: Duration::Quarter,
                    notation: Notation {
                        dynamic: Some(Dynamic::Pp),
                        ..Default::default()
                    },
                },
                Event {
                    duration: Duration::Half,
                    ..rest()
                },
            ]
        }
        Instrument::Triangle if [40, 44, 48, 52, 56, 60].contains(&bar) => {
            vec![Event {
                pitches: Pitches::single(Pitch::new(Step::B, 4)),
                notation: Notation {
                    dynamic: Some(Dynamic::Pp),
                    ..Default::default()
                },
                ..rest()
            }]
        }
        _ => vec![rest()],
    }
}

fn harp(
    source: &Score,
    score: &mut Score,
    ids: &[PartId],
    part: PartId,
    bar: usize,
) -> Result<(), ScoreError> {
    let active = (32..64).contains(&bar) && bar.is_multiple_of(2)
        || [0, 8, 16, 24, 64, 72, 80, 83].contains(&bar);
    let key = source.key_signature_at(bar);
    let mut upper = Pitches::default();
    let mut bass = None;
    if active {
        for (index, id) in ids.iter().take(4).enumerate() {
            let pitch = source.part(*id).unwrap().measures()[bar]
                .voice_iter(Voice::One)
                .find_map(|(_, e)| e.pitches.first());
            if let Some(mut pitch) = pitch {
                pitch = pitch.in_key(key);
                if index == 3 {
                    bass = Some(pitch);
                } else {
                    while pitch.diatonic() < 26 {
                        pitch = pitch.shift(7).unwrap();
                    }
                    while pitch.diatonic() > 37 {
                        pitch = pitch.shift(-7).unwrap();
                    }
                    if !upper.iter().any(|p| p.diatonic() == pitch.diatonic()) {
                        upper = upper.with(pitch)?;
                    }
                }
            }
        }
    }
    let id = score.insert_on(
        part,
        StaffId::Upper,
        bar,
        0,
        Event {
            pitches: upper,
            notation: Notation {
                dynamic: active.then_some(Dynamic::Pp),
                fermata: bar == 83,
                ..Default::default()
            },
            ..rest()
        },
    )?;
    score.insert_on(
        part,
        StaffId::Lower,
        bar,
        0,
        Event {
            pitches: Pitches::single(bass),
            notation: Notation {
                fermata: bar == 83,
                ..Default::default()
            },
            ..rest()
        },
    )?;
    if active && bass.is_some() && upper.len() >= 2 {
        score.set_arpeggio(id, Some(Arpeggio::Up))?;
        score.set_arpeggio_both_hands(id, true)?;
    }
    Ok(())
}
