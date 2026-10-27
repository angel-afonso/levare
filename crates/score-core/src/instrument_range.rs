//! Advisory orchestral ranges in sounding pitch, independent of sample banks.
//! Defaults and reference conventions are documented in docs/instrument-ranges.md.
use crate::{ElementId, Harmonic, Instrument, Part, PartId, Pitch, Score, ScoreError};
use serde::{Deserialize, Serialize};
use std::fmt;

/// Inclusive sounding MIDI bounds (middle C / C4 = 60).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstrumentRange {
    pub lowest: u8,
    pub highest: u8,
}

impl InstrumentRange {
    pub fn validate(self) -> Result<(), ScoreError> {
        if self.lowest > self.highest || self.highest > 127 {
            return Err(ScoreError(
                "El límite inferior debe ser menor o igual al superior (C0–G9)",
            ));
        }
        if self.lowest < 12 {
            return Err(ScoreError("El rango instrumental debe estar entre C0 y G9"));
        }
        Ok(())
    }
    pub fn contains(self, sounding_midi: i16) -> bool {
        (i16::from(self.lowest)..=i16::from(self.highest)).contains(&sounding_midi)
    }
}

impl Part {
    pub fn range_override(&self) -> Option<InstrumentRange> {
        self.instrument_range
    }

    pub fn sounding_range(&self) -> Option<InstrumentRange> {
        self.instrument_range
            .or_else(|| self.instrument().sounding_range())
    }
}

impl fmt::Display for InstrumentRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = |midi: u8| {
            format!(
                "{}{}",
                [
                    "C", "C♯", "D", "E♭", "E", "F", "F♯", "G", "A♭", "A", "B♭", "B"
                ][usize::from(midi % 12)],
                i16::from(midi / 12) - 1
            )
        };
        write!(f, "{}–{}", name(self.lowest), name(self.highest))
    }
}

impl Instrument {
    /// Typical professional range, not an absolute physical or difficulty limit.
    /// Unpitched percussion has no pitch-range warning.
    pub fn sounding_range(self) -> Option<InstrumentRange> {
        use Instrument::*;
        let (lowest, highest) = match self {
            Piano => (21, 108),             // A0–C8
            Guitar => (40, 83),             // E2–B5, 19-fret classical guitar
            Violin => (55, 105),            // G3–A7
            Viola => (48, 93),              // C3–A6
            Cello => (36, 81),              // C2–A5
            DoubleBass => (23, 67),         // B0–G4, orchestral five-string bass
            Flute => (59, 98),              // B3–D7, B footjoint
            Piccolo => (74, 108),           // D5–C8
            AltoFlute => (55, 91),          // G3–G6
            BassFlute => (48, 84),          // C3–C6
            Oboe => (58, 91),               // B♭3–G6
            OboeDAmore => (55, 88),         // G3–E6
            EnglishHorn => (52, 81),        // E3–A5
            Clarinet => (52, 96),           // E3–C7
            ClarinetBb => (50, 94),         // D3–B♭6
            ClarinetA => (49, 93),          // C♯3–A6
            ClarinetEb => (55, 99),         // G3–E♭7
            BassClarinet => (34, 83),       // B♭1–B5, low-C model
            Bassoon => (34, 77),            // B♭1–F5
            Contrabassoon => (23, 60),      // B0–C4
            SopranoSax => (56, 88),         // A♭3–E6
            AltoSax => (49, 81),            // C♯3–A5
            TenorSax => (44, 76),           // A♭2–E5
            BaritoneSax => (36, 69),        // C2–A4, low-A model
            Horn | HornF => (35, 77),       // B1–F5
            Trumpet => (54, 84),            // F♯3–C6
            TrumpetBb | Cornet => (52, 82), // E3–B♭5
            TrumpetD => (56, 86),           // A♭3–D6
            PiccoloTrumpet => (62, 91),     // D4–G6, four valves
            Trombone => (40, 77),           // E2–F5, ordinary (non-pedal) register
            BassTrombone => (22, 77),       // B♭0–F5
            Tuba => (28, 65),               // E1–F4, general orchestral tuba
            Timpani => (36, 57),            // C2–A3, standard set
            Glockenspiel => (79, 108),      // G5–C8
            Xylophone => (65, 108),         // F4–C8
            Marimba => (36, 96),            // C2–C7, five-octave model
            Vibraphone => (53, 89),         // F3–F6
            TubularBells => (60, 77),       // C4–F5
            Celesta => (60, 108),           // C4–C8, four-octave model
            Organ => (36, 96),              // C2–C7, manuals (without stop shifts)
            Harp => (24, 103),              // C1–G7
            Voice => (55, 84),              // G3–C6, a trained solo voice
            Soprano => (60, 81),            // C4–A5, choral writing
            Contralto => (53, 74),          // F3–D5
            Tenor => (48, 69),              // C3–A4, sounding
            BassVoice => (40, 62),          // E2–D4
            SnareDrum | BassDrum | Cymbals | Triangle | Tambourine | TamTam => return None,
        };
        Some(InstrumentRange { lowest, highest })
    }

    fn range_for(self, harmonic: Option<Harmonic>) -> Option<InstrumentRange> {
        let mut range = self.sounding_range()?;
        if let Some(harmonic) = harmonic {
            range.highest = match self {
                Self::Violin => 110,    // D8
                Self::Viola => 100,     // E7
                Self::Cello => 105,     // A7
                Self::DoubleBass => 91, // G6
                _ => return Some(range),
            };
            // The second partial is the lowest natural harmonic; artificial
            // fourth-partial harmonics sound two octaves above a stopped note.
            range.lowest += if harmonic == Harmonic::Natural {
                12
            } else {
                24
            };
        }
        Some(range)
    }
}

impl Score {
    pub fn set_instrument_range(
        &mut self,
        part: PartId,
        range: Option<InstrumentRange>,
    ) -> Result<bool, ScoreError> {
        let source = self
            .part(part)
            .ok_or(ScoreError("El instrumento no existe"))?;
        if let Some(range) = range {
            range.validate()?;
            if source.instrument().sounding_range().is_none() {
                return Err(ScoreError(
                    "La percusión sin altura no tiene rango instrumental",
                ));
            }
        }
        if source.instrument_range == range {
            return Ok(false);
        }
        self.part_mut(part)?.instrument_range = range;
        Ok(true)
    }

    /// A warning for one stored chord tone. Never rejects an edit or changes
    /// playback. Resolve the written key, the part's actual transposition and
    /// harmonic sound before comparing; generated touch diamonds are excluded.
    pub fn pitch_range_warning(&self, id: ElementId, pitch: Pitch) -> Option<InstrumentRange> {
        let part = self.part(id.part)?;
        let event = self.event(id)?;
        if !event.pitches.iter().any(|p| p == pitch) {
            return None;
        }
        let range = part
            .instrument_range
            .or_else(|| part.instrument().range_for(event.notation.harmonic))?;
        let pitch = pitch.in_key(self.written_key_at(id.part, id.measure));
        let harmonic_shift = if event.notation.harmonic == Some(Harmonic::Artificial) {
            24
        } else {
            0
        };
        let sounding = (i16::from(pitch.octave()) + 1) * 12
            + [0, 2, 4, 5, 7, 9, 11][pitch.step.index() as usize]
            + i16::from(pitch.alteration())
            + i16::from(part.transposition().chromatic)
            + harmonic_shift;
        (!range.contains(sounding)).then_some(range)
    }
}

#[cfg(test)]
mod tests;
