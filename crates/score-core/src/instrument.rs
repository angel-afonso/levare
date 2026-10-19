use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Instrument {
    Piano,
    Guitar,
    Violin,
    Viola,
    Cello,
    DoubleBass,
    Flute,
    Oboe,
    Clarinet,
    Bassoon,
    Trumpet,
    Horn,
    Trombone,
}

/// Presentation groups shared by the score, sidebar and engraver.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum InstrumentFamily {
    Woodwinds,
    Brass,
    Keyboards,
    PluckedStrings,
    Strings,
}
impl InstrumentFamily {
    pub const ALL: [Self; 5] = [
        Self::Woodwinds,
        Self::Brass,
        Self::Keyboards,
        Self::PluckedStrings,
        Self::Strings,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::Woodwinds => "MADERAS",
            Self::Brass => "METALES",
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
}

impl Clef {
    pub fn bottom_diatonic(self) -> i16 {
        match self {
            Self::Treble => 30,
            Self::Alto => 24,
            Self::Bass => 18,
        }
    }
}

impl Instrument {
    pub const ALL: [Self; 13] = [
        Self::Piano,
        Self::Guitar,
        Self::Violin,
        Self::Viola,
        Self::Cello,
        Self::DoubleBass,
        Self::Flute,
        Self::Oboe,
        Self::Clarinet,
        Self::Bassoon,
        Self::Trumpet,
        Self::Horn,
        Self::Trombone,
    ];
    pub fn family(self) -> InstrumentFamily {
        use InstrumentFamily::*;
        match self {
            Self::Flute | Self::Oboe | Self::Clarinet | Self::Bassoon => Woodwinds,
            Self::Horn | Self::Trumpet | Self::Trombone => Brass,
            Self::Piano => Keyboards,
            Self::Guitar => PluckedStrings,
            _ => Strings,
        }
    }
    /// Stable orchestral order. Repeated instruments retain their relative order.
    pub fn score_order(self) -> u8 {
        match self {
            Self::Flute => 0,
            Self::Oboe => 1,
            Self::Clarinet => 2,
            Self::Bassoon => 3,
            Self::Horn => 4,
            Self::Trumpet => 5,
            Self::Trombone => 6,
            Self::Piano => 7,
            Self::Guitar => 8,
            Self::Violin => 9,
            Self::Viola => 10,
            Self::Cello => 11,
            Self::DoubleBass => 12,
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Self::Piano => "Piano",
            Self::Guitar => "Guitarra",
            Self::Violin => "Violín",
            Self::Viola => "Viola",
            Self::Cello => "Violonchelo",
            Self::DoubleBass => "Contrabajo",
            Self::Flute => "Flauta",
            Self::Oboe => "Oboe",
            Self::Clarinet => "Clarinete",
            Self::Bassoon => "Fagot",
            Self::Trumpet => "Trompeta",
            Self::Horn => "Trompa",
            Self::Trombone => "Trombón",
        }
    }
    /// Zero-based General MIDI programs in the bundled GeneralUser bank.
    pub fn program(self) -> u8 {
        match self {
            Self::Piano => 0,
            Self::Guitar => 24,
            Self::Violin => 40,
            Self::Viola => 41,
            Self::Cello => 42,
            Self::DoubleBass => 43,
            Self::Flute => 73,
            Self::Oboe => 68,
            Self::Clarinet => 71,
            Self::Bassoon => 70,
            Self::Trumpet => 56,
            Self::Horn => 60,
            Self::Trombone => 57,
        }
    }
    pub fn clef(self) -> Clef {
        match self {
            Self::Viola => Clef::Alto,
            Self::Cello | Self::DoubleBass | Self::Bassoon | Self::Trombone => Clef::Bass,
            _ => Clef::Treble,
        }
    }
    pub fn entry_octave(self) -> u8 {
        match self.clef() {
            Clef::Treble => 4,
            Clef::Alto => 4,
            Clef::Bass => 3,
        }
    }
}
