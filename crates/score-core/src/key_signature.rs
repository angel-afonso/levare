use crate::{Accidental, Step};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum KeyMode {
    #[default]
    Major,
    Minor,
}

/// A conventional global signature: up to seven sharps or seven flats.
/// Mode names the relative major/minor; it does not add harmonic-minor accidentals.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "KeySignatureData")]
pub struct KeySignature {
    fifths: i8,
    mode: KeyMode,
}

#[derive(Deserialize)]
struct KeySignatureData {
    fifths: i8,
    mode: KeyMode,
}
impl TryFrom<KeySignatureData> for KeySignature {
    type Error = &'static str;
    fn try_from(data: KeySignatureData) -> Result<Self, Self::Error> {
        Self::new(data.fifths, data.mode).ok_or("Armadura fuera de rango (−7 a 7)")
    }
}

impl KeySignature {
    pub const SHARPS: [Step; 7] = [
        Step::F,
        Step::C,
        Step::G,
        Step::D,
        Step::A,
        Step::E,
        Step::B,
    ];
    pub const FLATS: [Step; 7] = [
        Step::B,
        Step::E,
        Step::A,
        Step::D,
        Step::G,
        Step::C,
        Step::F,
    ];

    pub fn new(fifths: i8, mode: KeyMode) -> Option<Self> {
        (-7..=7).contains(&fifths).then_some(Self { fifths, mode })
    }
    pub fn fifths(self) -> i8 {
        self.fifths
    }
    pub fn mode(self) -> KeyMode {
        self.mode
    }
    pub fn count(self) -> usize {
        self.fifths.unsigned_abs() as usize
    }
    pub fn steps(self) -> &'static [Step] {
        let order = if self.fifths < 0 {
            &Self::FLATS
        } else {
            &Self::SHARPS
        };
        &order[..self.count()]
    }
    pub fn accidental(self, step: Step) -> Option<Accidental> {
        self.steps().contains(&step).then_some(if self.fifths < 0 {
            Accidental::Flat
        } else {
            Accidental::Sharp
        })
    }
    pub fn label(self) -> &'static str {
        let names = match self.mode {
            KeyMode::Major => [
                "Do♭ mayor",
                "Sol♭ mayor",
                "Re♭ mayor",
                "La♭ mayor",
                "Mi♭ mayor",
                "Si♭ mayor",
                "Fa mayor",
                "Do mayor",
                "Sol mayor",
                "Re mayor",
                "La mayor",
                "Mi mayor",
                "Si mayor",
                "Fa♯ mayor",
                "Do♯ mayor",
            ],
            KeyMode::Minor => [
                "La♭ menor",
                "Mi♭ menor",
                "Si♭ menor",
                "Fa menor",
                "Do menor",
                "Sol menor",
                "Re menor",
                "La menor",
                "Mi menor",
                "Si menor",
                "Fa♯ menor",
                "Do♯ menor",
                "Sol♯ menor",
                "Re♯ menor",
                "La♯ menor",
            ],
        };
        names[(self.fifths + 7) as usize]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Pitch, Pitches};
    #[test]
    fn all_signatures_apply_in_every_octave_and_explicit_signs_override_them() {
        for fifths in -7i8..=7 {
            let major = KeySignature::new(fifths, KeyMode::Major).unwrap();
            let minor = KeySignature::new(fifths, KeyMode::Minor).unwrap();
            assert_eq!(major.steps(), minor.steps());
            assert_eq!(major.steps().len(), fifths.unsigned_abs() as usize);
            for step in [
                Step::C,
                Step::D,
                Step::E,
                Step::F,
                Step::G,
                Step::A,
                Step::B,
            ] {
                for octave in [0, 3, 4, 8] {
                    let pitch = Pitch::new(step, octave).unwrap();
                    assert_eq!(pitch.in_key(major).accidental(), major.accidental(step));
                    for sign in [Accidental::Natural, Accidental::Flat, Accidental::Sharp] {
                        let explicit = pitch.with_accidental(Some(sign));
                        assert_eq!(explicit.in_key(major), explicit);
                    }
                }
            }
        }
        assert!(KeySignature::new(-8, KeyMode::Major).is_none());
        assert!(KeySignature::new(8, KeyMode::Minor).is_none());
        assert_eq!(KeySignature::default().label(), "Do mayor");
        assert_eq!(
            KeySignature::new(1, KeyMode::Major).unwrap().label(),
            "Sol mayor"
        );
        assert_eq!(
            KeySignature::new(-3, KeyMode::Minor).unwrap().label(),
            "Do menor"
        );
        let chord = Pitches::new(&[
            Pitch::new(Step::F, 4).unwrap(),
            Pitch::new(Step::A, 4).unwrap(),
            Pitch::new(Step::C, 5).unwrap(),
        ])
        .unwrap();
        let resolved = chord.in_key(KeySignature::new(2, KeyMode::Major).unwrap());
        assert_eq!(
            resolved.iter().map(|p| p.to_string()).collect::<Vec<_>>(),
            ["F♯4", "A4", "C♯5"]
        );
        assert!(chord.iter().all(|p| p.accidental().is_none()));
    }
}
