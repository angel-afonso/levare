//! Generate a short orchestral fixture with written parts and concert-key changes.
use score_core::*;
fn main() {
    let output = std::env::args_os()
        .nth(1)
        .expect("usage: orchestra output.partitura");
    let mut score = Score::new(
        "Orquesta completa — estudio de transposición".into(),
        &Instrument::ORCHESTRA,
        4,
    )
    .unwrap();
    score
        .set_tempo_at(
            0,
            TempoMark {
                bpm: 96,
                text: "Andante maestoso".into(),
                ..Default::default()
            },
        )
        .unwrap();
    score
        .set_key_signature_at(2, KeySignature::new(1, KeyMode::Major).unwrap())
        .unwrap();
    let parts: Vec<_> = score.parts().iter().map(|p| p.id()).collect();
    for part in parts {
        let instrument = score.part(part).unwrap().instrument();
        let octave = match instrument {
            Instrument::Tuba | Instrument::DoubleBass | Instrument::Timpani => 2,
            Instrument::Bassoon
            | Instrument::BassTrombone
            | Instrument::Trombone
            | Instrument::Cello => 3,
            Instrument::Violin | Instrument::Flute | Instrument::Harp => 5,
            Instrument::Piccolo => 6,
            Instrument::Contrabassoon => 2,
            Instrument::BassClarinet => 3,
            _ => 4,
        };
        for bar in 0..4 {
            let steps = match bar {
                0 => [Step::C, Step::E, Step::G, Step::C],
                1 => [Step::G, Step::B, Step::D, Step::G],
                2 => [Step::D, Step::F, Step::A, Step::D],
                _ => [Step::G, Step::D, Step::B, Step::G],
            };
            for (index, step) in steps.into_iter().enumerate() {
                let pitches = if instrument.percussion_key().is_some() {
                    Pitches::single(Pitch::new(Step::B, 4))
                } else {
                    score
                        .part(part)
                        .unwrap()
                        .transposition()
                        .inverse()
                        .pitches(
                            Pitches::single(Pitch::new(step, octave)),
                            score.key_signature_at(bar),
                        )
                        .unwrap()
                };
                let id = score
                    .insert_on(
                        part,
                        StaffId::Upper,
                        bar,
                        index,
                        Event {
                            pitches,
                            duration: Duration::Quarter,
                            notation: Notation::default(),
                        },
                    )
                    .unwrap();
                if index == 0 && (bar == 0 || bar == 2) {
                    score
                        .set_dynamic(id, Some(if bar == 0 { Dynamic::Mp } else { Dynamic::F }))
                        .unwrap();
                }
                if instrument.family() == InstrumentFamily::Strings && bar == 0 && index == 0 {
                    score
                        .set_technique(id, Some(PlayingTechnique::Arco))
                        .unwrap();
                }
            }
            if instrument.is_grand_staff() {
                let pitches = Pitches::new(&[
                    Pitch::new(Step::C, 3).unwrap(),
                    Pitch::new(Step::E, 3).unwrap(),
                    Pitch::new(Step::G, 3).unwrap(),
                ])
                .unwrap();
                score
                    .insert_on(
                        part,
                        StaffId::Lower,
                        bar,
                        0,
                        Event {
                            pitches,
                            duration: Duration::Whole,
                            notation: Notation::default(),
                        },
                    )
                    .unwrap();
            }
        }
    }
    score.validate().unwrap();
    score_io::save(std::path::Path::new(&output), &score).unwrap();
    println!(
        "{} partes, {} compases",
        score.parts().len(),
        score.measures().len()
    );
}
