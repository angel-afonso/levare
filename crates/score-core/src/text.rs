//! Words written at a note or rest: an expression in italics below the staff
//! («dolce», «cresc.») or a playing instruction above it («con sord.»). The
//! words that have a musical meaning also change the playback.
use super::*;
use serde::{Deserializer, Serializer};

/// Room for one indication; longer prose belongs in a comment.
pub const MAX_TEXT_BYTES: usize = 40;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TextKind {
    Expression,
    Technique,
}

impl TextKind {
    pub const ALL: [Self; 2] = [Self::Expression, Self::Technique];
    pub const fn label(self) -> &'static str {
        match self {
            Self::Expression => "Texto de expresión",
            Self::Technique => "Texto de técnica",
        }
    }
    /// Common indications offered while writing.
    pub const fn suggestions(self) -> &'static [&'static str] {
        match self {
            Self::Expression => &[
                "dolce",
                "espress.",
                "cantabile",
                "legato",
                "cresc.",
                "dim.",
                "sempre",
                "subito",
                "marcato",
                "leggiero",
            ],
            Self::Technique => &[
                "con sord.",
                "senza sord.",
                "sul pont.",
                "sul tasto",
                "col legno",
                "legato",
                "non legato",
                "ord.",
            ],
        }
    }
}

/// Up to forty bytes of text, kept inline so a note stays `Copy`.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Words {
    len: u8,
    bytes: [u8; MAX_TEXT_BYTES],
}

impl Words {
    pub fn new(text: &str) -> Result<Self, ScoreError> {
        let text = text.trim();
        if text.is_empty() {
            return Err(ScoreError("Escribe el texto de la indicación"));
        }
        if text.chars().any(char::is_control) {
            return Err(ScoreError(
                "El texto no admite saltos de línea ni tabuladores",
            ));
        }
        if text.len() > MAX_TEXT_BYTES {
            return Err(ScoreError(
                "El texto es demasiado largo: escribe una indicación breve",
            ));
        }
        let mut bytes = [0; MAX_TEXT_BYTES];
        bytes[..text.len()].copy_from_slice(text.as_bytes());
        Ok(Self {
            len: text.len() as u8,
            bytes,
        })
    }

    pub fn as_str(&self) -> &str {
        std::str::from_utf8(&self.bytes[..usize::from(self.len)]).expect("validated UTF-8")
    }

    /// The words as a player reads them: lowercase, without a final period.
    fn normalized(&self) -> String {
        self.as_str().trim_end_matches('.').trim().to_lowercase()
    }

    /// What these words ask the player to do, if playback can follow it.
    pub fn effect(&self, kind: TextKind) -> Option<TextEffect> {
        let words = self.normalized();
        Some(match (kind, words.as_str()) {
            (TextKind::Expression, "cresc" | "crescendo" | "poco a poco cresc") => {
                TextEffect::Swell(true)
            }
            (TextKind::Expression, "dim" | "diminuendo" | "decresc" | "decrescendo") => {
                TextEffect::Swell(false)
            }
            (_, "legato") => TextEffect::Legato(true),
            (_, "non legato") => TextEffect::Legato(false),
            (TextKind::Technique, "con sord" | "con sordino" | "con sordina" | "sord") => {
                TextEffect::Mute(true)
            }
            (
                TextKind::Technique,
                "senza sord" | "senza sordino" | "senza sordina" | "via sord",
            ) => TextEffect::Mute(false),
            (TextKind::Technique, "ord" | "ordinario" | "nat" | "naturale") => TextEffect::Ordinary,
            _ => return None,
        })
    }
}

impl std::fmt::Debug for Words {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.as_str().fmt(f)
    }
}

impl PartialEq<str> for Words {
    fn eq(&self, other: &str) -> bool {
        self.as_str() == other
    }
}

impl PartialEq<&str> for Words {
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
    }
}

impl std::fmt::Display for Words {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl Serialize for Words {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for Words {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        Self::new(&text).map_err(|error| serde::de::Error::custom(error.0))
    }
}

/// The playback meaning of a written indication.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextEffect {
    /// «cresc.» (true) or «dim.» up to the next dynamic.
    Swell(bool),
    /// Bound notes until «non legato» or «ord.».
    Legato(bool),
    /// A mute softens and darkens the sound until «senza sord.» or «ord.».
    Mute(bool),
    /// «ord.»: back to the ordinary way of playing.
    Ordinary,
}

/// A written «cresc.» or «dim.» as the span of the envelope it shapes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Swell {
    pub start: u64,
    pub end: u64,
    pub crescendo: bool,
}

impl Score {
    pub fn text(&self, id: ElementId, kind: TextKind) -> Option<Words> {
        let notation = self.event(id)?.notation;
        match kind {
            TextKind::Expression => notation.expression_text,
            TextKind::Technique => notation.technique_text,
        }
    }

    /// Write `text` on the note or rest at `id`, or remove it with `None`.
    /// «pizz.» and «arco» on bowed strings become their playing technique.
    pub fn set_text(
        &mut self,
        id: ElementId,
        kind: TextKind,
        text: Option<&str>,
    ) -> Result<(), ScoreError> {
        self.event(id)
            .ok_or(ScoreError("Selecciona una nota o silencio"))?;
        let words = text.map(Words::new).transpose()?;
        if kind == TextKind::Technique
            && let Some(words) = words
            && self.part(id.part).unwrap().instrument().family() == InstrumentFamily::Strings
        {
            let technique = match words.normalized().as_str() {
                "pizz" | "pizzicato" => Some(PlayingTechnique::Pizzicato),
                "arco" => Some(PlayingTechnique::Arco),
                _ => None,
            };
            if let Some(technique) = technique {
                self.set_technique(id, Some(technique))?;
                return self.set_text(id, kind, None);
            }
        }
        let notation = &mut Arc::make_mut(
            &mut self.part_mut(id.part)?.staff_mut(id.staff)?.measures[id.measure].events,
        )[id.event]
            .notation;
        match kind {
            TextKind::Expression => {
                notation.expression_text = words;
                if words.is_none() {
                    notation.offsets.expression_text = None;
                }
            }
            TextKind::Technique => {
                notation.technique_text = words;
                if words.is_none() {
                    notation.offsets.technique_text = None;
                }
            }
        }
        Ok(())
    }

    /// Whether a hairpin or a written swell shapes this part's level.
    pub fn shapes_level(&self, part: PartId) -> bool {
        self.connections()
            .iter()
            .any(|c| c.kind.is_hairpin() && c.start.part == part)
            || self.part(part).is_some_and(|value| {
                value
                    .staves()
                    .iter()
                    .any(|staff| !self.swells(part, staff.id()).is_empty())
            })
    }

    /// Written «cresc.» and «dim.» on the staff's expression line, in order.
    /// Each lasts until the next dynamic or indication, else to the end of
    /// the following bar; one inside a hairpin gives way to it.
    pub fn swells(&self, part: PartId, staff: StaffId) -> Vec<Swell> {
        let Some(part_value) = self.part(part) else {
            return Vec::new();
        };
        let mut words = std::collections::BTreeMap::new();
        let mut stops = std::collections::BTreeSet::new();
        for source in part_value.expression_staves(staff) {
            for (index, bar) in source.measures().iter().enumerate() {
                let begin = self.measure_start_tick(index);
                for (_, onset, _, event) in bar.timed_iter() {
                    let tick = begin + u64::from(onset);
                    if event.notation.dynamic.is_some() {
                        stops.insert(tick);
                    }
                    if let Some(TextEffect::Swell(crescendo)) = event
                        .notation
                        .expression_text
                        .and_then(|text| text.effect(TextKind::Expression))
                    {
                        words.insert(tick, (crescendo, index));
                        stops.insert(tick);
                    }
                }
            }
        }
        if words.is_empty() {
            return Vec::new();
        }
        let hairpins: Vec<_> = self
            .connections()
            .iter()
            .filter(|c| {
                c.kind.is_hairpin()
                    && c.start.part == part
                    && (c.start.staff == staff || part_value.is_divisi())
            })
            .filter_map(|c| Some((self.event_tick(c.start)?, self.event_tick(c.end)?)))
            .collect();
        let total = self.total_ticks();
        words
            .into_iter()
            .filter_map(|(start, (crescendo, bar))| {
                let end = stops.range(start + 1..).next().copied().unwrap_or_else(|| {
                    if bar + 2 < self.measures().len() {
                        self.measure_start_tick(bar + 2)
                    } else {
                        total
                    }
                });
                let free = hairpins.iter().all(|&(a, b)| end <= a || start >= b);
                (start < end && free).then_some(Swell {
                    start,
                    end,
                    crescendo,
                })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn note(step: Step) -> Event {
        Event {
            pitches: Pitches::single(Pitch::new(step, 5)),
            duration: Duration::Quarter,
            notation: Notation::default(),
        }
    }

    #[test]
    fn words_are_short_single_line_indications() {
        let words = Words::new("  con sord. ").unwrap();
        assert_eq!(words.as_str(), "con sord.");
        assert_eq!(words.to_string(), "con sord.");
        assert!(Words::new("").is_err());
        assert!(Words::new("a\nb").is_err());
        assert!(Words::new(&"x".repeat(MAX_TEXT_BYTES + 1)).is_err());
        assert!(Words::new(&"é".repeat(MAX_TEXT_BYTES / 2)).is_ok());
    }

    #[test]
    fn known_words_carry_their_playback_meaning() {
        let effect = |kind, text| Words::new(text).unwrap().effect(kind);
        assert_eq!(
            effect(TextKind::Expression, "Cresc."),
            Some(TextEffect::Swell(true))
        );
        assert_eq!(
            effect(TextKind::Expression, "dim."),
            Some(TextEffect::Swell(false))
        );
        assert_eq!(
            effect(TextKind::Technique, "con sord."),
            Some(TextEffect::Mute(true))
        );
        assert_eq!(
            effect(TextKind::Technique, "ord."),
            Some(TextEffect::Ordinary)
        );
        assert_eq!(
            effect(TextKind::Expression, "legato"),
            Some(TextEffect::Legato(true))
        );
        assert_eq!(effect(TextKind::Expression, "dolce"), None);
        // A swell is an expression; as an instruction above it means nothing.
        assert_eq!(effect(TextKind::Technique, "cresc."), None);
    }

    #[test]
    fn texts_sit_on_notes_and_rests_and_pizz_on_strings_becomes_the_technique() {
        let mut score = Score::new("".into(), &[Instrument::Violin], 2).unwrap();
        let a = score.insert(0, 0, note(Step::A)).unwrap();
        score
            .set_text(a, TextKind::Expression, Some("dolce"))
            .unwrap();
        score
            .set_text(a, TextKind::Technique, Some("con sord."))
            .unwrap();
        assert_eq!(
            score.text(a, TextKind::Expression).unwrap().as_str(),
            "dolce"
        );
        score.erase(a).unwrap();
        assert_eq!(
            score.text(a, TextKind::Technique).unwrap().as_str(),
            "con sord."
        );
        score
            .set_text(a, TextKind::Technique, Some("pizz."))
            .unwrap();
        assert_eq!(score.text(a, TextKind::Technique), None);
        assert_eq!(
            score.event(a).unwrap().notation.technique,
            Some(PlayingTechnique::Pizzicato)
        );
        score.set_text(a, TextKind::Expression, None).unwrap();
        assert_eq!(score.text(a, TextKind::Expression), None);
        score.validate().unwrap();
    }

    #[test]
    fn a_written_crescendo_lasts_until_the_next_dynamic_or_two_bars() {
        let mut score = Score::default();
        for _ in 0..3 {
            score.append_measure().unwrap();
        }
        let ids: Vec<_> = (0..4)
            .map(|bar| score.insert(bar, 0, note(Step::C)).unwrap())
            .collect();
        score
            .set_text(ids[0], TextKind::Expression, Some("cresc."))
            .unwrap();
        score.set_dynamic(ids[1], Some(Dynamic::F)).unwrap();
        score
            .set_text(ids[2], TextKind::Expression, Some("dim."))
            .unwrap();
        let bar = u64::from(score.time_at(0).capacity());
        let part = score.parts()[0].id();
        assert_eq!(
            score.swells(part, StaffId::Upper),
            [
                Swell {
                    start: 0,
                    end: bar,
                    crescendo: true
                },
                Swell {
                    start: 2 * bar,
                    end: 4 * bar,
                    crescendo: false
                },
            ]
        );
    }
}
