//! Small, bounded mixer controls shared by either sound engine.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChannelMix {
    pub volume: u8,
    /// -100 = left, 0 = center, 100 = right.
    pub pan: i8,
}
impl Default for ChannelMix {
    fn default() -> Self {
        Self {
            volume: 100,
            pan: 0,
        }
    }
}
impl ChannelMix {
    /// Default seat of a part on a concert stage, as heard from the audience:
    /// violins left, low strings right, winds and brass spread behind them.
    /// A part alone stays centered; a repeated instrument (Violin II) sits
    /// 20 % further right than the previous one, so duplicates do not stack.
    pub fn seated(score: &score_core::Score, part: score_core::PartId) -> Self {
        use score_core::Instrument::*;
        let parts = score.parts();
        let Some(index) = parts.iter().position(|p| p.id() == part) else {
            return Self::default();
        };
        if parts.len() == 1 {
            return Self::default();
        }
        let instrument = parts[index].instrument();
        let pan: i16 = match instrument {
            Violin => -50,
            Viola => 15,
            Cello => 40,
            DoubleBass => 60,
            Flute => -15,
            Oboe => 10,
            Clarinet => -25,
            Bassoon => 20,
            Horn => -35,
            Trumpet => 20,
            Trombone => 40,
            Piano => -20,
            Guitar => 0,
            _ => match instrument.family() {
                score_core::InstrumentFamily::Woodwinds => -10,
                score_core::InstrumentFamily::Brass => 25,
                score_core::InstrumentFamily::Percussion => 10,
                _ => -20,
            },
        };
        let ordinal = parts
            .iter()
            .take(index)
            .filter(|p| p.instrument() == instrument)
            .count() as i16;
        Self {
            volume: 100,
            pan: (pan + ordinal * 20).clamp(-100, 100) as i8,
        }
    }
    pub(crate) fn packed(self) -> u32 {
        u32::from(self.volume.min(100))
            | (((i32::from(self.pan).clamp(-100, 100) + 100) as u32) << 8)
    }
    pub(crate) fn unpack(value: u32) -> Self {
        Self {
            volume: (value & 255) as u8,
            pan: ((value >> 8) as i32 - 100) as i8,
        }
    }
    pub(crate) fn midi_pan(self) -> u8 {
        let pan = i32::from(self.pan);
        (64 + if pan < 0 {
            pan * 64 / 100
        } else {
            pan * 63 / 100
        }) as u8
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReverbSettings {
    pub enabled: bool,
    pub amount: u8,
    pub room: u8,
}
impl Default for ReverbSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            amount: 14,
            room: 40,
        }
    }
}
impl ReverbSettings {
    pub(crate) fn packed(self) -> u32 {
        u32::from(if self.enabled {
            self.amount.min(100)
        } else {
            0
        }) | (u32::from(self.room.min(100)) << 8)
    }
}
