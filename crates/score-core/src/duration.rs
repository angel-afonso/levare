//! Written note values with up to two dots and an optional tuplet ratio, all
//! exact in ticks.
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// 2¹¹·3²·5·7: every value down to a double-dotted sixty-fourth, in any
/// tuplet from 2 to 9, lasts a whole number of ticks.
pub const TICKS_PER_WHOLE: u32 = 645_120;
pub const TICKS_PER_QUARTER: u32 = TICKS_PER_WHOLE / 4;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum NoteValue {
    Whole,
    Half,
    #[default]
    Quarter,
    Eighth,
    Sixteenth,
    ThirtySecond,
    SixtyFourth,
}
impl NoteValue {
    pub fn ticks(self) -> u32 {
        TICKS_PER_WHOLE >> self as u32
    }
}

/// `actual` notes of `unit` in the time of `normal`: five sixteenths in the
/// time of four is `5:4` counted in sixteenths. The unit says how far a group
/// reaches, not how long each of its notes lasts.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct Tuplet {
    actual: u8,
    normal: u8,
    unit: NoteValue,
}
impl Tuplet {
    pub const MAX_ACTUAL: u8 = 9;
    /// A ratio that compresses or stretches by less than double, and is not
    /// one to one.
    pub fn new(actual: u8, normal: u8, unit: NoteValue) -> Option<Self> {
        ((2..=Self::MAX_ACTUAL).contains(&actual)
            && normal != actual
            && u16::from(normal) * 2 > u16::from(actual)
            && u16::from(normal) < u16::from(actual) * 2)
            .then_some(Self {
                actual,
                normal,
                unit,
            })
    }
    pub fn triplet(unit: NoteValue) -> Self {
        Self {
            actual: 3,
            normal: 2,
            unit,
        }
    }
    /// The usual ratio for `actual` notes: 2:3, 3:2, 4:3, 5:4, 6:4, 7:4, 8:6
    /// and 9:8.
    pub fn usual(actual: u8, unit: NoteValue) -> Option<Self> {
        let normal = match actual {
            2 => 3,
            3 => 2,
            4 => 3,
            5..=7 => 4,
            8 => 6,
            9 => 8,
            _ => return None,
        };
        Self::new(actual, normal, unit)
    }
    pub fn actual(self) -> u8 {
        self.actual
    }
    pub fn normal(self) -> u8 {
        self.normal
    }
    pub fn unit(self) -> NoteValue {
        self.unit
    }
    pub fn is_triplet(self) -> bool {
        self.actual == 3 && self.normal == 2
    }
    fn with_unit(self, unit: NoteValue) -> Self {
        Self { unit, ..self }
    }
    /// «tresillo», «quintillo»…: what musicians call a group of `actual`.
    pub fn name(self) -> &'static str {
        match self.actual {
            2 => "dosillo",
            3 => "tresillo",
            4 => "cuatrillo",
            5 => "quintillo",
            6 => "seisillo",
            7 => "septillo",
            8 => "octillo",
            _ => "nonillo",
        }
    }
}
impl<'de> Deserialize<'de> for Tuplet {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Wire {
            actual: u8,
            normal: u8,
            unit: NoteValue,
        }
        let wire = Wire::deserialize(deserializer)?;
        Self::new(wire.actual, wire.normal, wire.unit)
            .ok_or_else(|| serde::de::Error::custom("proporción de grupillo no válida"))
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Duration {
    value: NoteValue,
    dots: u8,
    tuplet: Option<Tuplet>,
}
#[allow(non_upper_case_globals)]
impl Duration {
    pub const MAX_DOTS: u8 = 2;
    /// A sixty-fourth in the tightest ratio, 9:5.
    pub const MIN_TICKS: u32 = TICKS_PER_WHOLE / 64 * 5 / 9;
    pub const Whole: Self = Self::plain(NoteValue::Whole);
    pub const Half: Self = Self::plain(NoteValue::Half);
    pub const Quarter: Self = Self::plain(NoteValue::Quarter);
    pub const Eighth: Self = Self::plain(NoteValue::Eighth);
    pub const Sixteenth: Self = Self::plain(NoteValue::Sixteenth);
    pub const ThirtySecond: Self = Self::plain(NoteValue::ThirtySecond);
    pub const SixtyFourth: Self = Self::plain(NoteValue::SixtyFourth);
    pub const ALL: [Self; 7] = [
        Self::Whole,
        Self::Half,
        Self::Quarter,
        Self::Eighth,
        Self::Sixteenth,
        Self::ThirtySecond,
        Self::SixtyFourth,
    ];
    const fn plain(value: NoteValue) -> Self {
        Self {
            value,
            dots: 0,
            tuplet: None,
        }
    }
    pub fn value(self) -> NoteValue {
        self.value
    }
    pub fn dots(self) -> u8 {
        self.dots
    }
    pub fn tuplet(self) -> Option<Tuplet> {
        self.tuplet
    }
    /// One dot or two.
    pub fn is_dotted(self) -> bool {
        self.dots > 0
    }
    pub fn is_triplet(self) -> bool {
        self.tuplet.is_some_and(Tuplet::is_triplet)
    }
    /// One dot, keeping two if there are; or none.
    pub fn with_dotted(self, dotted: bool) -> Self {
        self.with_dots(if dotted { self.dots.max(1) } else { 0 })
    }
    pub fn with_dots(self, dots: u8) -> Self {
        Self {
            dots: dots.min(Self::MAX_DOTS),
            ..self
        }
    }
    /// A 3:2 tuplet counted in this value, or none.
    pub fn with_triplet(self, triplet: bool) -> Self {
        self.with_tuplet(triplet.then(|| Tuplet::triplet(self.value)))
    }
    pub fn with_tuplet(self, tuplet: Option<Tuplet>) -> Self {
        Self { tuplet, ..self }
    }
    /// A tuplet counted in the old value goes on counting in the new one.
    pub fn with_value(self, value: NoteValue) -> Self {
        Self {
            value,
            tuplet: self.tuplet.map(|t| {
                if t.unit == self.value {
                    t.with_unit(value)
                } else {
                    t
                }
            }),
            ..self
        }
    }
    pub fn ticks(self) -> u32 {
        let base = self.value.ticks();
        // Each dot adds half of what the previous one added.
        let dotted = (0..u32::from(self.dots)).fold(base, |sum, dot| sum + (base >> (dot + 1)));
        match self.tuplet {
            Some(t) => dotted / u32::from(t.actual) * u32::from(t.normal),
            None => dotted,
        }
    }
    pub fn label(self) -> String {
        let base = match self.value {
            NoteValue::Whole => "Redonda",
            NoteValue::Half => "Blanca",
            NoteValue::Quarter => "Negra",
            NoteValue::Eighth => "Corchea",
            NoteValue::Sixteenth => "Semicorchea",
            NoteValue::ThirtySecond => "Fusa",
            NoteValue::SixtyFourth => "Semifusa",
        };
        let dots = match self.dots {
            0 => "",
            1 => " con puntillo",
            _ => " con doble puntillo",
        };
        let tuplet = match self.tuplet {
            None => String::new(),
            Some(t) if t.is_triplet() => " · tresillo".into(),
            Some(t) => format!(" · {} ({}:{})", t.name(), t.actual, t.normal),
        };
        format!("{base}{dots}{tuplet}")
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Modified {
    value: NoteValue,
    #[serde(default, skip_serializing_if = "is_zero")]
    dots: u8,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    tuplet: Option<Tuplet>,
}
fn is_zero(value: &u8) -> bool {
    *value == 0
}
/// How files before format 30 wrote a dot and a 3:2 triplet.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Legacy {
    value: NoteValue,
    #[serde(default)]
    dotted: bool,
    #[serde(default)]
    triplet: bool,
}
impl Serialize for Duration {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        if self.dots == 0 && self.tuplet.is_none() {
            self.value.serialize(serializer)
        } else {
            Modified {
                value: self.value,
                dots: self.dots,
                tuplet: self.tuplet,
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
            Legacy(Legacy),
        }
        let duration = match Wire::deserialize(deserializer)? {
            Wire::Plain(value) => Self::plain(value),
            Wire::Modified(m) => Self {
                value: m.value,
                dots: m.dots,
                tuplet: m.tuplet,
            },
            Wire::Legacy(l) => Self::plain(l.value)
                .with_dotted(l.dotted)
                .with_triplet(l.triplet),
        };
        if duration.dots > Self::MAX_DOTS {
            return Err(serde::de::Error::custom("demasiados puntillos"));
        }
        Ok(duration)
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

    fn every_tuplet(unit: NoteValue) -> impl Iterator<Item = Option<Tuplet>> {
        std::iter::once(None).chain(
            (2..=Tuplet::MAX_ACTUAL)
                .flat_map(move |actual| {
                    (1..=2 * actual).filter_map(move |n| Tuplet::new(actual, n, unit))
                })
                .map(Some),
        )
    }

    #[test]
    fn every_supported_duration_has_exact_ticks() {
        let mut shortest = u32::MAX;
        for base in Duration::ALL {
            for dots in 0..=Duration::MAX_DOTS {
                let dotted = base.with_dots(dots);
                // Each dot adds half of the previous: ×1, ×3/2, ×7/4.
                let factor = [(1, 1), (3, 2), (7, 4)][usize::from(dots)];
                assert_eq!(dotted.ticks() * factor.1, base.ticks() * factor.0);
                for tuplet in every_tuplet(base.value()) {
                    let duration = dotted.with_tuplet(tuplet);
                    let (actual, normal) = tuplet.map_or((1, 1), |t| (t.actual(), t.normal()));
                    assert_eq!(
                        u64::from(duration.ticks()) * u64::from(actual),
                        u64::from(dotted.ticks()) * u64::from(normal),
                        "{}",
                        duration.label()
                    );
                    shortest = shortest.min(duration.ticks());
                }
            }
        }
        assert_eq!(shortest, Duration::MIN_TICKS);
    }

    #[test]
    fn tuplet_ratios_stay_within_double_and_half() {
        assert!(Tuplet::new(5, 4, NoteValue::Sixteenth).is_some());
        assert!(Tuplet::new(7, 8, NoteValue::Sixteenth).is_some());
        for (actual, normal) in [(3, 3), (5, 1), (2, 1), (4, 8), (10, 8), (1, 2)] {
            assert_eq!(Tuplet::new(actual, normal, NoteValue::Eighth), None);
        }
        for actual in 2..=Tuplet::MAX_ACTUAL {
            assert!(Tuplet::usual(actual, NoteValue::Eighth).is_some());
        }
    }

    #[test]
    fn legacy_triplet_json_reads_as_three_in_two_of_its_own_value() {
        let old: Duration =
            serde_json::from_str(r#"{"value":"Eighth","dotted":true,"triplet":true}"#).unwrap();
        assert_eq!(
            old,
            Duration::Eighth
                .with_dots(1)
                .with_tuplet(Some(Tuplet::triplet(NoteValue::Eighth)))
        );
        let dotted: Duration = serde_json::from_str(r#"{"value":"Half","dotted":true}"#).unwrap();
        assert_eq!(dotted, Duration::Half.with_dotted(true));
    }

    #[test]
    fn plain_values_still_serialize_as_bare_strings() {
        assert_eq!(
            serde_json::to_string(&Duration::Quarter).unwrap(),
            r#""Quarter""#
        );
        let quintuplet = Duration::Sixteenth
            .with_dots(2)
            .with_tuplet(Tuplet::usual(5, NoteValue::Sixteenth));
        let json = serde_json::to_string(&quintuplet).unwrap();
        assert_eq!(
            json,
            r#"{"value":"Sixteenth","dots":2,"tuplet":{"actual":5,"normal":4,"unit":"Sixteenth"}}"#
        );
        assert_eq!(serde_json::from_str::<Duration>(&json).unwrap(), quintuplet);
        assert!(serde_json::from_str::<Duration>(r#"{"value":"Half","dots":3}"#).is_err());
        assert!(
            serde_json::from_str::<Duration>(
                r#"{"value":"Half","tuplet":{"actual":3,"normal":3,"unit":"Half"}}"#
            )
            .is_err()
        );
    }

    #[test]
    fn labels_name_dots_and_tuplets() {
        assert_eq!(
            Duration::Quarter.with_dotted(true).label(),
            "Negra con puntillo"
        );
        assert_eq!(
            Duration::Eighth.with_triplet(true).label(),
            "Corchea · tresillo"
        );
        assert_eq!(
            Duration::Eighth
                .with_tuplet(Tuplet::usual(5, NoteValue::Eighth))
                .label(),
            "Corchea · quintillo (5:4)"
        );
        assert_eq!(
            Duration::SixtyFourth.with_dots(2).label(),
            "Semifusa con doble puntillo"
        );
    }
}
