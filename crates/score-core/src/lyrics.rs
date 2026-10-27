//! Sung text: one syllable per note and verse, joined to the next one by a
//! hyphen inside a word or held over the following notes by an extender.
use super::*;
use serde::{Deserializer, Serializer};

pub const MAX_VERSES: usize = 4;
/// Room for one syllable or a short word.
pub const MAX_SYLLABLE_BYTES: usize = 20;

/// What comes after a syllable.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Join {
    /// The word ends here.
    #[default]
    Word,
    /// The word goes on at the next syllable: a hyphen between them.
    Hyphen,
    /// The syllable is held over the next notes: an extender line.
    Melisma,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Syllable {
    len: u8,
    bytes: [u8; MAX_SYLLABLE_BYTES],
    pub join: Join,
}

impl Syllable {
    pub fn new(text: &str, join: Join) -> Result<Self, ScoreError> {
        let text = text.trim();
        if text.is_empty() {
            return Err(ScoreError("Escribe la sílaba"));
        }
        if text
            .chars()
            .any(|c| c.is_control() || (c.is_whitespace() && c != ' '))
        {
            return Err(ScoreError("La sílaba no admite saltos de línea"));
        }
        if text.len() > MAX_SYLLABLE_BYTES {
            return Err(ScoreError("La sílaba es demasiado larga"));
        }
        let mut bytes = [0; MAX_SYLLABLE_BYTES];
        bytes[..text.len()].copy_from_slice(text.as_bytes());
        Ok(Self {
            len: text.len() as u8,
            bytes,
            join,
        })
    }

    pub fn text(&self) -> &str {
        std::str::from_utf8(&self.bytes[..usize::from(self.len)]).expect("validated UTF-8")
    }
}

impl std::fmt::Debug for Syllable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?} {:?}", self.text(), self.join)
    }
}

#[derive(Serialize, Deserialize)]
struct StoredSyllable {
    text: String,
    #[serde(default, skip_serializing_if = "is_word")]
    join: Join,
}

fn is_word(join: &Join) -> bool {
    *join == Join::Word
}

impl Serialize for Syllable {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        StoredSyllable {
            text: self.text().to_owned(),
            join: self.join,
        }
        .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Syllable {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let stored = StoredSyllable::deserialize(deserializer)?;
        Self::new(&stored.text, stored.join).map_err(|error| serde::de::Error::custom(error.0))
    }
}

/// The syllables of one note, verse by verse.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Lyrics([Option<Syllable>; MAX_VERSES]);

impl Lyrics {
    pub fn is_empty(&self) -> bool {
        self.0.iter().all(Option::is_none)
    }
    pub fn verse(&self, verse: usize) -> Option<Syllable> {
        self.0.get(verse).copied().flatten()
    }
    /// Write or clear one verse; there is no verse past `MAX_VERSES`.
    pub fn set(&mut self, verse: usize, syllable: Option<Syllable>) -> bool {
        match self.0.get_mut(verse) {
            Some(slot) => {
                *slot = syllable;
                true
            }
            None => false,
        }
    }
    pub fn verses(&self) -> impl Iterator<Item = (usize, Syllable)> + '_ {
        self.0
            .iter()
            .enumerate()
            .filter_map(|(verse, syllable)| Some((verse, (*syllable)?)))
    }
}

impl Score {
    /// Sing `syllable` on the note at `id` in `verse` (from 0), or clear it.
    pub fn set_syllable(
        &mut self,
        id: ElementId,
        verse: usize,
        syllable: Option<Syllable>,
    ) -> Result<(), ScoreError> {
        let event = self.event(id).ok_or(ScoreError("Selecciona una nota"))?;
        if verse >= MAX_VERSES {
            return Err(ScoreError("Hay como mucho cuatro estrofas"));
        }
        if syllable.is_some() && event.pitches.is_empty() {
            return Err(ScoreError("Un silencio no lleva letra"));
        }
        Arc::make_mut(
            &mut self.part_mut(id.part)?.staff_mut(id.staff)?.measures[id.measure].events,
        )[id.event]
            .notation
            .lyrics
            .set(verse, syllable);
        Ok(())
    }

    pub(crate) fn validate_lyrics(event: &Event) -> Result<(), ScoreError> {
        if event.pitches.is_empty() && !event.notation.lyrics.is_empty() {
            return Err(ScoreError("Letra en un silencio"));
        }
        Ok(())
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
    fn syllables_are_short_and_keep_how_they_join_the_next() {
        let syllable = Syllable::new(" A- ", Join::Hyphen).unwrap();
        assert_eq!((syllable.text(), syllable.join), ("A-", Join::Hyphen));
        assert!(Syllable::new("", Join::Word).is_err());
        assert!(Syllable::new("a\nb", Join::Word).is_err());
        assert!(Syllable::new(&"x".repeat(MAX_SYLLABLE_BYTES + 1), Join::Word).is_err());
        assert!(
            Syllable::new("o a", Join::Word).is_ok(),
            "an elision keeps its space"
        );
    }

    #[test]
    fn notes_sing_up_to_four_verses_and_rests_none() {
        let mut score = Score::default();
        let a = score.insert(0, 0, note(Step::C)).unwrap();
        let sing = |text| Some(Syllable::new(text, Join::Word).unwrap());
        score.set_syllable(a, 0, sing("Sol")).unwrap();
        score.set_syllable(a, 3, sing("Luz")).unwrap();
        assert!(score.set_syllable(a, MAX_VERSES, sing("No")).is_err());
        let lyrics = score.event(a).unwrap().notation.lyrics;
        assert_eq!(
            lyrics
                .verses()
                .map(|(v, s)| (v, s.text().to_owned()))
                .collect::<Vec<_>>(),
            [(0, "Sol".to_owned()), (3, "Luz".to_owned())]
        );
        // Erasing the note takes its words with it.
        score.erase(a).unwrap();
        assert!(score.event(a).unwrap().notation.lyrics.is_empty());
        assert!(score.set_syllable(a, 0, sing("Sol")).is_err());
        score.validate().unwrap();
    }
}
