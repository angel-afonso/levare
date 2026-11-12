//! Ensembles a new score can start from. Each one is a list of catalog
//! instruments; clefs and transpositions come from the catalog and the order
//! from `Score::arrange_parts`, as when the musician adds them by hand.

use crate::{Instrument, Score, ScoreError};

/// A new score's line-up, by name.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Template {
    pub name: &'static str,
    /// What it holds, in a few words, for the list it is chosen from.
    pub detail: &'static str,
    pub instruments: &'static [Instrument],
}

use Instrument::*;

/// From the smallest to the largest. The empty score, Archivo › Nueva
/// partitura, is not among them: it is the default `Score`.
pub const TEMPLATES: &[Template] = &[
    Template {
        name: "Piano",
        detail: "Dos pentagramas, mano derecha e izquierda",
        instruments: &[Piano],
    },
    Template {
        name: "Melodía y cifrado",
        detail: "Una línea en clave de sol para la melodía y los acordes",
        instruments: &[Voice],
    },
    Template {
        name: "Voz y piano",
        detail: "Voz y piano a dos manos",
        instruments: &[Voice, Piano],
    },
    Template {
        name: "Coro mixto",
        detail: "Soprano, contralto, tenor y bajo",
        instruments: &[Soprano, Contralto, Tenor, BassVoice],
    },
    Template {
        name: "Cuarteto de cuerda",
        detail: "Dos violines, viola y violoncello",
        instruments: &[Violin, Violin, Viola, Cello],
    },
    Template {
        name: "Quinteto de viento",
        detail: "Flauta, oboe, clarinete en Si♭, fagot y corno en Fa",
        instruments: &[Flute, Oboe, ClarinetBb, HornF, Bassoon],
    },
    Template {
        name: "Quinteto de metales",
        detail: "Dos trompetas en Si♭, corno en Fa, trombón y tuba",
        instruments: &[TrumpetBb, TrumpetBb, HornF, Trombone, Tuba],
    },
    Template {
        name: "Orquesta de cuerda",
        detail: "Violines I y II, viola, violoncello y contrabajo",
        instruments: &[Violin, Violin, Viola, Cello, DoubleBass],
    },
    Template {
        name: "Orquesta de cámara",
        detail: "Maderas y metales a dos, timbales y cuerda",
        instruments: &[
            Flute, Flute, Oboe, Oboe, ClarinetBb, ClarinetBb, Bassoon, Bassoon, HornF, HornF,
            TrumpetBb, TrumpetBb, Timpani, Violin, Violin, Viola, Cello, DoubleBass,
        ],
    },
    Template {
        name: "Orquesta sinfónica",
        detail: "Maderas a dos con auxiliares, cuatro cornos, percusión, arpa y cuerda",
        instruments: &Instrument::ORCHESTRA,
    },
    Template {
        name: "Banda sinfónica",
        detail: "Maderas, saxofones, metales y percusión",
        instruments: &[
            Piccolo,
            Flute,
            Flute,
            Oboe,
            Bassoon,
            ClarinetEb,
            ClarinetBb,
            ClarinetBb,
            ClarinetBb,
            BassClarinet,
            AltoSax,
            AltoSax,
            TenorSax,
            BaritoneSax,
            HornF,
            HornF,
            HornF,
            HornF,
            TrumpetBb,
            TrumpetBb,
            TrumpetBb,
            Trombone,
            Trombone,
            BassTrombone,
            Tuba,
            Timpani,
            SnareDrum,
            BassDrum,
            Cymbals,
            Glockenspiel,
        ],
    },
];

impl Template {
    /// An untitled score with these parts in score order and as many bars
    /// as a new score has.
    pub fn score(&self) -> Result<Score, ScoreError> {
        let measures = Score::default().measures().len();
        let mut score = Score::new(
            Score::default().title().to_owned(),
            self.instruments,
            measures,
        )?;
        score.arrange_parts();
        Ok(score)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Clef, StaffId, Transposition};

    fn template(name: &str) -> &'static Template {
        TEMPLATES
            .iter()
            .find(|template| template.name == name)
            .unwrap_or_else(|| panic!("no template «{name}»"))
    }

    /// Name, clef of each staff and transposition (diatonic, chromatic) of
    /// every part, top to bottom.
    fn line_up(name: &str) -> Vec<(String, Vec<Clef>, (i8, i8))> {
        let score = template(name).score().unwrap();
        score
            .parts()
            .iter()
            .map(|part| {
                let Transposition {
                    diatonic,
                    chromatic,
                } = part.transposition();
                (
                    score.part_name(part.id()),
                    part.staves()
                        .iter()
                        .map(|staff| part.clef(staff.id))
                        .collect(),
                    (diatonic, chromatic),
                )
            })
            .collect()
    }

    fn row(name: &str, clefs: &[Clef], transposition: (i8, i8)) -> (String, Vec<Clef>, (i8, i8)) {
        (name.to_owned(), clefs.to_vec(), transposition)
    }

    use Clef::{Alto, Bass, Percussion, Treble};
    const C: (i8, i8) = (0, 0);

    #[test]
    fn every_template_builds_a_valid_score_in_score_order() {
        let empty = Score::default();
        for template in TEMPLATES {
            let score = template.score().unwrap();
            score.validate().unwrap();
            assert_eq!(
                score.parts().len(),
                template.instruments.len(),
                "{}",
                template.name
            );
            assert_eq!(
                score.measures().len(),
                empty.measures().len(),
                "{}",
                template.name
            );
            assert_eq!(score.title(), empty.title());
            assert_eq!(score.time(), empty.time());
            assert_eq!(score.key_signature(), empty.key_signature());
            assert!(
                score
                    .parts()
                    .iter()
                    .map(|part| part.instrument().score_order())
                    .is_sorted(),
                "{} is in score order",
                template.name
            );
            assert!(!template.detail.is_empty());
        }
        let names: Vec<_> = TEMPLATES.iter().map(|template| template.name).collect();
        let mut unique = names.clone();
        unique.sort_unstable();
        unique.dedup();
        assert_eq!(unique.len(), names.len(), "names are unique");
    }

    #[test]
    fn the_piano_template_is_the_new_score() {
        assert_eq!(template("Piano").score().unwrap(), Score::default());
    }

    #[test]
    fn piano_lead_sheet_and_voices() {
        assert_eq!(line_up("Piano"), [row("Piano", &[Treble, Bass], C)]);
        assert_eq!(line_up("Melodía y cifrado"), [row("Voz", &[Treble], C)]);
        // Score order puts keyboards above voices, as in an orchestral score.
        assert_eq!(
            line_up("Voz y piano"),
            [row("Piano", &[Treble, Bass], C), row("Voz", &[Treble], C)]
        );
        assert_eq!(
            line_up("Coro mixto"),
            [
                row("Soprano", &[Treble], C),
                row("Contralto", &[Treble], C),
                // Written an octave above its sound, as the catalog has it.
                row("Tenor", &[Treble], (-7, -12)),
                row("Bajo", &[Bass], C),
            ]
        );
    }

    #[test]
    fn chamber_ensembles() {
        assert_eq!(
            line_up("Cuarteto de cuerda"),
            [
                row("Violín I", &[Treble], C),
                row("Violín II", &[Treble], C),
                row("Viola", &[Alto], C),
                row("Violoncello", &[Bass], C),
            ]
        );
        assert_eq!(
            line_up("Quinteto de viento"),
            [
                row("Flauta", &[Treble], C),
                row("Oboe", &[Treble], C),
                row("Clarinete en Si♭", &[Treble], (-1, -2)),
                row("Fagot", &[Bass], C),
                row("Corno en Fa", &[Treble], (-4, -7)),
            ]
        );
        assert_eq!(
            line_up("Quinteto de metales"),
            [
                row("Corno en Fa", &[Treble], (-4, -7)),
                row("Trompeta en Si♭ I", &[Treble], (-1, -2)),
                row("Trompeta en Si♭ II", &[Treble], (-1, -2)),
                row("Trombón tenor", &[Bass], C),
                row("Tuba", &[Bass], C),
            ]
        );
        assert_eq!(
            line_up("Orquesta de cuerda"),
            [
                row("Violín I", &[Treble], C),
                row("Violín II", &[Treble], C),
                row("Viola", &[Alto], C),
                row("Violoncello", &[Bass], C),
                row("Contrabajo", &[Bass], (-7, -12)),
            ]
        );
    }

    #[test]
    fn orchestras() {
        let chamber = line_up("Orquesta de cámara");
        let names: Vec<_> = chamber.iter().map(|(name, ..)| name.as_str()).collect();
        assert_eq!(
            names,
            [
                "Flauta I",
                "Flauta II",
                "Oboe I",
                "Oboe II",
                "Clarinete en Si♭ I",
                "Clarinete en Si♭ II",
                "Fagot I",
                "Fagot II",
                "Corno en Fa I",
                "Corno en Fa II",
                "Trompeta en Si♭ I",
                "Trompeta en Si♭ II",
                "Timbales",
                "Violín I",
                "Violín II",
                "Viola",
                "Violoncello",
                "Contrabajo",
            ]
        );
        assert_eq!(chamber[4].2, (-1, -2));
        assert_eq!(chamber[8].2, (-4, -7));
        assert_eq!(chamber[12].1, [Bass]);
        let full = template("Orquesta sinfónica").score().unwrap();
        assert_eq!(full.parts().len(), Instrument::ORCHESTRA.len());
        let first = full.parts().front().unwrap();
        assert_eq!(first.instrument(), Instrument::Piccolo);
        assert_eq!(first.transposition(), Transposition::new(7, 12));
        assert_eq!(
            full.parts().back().unwrap().instrument(),
            Instrument::DoubleBass
        );
        let harp = full
            .parts()
            .iter()
            .find(|part| part.instrument() == Instrument::Harp)
            .unwrap();
        assert_eq!(harp.clef(StaffId::Upper), Treble);
        assert_eq!(harp.clef(StaffId::Lower), Bass);
    }

    #[test]
    fn concert_band() {
        let band = line_up("Banda sinfónica");
        let find = |name: &str| {
            band.iter()
                .find(|(part, ..)| part == name)
                .unwrap_or_else(|| panic!("no {name}"))
                .clone()
        };
        assert_eq!(band.first().unwrap().0, "Piccolo");
        assert_eq!(
            find("Clarinete en Mi♭"),
            row("Clarinete en Mi♭", &[Treble], (2, 3))
        );
        assert_eq!(
            find("Clarinete en Si♭ III"),
            row("Clarinete en Si♭ III", &[Treble], (-1, -2))
        );
        assert_eq!(
            find("Clarinete bajo en Si♭"),
            row("Clarinete bajo en Si♭", &[Treble], (-8, -14))
        );
        assert_eq!(
            find("Saxofón alto en Mi♭ II"),
            row("Saxofón alto en Mi♭ II", &[Treble], (-5, -9))
        );
        assert_eq!(
            find("Saxofón barítono en Mi♭"),
            row("Saxofón barítono en Mi♭", &[Treble], (-12, -21))
        );
        assert_eq!(
            find("Corno en Fa IV"),
            row("Corno en Fa IV", &[Treble], (-4, -7))
        );
        assert_eq!(find("Caja"), row("Caja", &[Percussion], C));
        assert_eq!(
            find("Glockenspiel"),
            row("Glockenspiel", &[Treble], (14, 24))
        );
        // Saxophones sit with the woodwinds, before the brass.
        let position = |name: &str| band.iter().position(|(part, ..)| part == name).unwrap();
        assert!(position("Saxofón barítono en Mi♭") < position("Corno en Fa I"));
        assert!(position("Tuba") < position("Timbales"));
    }
}
