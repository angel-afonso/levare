use crate::Transposition;
use serde::{Deserialize, Serialize};

/// Presentation groups shared by the score, sidebar and engraver.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum InstrumentFamily {
    Woodwinds,
    Brass,
    Percussion,
    Keyboards,
    PluckedStrings,
    Strings,
}
impl InstrumentFamily {
    pub const ALL: [Self; 6] = [
        Self::Woodwinds,
        Self::Brass,
        Self::Percussion,
        Self::Keyboards,
        Self::PluckedStrings,
        Self::Strings,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::Woodwinds => "MADERAS",
            Self::Brass => "METALES",
            Self::Percussion => "PERCUSIÓN",
            Self::Keyboards => "TECLADOS",
            Self::PluckedStrings => "CUERDAS PULSADAS",
            Self::Strings => "CUERDAS",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Clef {
    Treble,
    Alto,
    Bass,
    Percussion,
}
impl Clef {
    pub fn bottom_diatonic(self) -> i16 {
        match self {
            Self::Treble | Self::Percussion => 30,
            Self::Alto => 24,
            Self::Bass => 18,
        }
    }
}

macro_rules! catalog {
    ($( $variant:ident, $family:ident, $name:literal, $program:literal, $clef:ident, $order:literal, $diatonic:expr, $chromatic:expr, $drum:expr, $grand:literal; )*) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
        pub enum Instrument { $( $variant, )* }
        impl Instrument {
            pub const ALL: [Self; catalog!(@count $( $variant )*)] = [$( Self::$variant, )*];
            pub fn family(self) -> InstrumentFamily { match self { $( Self::$variant => InstrumentFamily::$family, )* } }
            pub fn name(self) -> &'static str { match self { $( Self::$variant => $name, )* } }
            pub fn program(self) -> u8 { match self { $( Self::$variant => $program, )* } }
            pub fn clef(self) -> Clef { match self { $( Self::$variant => Clef::$clef, )* } }
            pub fn score_order(self) -> u8 { match self { $( Self::$variant => $order, )* } }
            pub fn standard_transposition(self) -> Transposition { match self { $( Self::$variant => Transposition::new($diatonic, $chromatic), )* } }
            pub fn percussion_key(self) -> Option<u8> { match self { $( Self::$variant => $drum, )* } }
            pub fn is_grand_staff(self) -> bool { match self { $( Self::$variant => $grand, )* } }
        }
    };
    (@count $( $variant:ident )*) => { <[()]>::len(&[$( catalog!(@unit $variant), )*]) };
    (@unit $variant:ident) => { () };
}
catalog! {
    Piano, Keyboards, "Piano", 0, Treble, 44, 0, 0, None, true;
    Guitar, PluckedStrings, "Guitarra", 24, Treble, 48, -7, -12, None, false;
    Violin, Strings, "Violín", 40, Treble, 50, 0, 0, None, false;
    Viola, Strings, "Viola", 41, Alto, 51, 0, 0, None, false;
    Cello, Strings, "Violoncello", 42, Bass, 52, 0, 0, None, false;
    DoubleBass, Strings, "Contrabajo", 43, Bass, 53, -7, -12, None, false;
    Flute, Woodwinds, "Flauta", 73, Treble, 1, 0, 0, None, false;
    Oboe, Woodwinds, "Oboe", 68, Treble, 4, 0, 0, None, false;
    Clarinet, Woodwinds, "Clarinete en Do", 71, Treble, 10, 0, 0, None, false;
    Bassoon, Woodwinds, "Fagot", 70, Bass, 12, 0, 0, None, false;
    Trumpet, Brass, "Trompeta en Do", 56, Treble, 22, 0, 0, None, false;
    Horn, Brass, "Corno en Do", 60, Treble, 21, 0, 0, None, false;
    Trombone, Brass, "Trombón tenor", 57, Bass, 27, 0, 0, None, false;
    Piccolo, Woodwinds, "Piccolo", 72, Treble, 0, 7, 12, None, false;
    AltoFlute, Woodwinds, "Flauta alto en Sol", 73, Treble, 2, -3, -5, None, false;
    BassFlute, Woodwinds, "Flauta baja", 73, Treble, 3, -7, -12, None, false;
    OboeDAmore, Woodwinds, "Oboe de amor en La", 68, Treble, 5, -2, -3, None, false;
    EnglishHorn, Woodwinds, "Corno inglés en Fa", 69, Treble, 6, -4, -7, None, false;
    ClarinetBb, Woodwinds, "Clarinete en Si♭", 71, Treble, 8, -1, -2, None, false;
    ClarinetA, Woodwinds, "Clarinete en La", 71, Treble, 9, -2, -3, None, false;
    ClarinetEb, Woodwinds, "Clarinete en Mi♭", 71, Treble, 7, 2, 3, None, false;
    BassClarinet, Woodwinds, "Clarinete bajo en Si♭", 71, Treble, 11, -8, -14, None, false;
    Contrabassoon, Woodwinds, "Contrafagot", 70, Bass, 13, -7, -12, None, false;
    SopranoSax, Woodwinds, "Saxofón soprano en Si♭", 64, Treble, 14, -1, -2, None, false;
    AltoSax, Woodwinds, "Saxofón alto en Mi♭", 65, Treble, 15, -5, -9, None, false;
    TenorSax, Woodwinds, "Saxofón tenor en Si♭", 66, Treble, 16, -8, -14, None, false;
    BaritoneSax, Woodwinds, "Saxofón barítono en Mi♭", 67, Treble, 17, -12, -21, None, false;
    HornF, Brass, "Corno en Fa", 60, Treble, 20, -4, -7, None, false;
    TrumpetBb, Brass, "Trompeta en Si♭", 56, Treble, 23, -1, -2, None, false;
    TrumpetD, Brass, "Trompeta en Re", 56, Treble, 24, 1, 2, None, false;
    PiccoloTrumpet, Brass, "Trompeta piccolo en Si♭", 56, Treble, 25, 6, 10, None, false;
    Cornet, Brass, "Corneta en Si♭", 56, Treble, 26, -1, -2, None, false;
    BassTrombone, Brass, "Trombón bajo", 57, Bass, 28, 0, 0, None, false;
    Tuba, Brass, "Tuba", 58, Bass, 29, 0, 0, None, false;
    Timpani, Percussion, "Timbales", 47, Bass, 30, 0, 0, None, false;
    SnareDrum, Percussion, "Caja", 0, Percussion, 31, 0, 0, Some(38), false;
    BassDrum, Percussion, "Bombo", 0, Percussion, 32, 0, 0, Some(36), false;
    Cymbals, Percussion, "Platillos", 0, Percussion, 33, 0, 0, Some(49), false;
    Triangle, Percussion, "Triángulo", 0, Percussion, 34, 0, 0, Some(81), false;
    Tambourine, Percussion, "Pandereta", 0, Percussion, 35, 0, 0, Some(54), false;
    TamTam, Percussion, "Tam-tam", 0, Percussion, 36, 0, 0, Some(52), false;
    Glockenspiel, Percussion, "Glockenspiel", 9, Treble, 37, 14, 24, None, false;
    Xylophone, Percussion, "Xilófono", 13, Treble, 38, 7, 12, None, false;
    Marimba, Percussion, "Marimba", 12, Treble, 39, 0, 0, None, true;
    Vibraphone, Percussion, "Vibráfono", 11, Treble, 40, 0, 0, None, true;
    TubularBells, Percussion, "Campanas tubulares", 14, Treble, 41, 0, 0, None, false;
    Celesta, Keyboards, "Celesta", 8, Treble, 45, 7, 12, None, true;
    Organ, Keyboards, "Órgano", 19, Treble, 46, 0, 0, None, true;
    Harp, PluckedStrings, "Arpa", 46, Treble, 47, 0, 0, None, true;
}
impl Instrument {
    /// Common symphonic desk configuration, with auxiliary winds and two violin sections.
    pub const ORCHESTRA: [Self; 34] = [
        Self::Piccolo,
        Self::Flute,
        Self::Flute,
        Self::Oboe,
        Self::Oboe,
        Self::EnglishHorn,
        Self::ClarinetBb,
        Self::ClarinetBb,
        Self::BassClarinet,
        Self::Bassoon,
        Self::Bassoon,
        Self::Contrabassoon,
        Self::HornF,
        Self::HornF,
        Self::HornF,
        Self::HornF,
        Self::TrumpetBb,
        Self::TrumpetBb,
        Self::TrumpetBb,
        Self::Trombone,
        Self::Trombone,
        Self::BassTrombone,
        Self::Tuba,
        Self::Timpani,
        Self::SnareDrum,
        Self::BassDrum,
        Self::Cymbals,
        Self::Triangle,
        Self::Harp,
        Self::Violin,
        Self::Violin,
        Self::Viola,
        Self::Cello,
        Self::DoubleBass,
    ];
    pub fn entry_octave(self) -> u8 {
        if self.clef() == Clef::Bass { 3 } else { 4 }
    }
}

impl crate::Score {
    /// Orchestral name shared by the editor and engraving. Repeated instruments
    /// get Roman numerals in their current score order, independently of divisi.
    pub fn part_name(&self, id: crate::PartId) -> String {
        let Some(part) = self.part(id) else {
            return String::new();
        };
        if let Some(name) = &part.name {
            return name.clone();
        }
        let matches: Vec<_> = self
            .parts()
            .iter()
            .filter(|p| p.instrument() == part.instrument())
            .map(|p| p.id())
            .collect();
        if matches.len() > 1 {
            let index = matches.iter().position(|part| *part == id).unwrap();
            format!("{} {}", part.instrument().name(), roman(index + 1))
        } else {
            part.instrument().name().to_owned()
        }
    }
}

fn roman(mut value: usize) -> String {
    let mut result = String::new();
    for (n, text) in [
        (50, "L"),
        (40, "XL"),
        (10, "X"),
        (9, "IX"),
        (5, "V"),
        (4, "IV"),
        (1, "I"),
    ] {
        while value >= n {
            result.push_str(text);
            value -= n;
        }
    }
    result
}
