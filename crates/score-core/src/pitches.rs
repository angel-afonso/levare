use crate::{Accidental, KeySignature, Pitch, ScoreError};
use serde::{
    Deserialize, Deserializer, Serialize, Serializer,
    de::{self, SeqAccess, Visitor},
};
use std::fmt;

pub const MAX_CHORD_NOTES: usize = 8;

/// A bounded, sorted set. Empty means rest; one pitch means a single note.
/// Inline storage keeps events cheap to copy and never allocates in audio code.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Pitches([Option<Pitch>; MAX_CHORD_NOTES]);

impl Pitches {
    pub const fn single(pitch: Option<Pitch>) -> Self {
        let mut pitches = [None; MAX_CHORD_NOTES];
        pitches[0] = pitch;
        Self(pitches)
    }
    pub fn new(pitches: &[Pitch]) -> Result<Self, ScoreError> {
        let mut set = Self::default();
        for pitch in pitches {
            set = set.with(*pitch)?;
        }
        Ok(set)
    }
    pub fn iter(&self) -> impl DoubleEndedIterator<Item = Pitch> + '_ {
        self.0.iter().flatten().copied()
    }
    pub fn len(&self) -> usize {
        self.iter().count()
    }
    pub fn is_empty(&self) -> bool {
        self.0[0].is_none()
    }
    pub fn first(&self) -> Option<Pitch> {
        self.0[0]
    }
    pub fn last(&self) -> Option<Pitch> {
        self.iter().last()
    }
    pub fn with(self, pitch: Pitch) -> Result<Self, ScoreError> {
        if pitch.octave() > 8 {
            return Err(ScoreError("Nota fuera de rango"));
        }
        if self
            .iter()
            .any(|existing| existing.diatonic() == pitch.diatonic())
        {
            return Err(ScoreError(
                "Ya hay una nota en esa posición del acorde; cambia su alteración",
            ));
        }
        let len = self.len();
        if len == MAX_CHORD_NOTES {
            return Err(ScoreError("Máximo de 8 notas por acorde"));
        }
        let mut result = self;
        result.0[len] = Some(pitch);
        result
            .0
            .sort_unstable_by_key(|pitch| pitch.map(|pitch| pitch.diatonic()).unwrap_or(i16::MAX));
        Ok(result)
    }
    pub fn in_key(mut self, key: KeySignature) -> Self {
        for pitch in self.0.iter_mut().flatten() {
            *pitch = pitch.in_key(key);
        }
        self
    }
    pub fn with_accidental(
        self,
        index: usize,
        accidental: Option<Accidental>,
    ) -> Result<Self, ScoreError> {
        let mut result = self;
        let pitch = result
            .0
            .get_mut(index)
            .and_then(Option::as_mut)
            .ok_or(ScoreError("Selecciona una nota del acorde"))?;
        *pitch = pitch.with_accidental(accidental);
        Ok(result)
    }
    pub fn without_highest(self) -> Result<Self, ScoreError> {
        let len = self.len();
        if len < 2 {
            return Err(ScoreError("El acorde debe conservar al menos una nota"));
        }
        let mut result = self;
        result.0[len - 1] = None;
        Ok(result)
    }
    pub fn transposed(self, steps: i16) -> Result<Self, ScoreError> {
        if self.is_empty() {
            return Err(ScoreError("Un silencio no se puede transponer"));
        }
        let mut result = Self::default();
        for (index, pitch) in self.iter().enumerate() {
            result.0[index] = Some(
                pitch
                    .shift(steps)
                    .ok_or(ScoreError("Nota fuera de rango (C0–B8)"))?,
            );
        }
        Ok(result)
    }
    pub fn validate(&self) -> Result<(), ScoreError> {
        if self.iter().any(|pitch| pitch.octave() > 8) {
            return Err(ScoreError("Nota fuera de rango"));
        }
        Ok(())
    }
}

impl Serialize for Pitches {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_seq(self.iter())
    }
}
impl<'de> Deserialize<'de> for Pitches {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct PitchVisitor;
        impl<'de> Visitor<'de> for PitchVisitor {
            type Value = Pitches;
            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("hasta 8 notas distintas")
            }
            fn visit_seq<A: SeqAccess<'de>>(
                self,
                mut sequence: A,
            ) -> Result<Self::Value, A::Error> {
                let mut pitches = Pitches::default();
                while let Some(pitch) = sequence.next_element::<Pitch>()? {
                    pitches = pitches.with(pitch).map_err(de::Error::custom)?;
                }
                Ok(pitches)
            }
        }
        deserializer.deserialize_seq(PitchVisitor)
    }
}
