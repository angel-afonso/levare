//! Written note values and exact single-dot / 3:2 time modifications.
use serde::{Deserialize, Deserializer, Serialize, Serializer};

pub const TICKS_PER_WHOLE: u16 = 192;
pub const TICKS_PER_QUARTER: u16 = TICKS_PER_WHOLE / 4;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum NoteValue {
    Whole,
    Half,
    #[default]
    Quarter,
    Eighth,
    Sixteenth,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Duration {
    value: NoteValue,
    dotted: bool,
    triplet: bool,
}
#[allow(non_upper_case_globals)]
impl Duration {
    pub const MIN_TICKS: u16 = 8;
    pub const Whole: Self = Self::plain(NoteValue::Whole);
    pub const Half: Self = Self::plain(NoteValue::Half);
    pub const Quarter: Self = Self::plain(NoteValue::Quarter);
    pub const Eighth: Self = Self::plain(NoteValue::Eighth);
    pub const Sixteenth: Self = Self::plain(NoteValue::Sixteenth);
    pub const ALL: [Self; 5] = [
        Self::Whole,
        Self::Half,
        Self::Quarter,
        Self::Eighth,
        Self::Sixteenth,
    ];
    const fn plain(value: NoteValue) -> Self {
        Self {
            value,
            dotted: false,
            triplet: false,
        }
    }
    pub fn value(self) -> NoteValue {
        self.value
    }
    pub fn is_dotted(self) -> bool {
        self.dotted
    }
    pub fn is_triplet(self) -> bool {
        self.triplet
    }
    pub fn with_dotted(self, dotted: bool) -> Self {
        Self { dotted, ..self }
    }
    pub fn with_triplet(self, triplet: bool) -> Self {
        Self { triplet, ..self }
    }
    pub fn with_value(self, value: NoteValue) -> Self {
        Self { value, ..self }
    }
    pub fn ticks(self) -> u16 {
        let mut ticks = match self.value {
            NoteValue::Whole => 192,
            NoteValue::Half => 96,
            NoteValue::Quarter => 48,
            NoteValue::Eighth => 24,
            NoteValue::Sixteenth => 12,
        };
        if self.dotted {
            ticks = ticks * 3 / 2;
        }
        if self.triplet {
            ticks = ticks * 2 / 3;
        }
        ticks
    }
    pub fn label(self) -> String {
        let base = match self.value {
            NoteValue::Whole => "Redonda",
            NoteValue::Half => "Blanca",
            NoteValue::Quarter => "Negra",
            NoteValue::Eighth => "Corchea",
            NoteValue::Sixteenth => "Semicorchea",
        };
        format!(
            "{base}{}{}",
            if self.dotted { " con puntillo" } else { "" },
            if self.triplet { " · tresillo" } else { "" }
        )
    }
    pub fn all_modified() -> Vec<Self> {
        Self::ALL
            .into_iter()
            .flat_map(|d| {
                [
                    d,
                    d.with_dotted(true),
                    d.with_triplet(true),
                    d.with_dotted(true).with_triplet(true),
                ]
            })
            .collect()
    }
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Modified {
    value: NoteValue,
    #[serde(default)]
    dotted: bool,
    #[serde(default)]
    triplet: bool,
}
impl Serialize for Duration {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        if !self.dotted && !self.triplet {
            self.value.serialize(serializer)
        } else {
            Modified {
                value: self.value,
                dotted: self.dotted,
                triplet: self.triplet,
            }
            .serialize(serializer)
        }
    }
}
impl<'de> Deserialize<'de> for Duration {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Wire {
            Plain(NoteValue),
            Modified(Modified),
        }
        Ok(match Wire::deserialize(deserializer)? {
            Wire::Plain(value) => Self::plain(value),
            Wire::Modified(m) => Self {
                value: m.value,
                dotted: m.dotted,
                triplet: m.triplet,
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn dots_and_triplets_keep_exact_integer_ratios_for_every_base_value() {
        for base in Duration::ALL {
            let dotted = base.with_dotted(true);
            let triplet = base.with_triplet(true);
            assert_eq!(dotted.ticks() * 2, base.ticks() * 3);
            assert_eq!(triplet.ticks() * 3, base.ticks() * 2);
            assert_eq!(dotted.with_triplet(true).ticks(), base.ticks());
            assert_eq!(dotted.with_dotted(false), base);
            assert_eq!(triplet.with_triplet(false), base);
        }
        assert_eq!(
            Duration::Quarter.with_dotted(true).ticks() + Duration::Eighth.ticks(),
            Duration::Half.ticks()
        );
        assert_eq!(
            Duration::Eighth.with_triplet(true).ticks() * 3,
            Duration::Quarter.ticks()
        );
    }
}
