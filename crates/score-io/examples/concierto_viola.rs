//! Original short concerto; all musical data goes through the score's public API.
use score_core::*;
use std::{error::Error, path::Path};

type Result<T> = std::result::Result<T, Box<dyn Error>>;
const BARS: usize = 224;
const NAMES: [&str; 6] = [
    "Viola solista",
    "Violines I",
    "Violines II",
    "Violas",
    "Violonchelos",
    "Contrabajos",
];

fn pitch(text: &str) -> Pitch {
    let bytes = text.as_bytes();
    let step = Step::from_letter(&text[..1].to_lowercase()).unwrap();
    let octave = bytes[bytes.len() - 1] - b'0';
    let accidental = match bytes.get(1) {
        Some(b'#') => Some(Accidental::Sharp),
        Some(b'b') => Some(Accidental::Flat),
        Some(b'n') => Some(Accidental::Natural),
        _ => None,
    };
    Pitch::new(step, octave)
        .unwrap()
        .with_accidental(accidental)
}

fn duration(text: &str) -> Duration {
    let base = match text.as_bytes()[0] {
        b'w' => Duration::Whole,
        b'h' => Duration::Half,
        b'q' => Duration::Quarter,
        b'e' => Duration::Eighth,
        b's' => Duration::Sixteenth,
        _ => panic!("unknown duration: {text}"),
    };
    base.with_dotted(text.contains('.'))
        .with_triplet(text.contains('t'))
}

fn phrase(text: &str) -> Vec<Event> {
    text.split_whitespace()
        .map(|token| {
            let (notes, value) = token.split_once('/').unwrap();
            Event {
                pitches: if notes == "r" {
                    Pitches::default()
                } else {
                    Pitches::new(&notes.split('+').map(pitch).collect::<Vec<_>>()).unwrap()
                },
                duration: duration(value),
                notation: Notation::default(),
            }
        })
        .collect()
}

fn signature_pitches(pitches: Pitches, key: KeySignature) -> Pitches {
    Pitches::new(
        &pitches
            .iter()
            .map(|p| {
                if p.accidental().is_some()
                    && p.alteration()
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
    )
    .unwrap()
}

// Four independently voiced desks, from the cello up to first violins.
fn chord(name: &str) -> [&'static str; 4] {
    match name {
        "Dm" => ["D3", "A3", "F4", "D5"],
        "Gm" => ["G2", "Bb3", "G4", "D5"],
        "A" => ["A2", "A3", "E4", "C#5"],
        "A7" => ["A2", "G3", "E4", "C#5"],
        "Bb" => ["Bb2", "F3", "D4", "Bb4"],
        "F" => ["F2", "A3", "F4", "C5"],
        "C" => ["C3", "G3", "En4", "C5"],
        "Cm" => ["C3", "G3", "Eb4", "C5"],
        "Eb" => ["Eb3", "Bb3", "G4", "Eb5"],
        "D" => ["D3", "A3", "F#4", "D5"],
        "G" => ["G2", "B3", "G4", "D5"],
        "Em" => ["E3", "B3", "G4", "E5"],
        "Bm" => ["B2", "F#3", "D4", "B4"],
        "F#m" => ["F#2", "C#4", "A4", "F#5"],
        _ => panic!("unknown harmony: {name}"),
    }
}

const FIRST_A: [&str; 8] = [
    "r/q D4/e F4/e A4/q G4/e F4/e",
    "E4/e F4/e D4/q A3/q r/q",
    "G3/q Bb3/e D4/e G4/q F4/q",
    "E4/q C#4/e D4/e E4/h",
    "A4/q C5/e A4/e G4/q F4/q",
    "F4/e G4/e Bb4/q A4/e G4/e F4/q",
    "D4/q G4/q Bb4/e A4/e G4/e F4/e",
    "E4/q C#4/q A3/h",
];
const FIRST_B: [&str; 8] = [
    "A4/h G4/q F4/q",
    "G4/q E4/e F4/e G4/h",
    "F4/q A4/q D5/q C5/q",
    "Bb4/h A4/e G4/e F4/q",
    "G4/q D4/q Bb3/q D4/q",
    "F4/h E4/q D4/q",
    "C#4/q D4/e E4/e A4/q C#5/q",
    "D5/h A4/q F4/q",
];
const SLOW: [&str; 12] = [
    "Bb3/q D4/q F4/q",
    "A4/h G4/q",
    "F4/q Eb4/q D4/q",
    "C4/h.",
    "D4/h F4/q",
    "G4/q F4/e Eb4/e D4/q",
    "C4/q Bb3/q A3/q",
    "Bb3/h.",
    "A3/q D4/q F4/q",
    "En4/h D4/q",
    "C#4/q En4/q A4/q",
    "D4/h.",
];
const DANCE: [&str; 8] = [
    "D4/e F#4/e A4/e B4/q A4/e",
    "G4/e F#4/e E4/e F#4/q D4/e",
    "G4/e B4/e D5/e C#5/e B4/e A4/e",
    "G4/q E4/e C#4/q A3/e",
    "D4/e F#4/e A4/e D5/q C#5/e",
    "B4/e A4/e F#4/e E4/q D4/e",
    "E4/e G4/e B4/e A4/e G4/e E4/e",
    "C#4/q E4/e A3/q.",
];
const DANCE_B: [&str; 8] = [
    "B3/q D4/e F#4/q A4/e",
    "G4/q B4/e D5/q B4/e",
    "A4/e F#4/e D4/e F#4/q A4/e",
    "G4/e E4/e C#4/e A3/q.",
    "G3/e B3/e D4/e G4/q F#4/e",
    "E4/e F#4/e G4/e B4/q A4/e",
    "G4/e F#4/e E4/e C#4/e E4/e A4/e",
    "F#4/q E4/e D4/q.",
];

fn harmony(bar: usize) -> &'static str {
    if bar < 80 {
        match bar {
            22 | 23 | 55 | 63 | 78 => "A",
            24 | 25 | 79 => "Dm",
            26..=39 => ["F", "C", "Dm", "Bb", "Gm", "Dm", "A", "Dm"][(bar - 26) % 8],
            40..=55 => ["Gm", "C", "F", "Bb", "Gm", "Dm", "A7", "A"][(bar - 40) % 8],
            56..=62 => "A",
            76 => "Bb",
            77 => "Gm",
            _ => ["Dm", "Dm", "Gm", "A", "F", "Bb", "Gm", "A"][bar % 8],
        }
    } else if bar < 128 {
        let b = bar - 80;
        if b >= 44 {
            return ["Bb", "Eb", "F", "Bb"][b - 44];
        }
        if b < 4 {
            return ["Bb", "Gm", "Eb", "F"][b];
        }
        [
            "Bb", "F", "Gm", "Cm", "Bb", "Eb", "F", "Bb", "Dm", "A", "A", "Dm",
        ][(b - 4) % 12]
    } else {
        let b = bar - 128;
        match b {
            24..=39 | 56..=67 => ["Bm", "G", "D", "A", "G", "Em", "A7", "D"][b % 8],
            68..=71 => "A",
            80 => "A7",
            88..=95 => ["D", "G", "D", "A7", "G", "A", "D", "D"][b - 88],
            _ => ["D", "D", "G", "A", "D", "Bm", "Em", "A"][b % 8],
        }
    }
}

fn raised(mut events: Vec<Event>, steps: i16) -> Vec<Event> {
    for event in &mut events {
        event.pitches = Pitches::new(
            &event
                .pitches
                .iter()
                .map(|p| p.shift(steps).unwrap())
                .collect::<Vec<_>>(),
        )
        .unwrap();
    }
    events
}

fn violin_divisi(name: &str) -> (Pitch, Pitch) {
    let c = chord(name);
    let mut lower = pitch(c[1]);
    if lower.diatonic() < pitch("G3").diatonic() {
        lower = lower.shift(7).unwrap();
    }
    let mut upper = pitch(c[2]);
    if upper.diatonic() <= lower.diatonic() {
        upper = upper.shift(7).unwrap();
    }
    (upper, lower)
}

fn solo(bar: usize) -> Vec<Event> {
    match bar {
        0..=6 => phrase("r/w"),
        7 => phrase("r/h. A3/q"),
        8..=21 => {
            let mut events = phrase(FIRST_A[(bar - 8) % 8]);
            if bar >= 16 && bar.is_multiple_of(4) {
                events = raised(events, 7);
            }
            events
        }
        22 => phrase("E4/e F4/e G4/q E4/q C#4/q"),
        23 => phrase("A3/h r/h"),
        24 => phrase("F4/q E4/e D4/e A3/q C4/q"),
        25 => phrase("D4/h r/h"),
        26..=39 => phrase(FIRST_B[(bar - 26) % 8]),
        40..=55 => {
            let c = chord(harmony(bar));
            let a = pitch(c[1]);
            let b = pitch(c[2]);
            let notes = [
                a,
                a.shift(1).unwrap(),
                b,
                b.shift(1).unwrap(),
                b.shift(2).unwrap(),
                b,
                a.shift(1).unwrap(),
                a,
            ];
            notes
                .into_iter()
                .map(|p| Event {
                    pitches: Pitches::single(Some(p)),
                    duration: Duration::Eighth,
                    notation: Notation::default(),
                })
                .collect()
        }
        56 => phrase("D4/q F4/e A4/e D5/q C5/e Bb4/e"),
        57 => phrase("A4/et Bb4/et A4/et G4/et A4/et G4/et F4/et G4/et F4/et E4/et F4/et E4/et"),
        58 => phrase("D4/s E4/s F4/s G4/s A4/s Bb4/s C#5/s D5/s E5/e D5/e C#5/e A4/e"),
        59 => phrase("D5/h r/q A4/q"),
        60 => phrase("D4/h F4/h"),
        61 => phrase("G4/q F4/q E4/q D4/q"),
        62 => phrase("C#4/q E4/e G4/e Bb4/q C#5/q"),
        63 => phrase("A4/h r/h"),
        64..=71 => phrase(FIRST_A[(bar - 64) % 8]),
        72..=75 => raised(phrase(FIRST_A[(bar - 64) % 8]), 7),
        76 => phrase("D5/q Bb4/e A4/e F4/q D4/q"),
        77 => phrase("G4/q Bb4/q D5/q Bb4/q"),
        78 => phrase("A4/e G4/e E4/q C#4/q A3/q"),
        79 => phrase("D4/w"),
        80..=83 => phrase("r/h."),
        84..=99 => phrase(SLOW[(bar - 84) % 12]),
        100..=103 => phrase(["D4/h.", "G3/h.", "A3/h.", "D4/h."][bar - 100]),
        104..=107 => phrase(SLOW[(bar - 84) % 12]),
        108..=119 => raised(phrase(SLOW[(bar - 108) % 12]), 7),
        120 => phrase("D6/h."),
        121 => phrase("A5/h."),
        122 => phrase("D6/h."),
        123 => phrase("G5/h."),
        124 => phrase("F4/q D4/q Bb3/q"),
        125 => phrase("G4/h Eb4/q"),
        126 => phrase("C4/q A3/q C4/q"),
        127 => phrase("Bb3/h."),
        128..=134 => phrase("r/h."),
        135 => phrase("r/q. A3/e B3/e C#4/e"),
        136..=151 => phrase(DANCE[(bar - 136) % 8]),
        152..=167 => phrase(DANCE_B[(bar - 152) % 8]),
        168..=183 => {
            let p = phrase(DANCE[(bar - 168) % 8]);
            if bar >= 176 { raised(p, 7) } else { p }
        }
        184..=195 => phrase(DANCE_B[(bar - 184) % 8]),
        196 => phrase("A3/e C#4/e E4/e G4/e Bb4/e C#5/e"),
        197 => phrase("D5/s C#5/s B4/s A4/s G4/s F#4/s E4/s D4/s C#4/s B3/s A3/s G3/s"),
        198 => phrase("D4/q. A3/q."),
        199 => phrase("C#4/q E4/e A4/q."),
        200..=207 => phrase(DANCE[(bar - 200) % 8]),
        208 => phrase("C#4/q E4/e A3/q."),
        209..=215 => raised(phrase(DANCE[(bar - 208) % 8]), 7),
        216 => phrase("D5/e A4/e F#4/e D4/q A3/e"),
        217 => phrase("G4/e B4/e D5/e B4/q G4/e"),
        218 => phrase("F#4/e A4/e D5/e F#5/q E5/e"),
        219 => phrase("C#5/q E5/e A4/q G4/e"),
        220 => phrase("B4/e D5/e G5/e F#5/e E5/e D5/e"),
        221 => phrase("C#5/e B4/e A4/e G4/e E4/e C#4/e"),
        222 => phrase("D4+A4+D5/q. r/e A4/e C#5/e"),
        223 => phrase("D4+A4+F#5/h."),
        _ => unreachable!(),
    }
}

fn desk(part: usize, bar: usize) -> Vec<Event> {
    let c = chord(harmony(bar));
    let value = if bar >= 80 { "h." } else { "w" };
    let rest = || phrase(&format!("r/{value}"));
    if (56..63).contains(&bar) || (196..199).contains(&bar) {
        return rest();
    }
    if (8..12).contains(&bar) && part == 1 {
        return rest();
    }
    if (84..88).contains(&bar) && part == 1 {
        return rest();
    }
    if part == 1 && bar == 80 {
        return phrase("D5/h.");
    }
    if part == 1 && (bar < 8 || (128..136).contains(&bar)) {
        return raised(
            phrase(if bar < 8 {
                FIRST_A[bar]
            } else {
                DANCE[(bar - 128) % 8]
            }),
            7,
        );
    }
    let p = c[match part {
        1 => 3,
        2 => 2,
        3 => 1,
        4 | 5 => 0,
        _ => unreachable!(),
    }];
    if bar == 79 || bar == 127 || bar == 223 {
        return phrase(&format!("{p}/{value}"));
    }
    if (80..128).contains(&bar) {
        return match part {
            1 => phrase(&format!("{p}/h.")),
            2 => {
                let (upper, lower) = violin_divisi(harmony(bar));
                [(upper, Duration::Half), (lower, Duration::Quarter)]
                    .into_iter()
                    .map(|(p, duration)| Event {
                        pitches: Pitches::single(Some(p)),
                        duration,
                        notation: Notation::default(),
                    })
                    .collect()
            }
            3 => phrase(&format!("{p}/q r/q {p}/q")),
            4 if (100..108).contains(&bar) => raised(phrase(SLOW[(bar - 84) % 12]), -7),
            4 => phrase(&format!("{p}/h r/q")),
            5 => phrase(&format!("{p}/q r/h")),
            _ => unreachable!(),
        };
    }
    if bar >= 128 {
        return match part {
            1 if (184..196).contains(&bar) => raised(phrase(DANCE_B[(bar - 184) % 8]), 7),
            1 => phrase(&format!("{p}/q. r/q.")),
            2 => phrase(&format!("r/e {p}/e {p}/e r/e {p}/e {p}/e")),
            3 => phrase(&format!("{p}/q. {}/q.", c[2])),
            4 => phrase(&format!("{p}/q r/e {p}/q r/e")),
            5 => phrase(&format!("{p}/q. r/q.")),
            _ => unreachable!(),
        };
    }
    match part {
        1 => phrase(&format!("{p}/h r/q {}/q", c[2])),
        2 if (40..56).contains(&bar) => phrase(&format!("{p}/w")),
        2 => phrase(&format!("r/e {p}/e r/e {p}/e r/e {p}/e r/e {p}/e")),
        3 => phrase(&format!("{p}/h {}/h", c[2])),
        4 if (40..56).contains(&bar) => phrase(&format!("{p}/q {}/q {p}/q {}/q", c[1], c[1])),
        4 => phrase(&format!("{p}/q r/q {}/q r/q", c[1])),
        5 => phrase(&format!("{p}/h r/h")),
        _ => unreachable!(),
    }
}

fn dynamic(part: usize, bar: usize) -> Option<Dynamic> {
    use Dynamic::*;
    let solo = part == 0;
    match bar {
        0 | 128 => Some(Mf),
        7 | 135 if solo => Some(Mp),
        8 | 136 => Some(if solo { Mf } else { P }),
        26 => Some(if solo { Mp } else { P }),
        40 | 168 => Some(if solo { F } else { Mp }),
        48 | 76 | 176 | 216 => Some(F),
        56 | 80 => Some(P),
        60 if solo => Some(Pp),
        64 | 200 => Some(if solo { F } else { Mp }),
        84 => Some(if solo { Mp } else { Pp }),
        96 | 108 => Some(if solo { Mf } else { P }),
        100 => Some(Pp),
        116 => Some(Mp),
        120 => Some(Ppp),
        124 => Some(Pp),
        152 | 184 => Some(if solo { Mp } else { P }),
        196 if solo => Some(Mf),
        209 => Some(F),
        222 => Some(Ff),
        _ => None,
    }
}

fn main() -> Result<()> {
    let output = std::env::args_os()
        .nth(1)
        .unwrap_or_else(|| "examples/umbral-y-lumbre-concierto-viola.partitura".into());
    let mut score = Score::new(
        "Umbral y lumbre".into(),
        &[
            Instrument::Viola,
            Instrument::Violin,
            Instrument::Violin,
            Instrument::Viola,
            Instrument::Cello,
            Instrument::DoubleBass,
        ],
        BARS,
    )?;
    let mut data = serde_json::to_value(&score)?;
    for (part, name) in data["parts"].as_array_mut().unwrap().iter_mut().zip(NAMES) {
        part["name"] = name.into();
    }
    score = serde_json::from_value(data)?;
    score.set_header(
        "Umbral y lumbre",
        &Credits {
            subtitle: "Concierto breve para viola y orquesta de cuerda en tres movimientos".into(),
            composer: "Composición original asistida por IA".into(),
            year: "2026".into(),
            ..Default::default()
        },
    )?;
    score.set_key_signature_at(0, KeySignature::new(-1, KeyMode::Minor).unwrap())?;
    score.set_time_signature_at(
        80,
        TimeSignature {
            numerator: 3,
            denominator: 4,
        },
    )?;
    score.set_key_signature_at(80, KeySignature::new(-2, KeyMode::Major).unwrap())?;
    score.set_time_signature_at(
        128,
        TimeSignature {
            numerator: 6,
            denominator: 8,
        },
    )?;
    score.set_key_signature_at(128, KeySignature::new(2, KeyMode::Major).unwrap())?;
    for (bar, bpm, beat, text) in [
        (0, 104, TempoBeat::Quarter, "I. Allegro moderato - Ombra"),
        (26, 96, TempoBeat::Quarter, "Cantabile"),
        (40, 112, TempoBeat::Quarter, "Agitato"),
        (56, 66, TempoBeat::Quarter, "Cadenza scritta, rubato"),
        (64, 104, TempoBeat::Quarter, "Tempo I"),
        (78, 80, TempoBeat::Quarter, "Allargando"),
        (80, 56, TempoBeat::Quarter, "II. Andante espressivo - Vetro"),
        (108, 62, TempoBeat::Quarter, "Poco animato"),
        (120, 50, TempoBeat::Quarter, "Come da lontano"),
        (128, 88, TempoBeat::DottedQuarter, "III. Vivo - Lumbre"),
        (184, 76, TempoBeat::DottedQuarter, "Meno mosso, cantabile"),
        (196, 64, TempoBeat::DottedQuarter, "Quasi cadenza"),
        (200, 88, TempoBeat::DottedQuarter, "Tempo I"),
        (209, 96, TempoBeat::DottedQuarter, "Coda - con slancio"),
        (222, 80, TempoBeat::DottedQuarter, "Largamente"),
    ] {
        score.set_tempo_at(
            bar,
            TempoMark {
                bpm,
                beat,
                text: text.into(),
            },
        )?;
    }
    let mut ids = vec![vec![Vec::new(); BARS]; 6];
    for (part, part_ids) in ids.iter_mut().enumerate() {
        for (bar, bar_ids) in part_ids.iter_mut().enumerate() {
            let mut events = if part == 0 {
                solo(bar)
            } else {
                desk(part, bar)
            };
            assert_eq!(
                events.iter().map(|e| e.duration.ticks()).sum::<u16>(),
                score.time_at(bar).capacity(),
                "{part}:{bar}"
            );
            let first_note = events.iter().position(|e| !e.pitches.is_empty());
            let last_event = events.len() - 1;
            for (index, event) in events.iter_mut().enumerate() {
                if Some(index) == first_note {
                    event.notation.dynamic = dynamic(part, bar);
                    event.notation.technique = match (part, bar) {
                        (1..=5, 0 | 40 | 64 | 108 | 128 | 168 | 200 | 209) | (0, 0 | 64 | 128) => {
                            Some(PlayingTechnique::Arco)
                        }
                        (3..=5, 26 | 80 | 152 | 184) => Some(PlayingTechnique::Pizzicato),
                        _ => None,
                    };
                }
                if !event.pitches.is_empty() {
                    if part == 0 && (100..104).contains(&bar) {
                        event.notation.harmonic = Some(Harmonic::Artificial);
                    }
                    if part == 0 && ((60..62).contains(&bar) || (120..124).contains(&bar)) {
                        if bar < 80 {
                            // Written at sounding pitch: D/A open-string partials.
                            event.pitches =
                                Pitches::single(Some(pitch(if bar == 60 { "D6" } else { "A5" })));
                        }
                        event.notation.harmonic = Some(Harmonic::Natural);
                    }
                    if part == 2 && ((40..56).contains(&bar) || (116..120).contains(&bar)) {
                        event.notation.tremolo = Some(if bar % 2 == 0 {
                            Tremolo::Two
                        } else {
                            Tremolo::Three
                        });
                    }
                    if part == 0 && (196..198).contains(&bar) {
                        event.notation.articulation = Some(Articulation::Marcato);
                    }
                    if part > 0 && bar >= 128 && event.duration == Duration::Eighth {
                        event.notation.articulation = Some(Articulation::Staccato);
                    }
                }
                // Shared cadential holds align every desk, including resting players.
                if ([79, 127, 223, 55, 63, 199].contains(&bar) && index == last_event)
                    || (part == 0 && bar == 59 && Some(index) == first_note)
                {
                    event.notation.fermata = true;
                }
                event.pitches = signature_pitches(event.pitches, score.key_signature_at(bar));
                let id =
                    score.insert_on(PartId(part as u32), StaffId::Upper, bar, index, *event)?;
                bar_ids.push(id);
            }
            if part == 0 || (part == 1 && (bar < 8 || (128..136).contains(&bar))) {
                // Slur short groups rather than entire long runs; rests break phrases.
                let mut group = Vec::new();
                for &id in bar_ids.iter() {
                    if score.event(id).unwrap().pitches.is_empty() {
                        slur(&mut score, &group)?;
                        group.clear();
                    } else {
                        group.push(id);
                        if group.len() == 3 {
                            slur(&mut score, &group)?;
                            group.clear();
                        }
                    }
                }
                slur(&mut score, &group)?;
            }
        }
    }
    // Real two-voice divisi in the slow introduction and accompanying first period.
    for bar in 80..96 {
        let (_, lower) = violin_divisi(harmony(bar));
        score.insert_in_voice(
            PartId(2),
            StaffId::Upper,
            bar,
            Voice::Two,
            0,
            Event {
                pitches: signature_pitches(
                    Pitches::single(Some(lower)),
                    score.key_signature_at(bar),
                ),
                duration: Duration::Half.with_dotted(true),
                notation: Notation {
                    dynamic: (bar == 80).then_some(Dynamic::Pp),
                    ..Default::default()
                },
            },
        )?;
    }
    score.set_passage_divisi(PartId(2), 80, Some(95), true)?;
    score.add_connection(Connection {
        start: ids[1][80][0],
        end: ids[1][81][0],
        kind: ConnectionKind::Tie,
    })?;
    for (start, end, kind) in [
        (8, 11, ConnectionKind::Crescendo),
        (12, 15, ConnectionKind::Diminuendo),
        (26, 29, ConnectionKind::Crescendo),
        (40, 43, ConnectionKind::Crescendo),
        (48, 51, ConnectionKind::Diminuendo),
        (64, 67, ConnectionKind::Crescendo),
        (84, 87, ConnectionKind::Crescendo),
        (88, 91, ConnectionKind::Diminuendo),
        (108, 111, ConnectionKind::Crescendo),
        (116, 119, ConnectionKind::Diminuendo),
        (136, 139, ConnectionKind::Crescendo),
        (152, 155, ConnectionKind::Diminuendo),
        (200, 203, ConnectionKind::Crescendo),
        (209, 212, ConnectionKind::Crescendo),
    ] {
        let start = ids[0][start]
            .iter()
            .copied()
            .find(|id| !score.event(*id).unwrap().pitches.is_empty())
            .unwrap();
        let end = ids[0][end]
            .iter()
            .copied()
            .rev()
            .find(|id| !score.event(*id).unwrap().pitches.is_empty())
            .unwrap();
        score.add_connection(Connection { start, end, kind })?;
    }
    score.set_bar_form(
        8,
        BarForm {
            repeat_start: true,
            ..Default::default()
        },
    )?;
    score.set_bar_form(
        23,
        BarForm {
            repeat_end: Some(2),
            ..Default::default()
        },
    )?;
    score.set_ending(Ending {
        start: 22,
        end: 23,
        numbers: vec![1],
    })?;
    score.set_ending(Ending {
        start: 24,
        end: 25,
        numbers: vec![2],
    })?;
    for bar in [79, 127, 223] {
        score.set_bar_form(
            bar,
            BarForm {
                barline: Barline::Final,
                ..Default::default()
            },
        )?;
    }
    score.set_bar_form(
        136,
        BarForm {
            segno: true,
            ..Default::default()
        },
    )?;
    score.set_bar_form(
        151,
        BarForm {
            to_coda: true,
            ..Default::default()
        },
    )?;
    score.set_bar_form(
        208,
        BarForm {
            jump: Some(Jump::DalSegnoAlCoda),
            ..Default::default()
        },
    )?;
    score.set_bar_form(
        209,
        BarForm {
            coda: true,
            ..Default::default()
        },
    )?;
    for (bar, label) in [
        (0, "I - Ombra"),
        (8, "A"),
        (26, "B"),
        (40, "C"),
        (56, "Cadenza"),
        (64, "D"),
        (80, "II - Vetro"),
        (84, "E"),
        (100, "F - armonici"),
        (108, "G"),
        (120, "H"),
        (128, "III - Lumbre"),
        (136, "J"),
        (152, "K"),
        (168, "L"),
        (184, "M"),
        (196, "Quasi cadenza"),
        (200, "N"),
    ] {
        score.set_rehearsal_custom(bar, Some(label.into()))?;
    }
    score.validate()?;
    for part in score.parts() {
        let lowest = match part.instrument() {
            Instrument::Violin => pitch("G3"),
            Instrument::Viola => pitch("C3"),
            Instrument::Cello => pitch("C2"),
            Instrument::DoubleBass => pitch("E2"),
            _ => unreachable!(),
        };
        for (bar, measure) in part.measures().iter().enumerate() {
            assert_eq!(
                measure
                    .voice_iter(Voice::One)
                    .map(|(_, e)| e.duration.ticks())
                    .sum::<u16>(),
                score.time_at(bar).capacity()
            );
            for (_, event) in measure.iter() {
                for p in event.pitches.iter() {
                    assert!(
                        p.diatonic() >= lowest.diatonic(),
                        "{}: compás {}, {p:?}",
                        score.part_name(part.id()),
                        bar + 1
                    );
                }
            }
        }
    }
    score_io::save(Path::new(&output), &score)?;
    assert_eq!(score_io::load(Path::new(&output))?, score);
    let order = score.play_order(true);
    let seconds: f64 = order
        .iter()
        .map(|played| {
            let mark = score.tempo_at(played.bar);
            f64::from(score.time_at(played.bar).capacity()) / f64::from(mark.beat.ticks()) * 60.
                / f64::from(mark.bpm)
        })
        .sum();
    println!(
        "{}: {} compases escritos, {} interpretados, {:.1} minutos antes de calderones",
        Path::new(&output).display(),
        BARS,
        order.len(),
        seconds / 60.
    );
    Ok(())
}

fn slur(score: &mut Score, ids: &[ElementId]) -> Result<()> {
    if ids.len() > 1 {
        score.add_connection(Connection {
            start: ids[0],
            end: *ids.last().unwrap(),
            kind: ConnectionKind::Slur,
        })?;
    }
    Ok(())
}
