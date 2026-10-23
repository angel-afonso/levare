//! Chord symbols in letter names: a root, a quality and an optional bass,
//! written above the staff from a note or rest on. Like the notes, they are
//! stored as the player reads them; playback applies the transposition.
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ChordQuality {
    Major,
    Minor,
    Augmented,
    Diminished,
    Sus2,
    Sus4,
    Power,
    Major6,
    Minor6,
    Dominant7,
    Major7,
    Minor7,
    MinorMajor7,
    HalfDiminished,
    Diminished7,
    Dominant7Sus4,
    Add9,
    Dominant9,
    Major9,
    Minor9,
    Dominant7Flat9,
    Dominant7Sharp9,
    Dominant11,
    Dominant13,
}

impl ChordQuality {
    pub const ALL: [Self; 24] = [
        Self::Major,
        Self::Minor,
        Self::Augmented,
        Self::Diminished,
        Self::Sus2,
        Self::Sus4,
        Self::Power,
        Self::Major6,
        Self::Minor6,
        Self::Dominant7,
        Self::Major7,
        Self::Minor7,
        Self::MinorMajor7,
        Self::HalfDiminished,
        Self::Diminished7,
        Self::Dominant7Sus4,
        Self::Add9,
        Self::Dominant9,
        Self::Major9,
        Self::Minor9,
        Self::Dominant7Flat9,
        Self::Dominant7Sharp9,
        Self::Dominant11,
        Self::Dominant13,
    ];

    /// Written after the root, with ♭ and ♯ as music signs.
    pub const fn suffix(self) -> &'static str {
        match self {
            Self::Major => "",
            Self::Minor => "m",
            Self::Augmented => "+",
            Self::Diminished => "dim",
            Self::Sus2 => "sus2",
            Self::Sus4 => "sus4",
            Self::Power => "5",
            Self::Major6 => "6",
            Self::Minor6 => "m6",
            Self::Dominant7 => "7",
            Self::Major7 => "maj7",
            Self::Minor7 => "m7",
            Self::MinorMajor7 => "m(maj7)",
            Self::HalfDiminished => "m7♭5",
            Self::Diminished7 => "dim7",
            Self::Dominant7Sus4 => "7sus4",
            Self::Add9 => "add9",
            Self::Dominant9 => "9",
            Self::Major9 => "maj9",
            Self::Minor9 => "m9",
            Self::Dominant7Flat9 => "7(♭9)",
            Self::Dominant7Sharp9 => "7(♯9)",
            Self::Dominant11 => "11",
            Self::Dominant13 => "13",
        }
    }

    /// Semitones above the root that sound.
    pub const fn intervals(self) -> &'static [u8] {
        match self {
            Self::Major => &[0, 4, 7],
            Self::Minor => &[0, 3, 7],
            Self::Augmented => &[0, 4, 8],
            Self::Diminished => &[0, 3, 6],
            Self::Sus2 => &[0, 2, 7],
            Self::Sus4 => &[0, 5, 7],
            Self::Power => &[0, 7],
            Self::Major6 => &[0, 4, 7, 9],
            Self::Minor6 => &[0, 3, 7, 9],
            Self::Dominant7 => &[0, 4, 7, 10],
            Self::Major7 => &[0, 4, 7, 11],
            Self::Minor7 => &[0, 3, 7, 10],
            Self::MinorMajor7 => &[0, 3, 7, 11],
            Self::HalfDiminished => &[0, 3, 6, 10],
            Self::Diminished7 => &[0, 3, 6, 9],
            Self::Dominant7Sus4 => &[0, 5, 7, 10],
            Self::Add9 => &[0, 4, 7, 14],
            Self::Dominant9 => &[0, 4, 7, 10, 14],
            Self::Major9 => &[0, 4, 7, 11, 14],
            Self::Minor9 => &[0, 3, 7, 10, 14],
            Self::Dominant7Flat9 => &[0, 4, 7, 10, 13],
            Self::Dominant7Sharp9 => &[0, 4, 7, 10, 15],
            // The third clashes with the eleventh; the fifth makes room for the thirteenth.
            Self::Dominant11 => &[0, 7, 10, 14, 17],
            Self::Dominant13 => &[0, 4, 10, 14, 21],
        }
    }

    /// MusicXML `kind` value, with the added degree (value, alteration)
    /// that completes it.
    pub const fn musicxml(self) -> (&'static str, Option<(u8, i8)>) {
        match self {
            Self::Major => ("major", None),
            Self::Minor => ("minor", None),
            Self::Augmented => ("augmented", None),
            Self::Diminished => ("diminished", None),
            Self::Sus2 => ("suspended-second", None),
            Self::Sus4 => ("suspended-fourth", None),
            Self::Power => ("power", None),
            Self::Major6 => ("major-sixth", None),
            Self::Minor6 => ("minor-sixth", None),
            Self::Dominant7 => ("dominant", None),
            Self::Major7 => ("major-seventh", None),
            Self::Minor7 => ("minor-seventh", None),
            Self::MinorMajor7 => ("major-minor", None),
            Self::HalfDiminished => ("half-diminished", None),
            Self::Diminished7 => ("diminished-seventh", None),
            Self::Dominant7Sus4 => ("suspended-fourth", Some((7, -1))),
            Self::Add9 => ("major", Some((9, 0))),
            Self::Dominant9 => ("dominant-ninth", None),
            Self::Major9 => ("major-ninth", None),
            Self::Minor9 => ("minor-ninth", None),
            Self::Dominant7Flat9 => ("dominant", Some((9, -1))),
            Self::Dominant7Sharp9 => ("dominant", Some((9, 1))),
            Self::Dominant11 => ("dominant-11th", None),
            Self::Dominant13 => ("dominant-13th", None),
        }
    }

    /// The quality for a MusicXML `kind`, its added degree and printed text.
    /// The text wins when it names a known suffix.
    pub fn from_musicxml(kind: &str, degree: Option<(u8, i8)>, text: Option<&str>) -> Option<Self> {
        if let Some(quality) = text.and_then(Self::parse) {
            return Some(quality);
        }
        Self::ALL
            .into_iter()
            .find(|quality| quality.musicxml() == (kind, degree))
            .or_else(|| {
                Self::ALL
                    .into_iter()
                    .find(|quality| quality.musicxml() == (kind, None))
            })
            .or(match kind {
                "augmented-seventh" => Some(Self::Dominant7),
                "major-11th" | "major-13th" => Some(Self::Major9),
                "minor-11th" | "minor-13th" => Some(Self::Minor9),
                _ => None,
            })
    }

    /// The suffix as it is typed, in any common spelling.
    fn parse(suffix: &str) -> Option<Self> {
        let suffix: String = suffix
            .chars()
            .filter(|c| !matches!(c, '(' | ')' | ' '))
            .map(|c| match c {
                '♭' => 'b',
                '♯' => '#',
                'º' => '°',
                other => other,
            })
            .collect();
        Some(match suffix.as_str() {
            "" | "maj" | "M" | "ma" => Self::Major,
            "m" | "min" | "-" | "mi" => Self::Minor,
            "+" | "aug" | "+5" | "#5" => Self::Augmented,
            "dim" | "°" | "o" => Self::Diminished,
            "sus2" | "2" => Self::Sus2,
            "sus4" | "sus" | "4" => Self::Sus4,
            "5" => Self::Power,
            "6" | "maj6" | "M6" => Self::Major6,
            "m6" | "min6" | "-6" => Self::Minor6,
            "7" | "dom7" => Self::Dominant7,
            "maj7" | "M7" | "Δ" | "Δ7" | "ma7" | "j7" => Self::Major7,
            "m7" | "min7" | "-7" | "mi7" => Self::Minor7,
            "mmaj7" | "mM7" | "minmaj7" | "-maj7" | "-Δ7" | "mΔ7" => Self::MinorMajor7,
            "m7b5" | "ø" | "ø7" | "-7b5" | "min7b5" => Self::HalfDiminished,
            "dim7" | "°7" | "o7" => Self::Diminished7,
            "7sus4" | "7sus" => Self::Dominant7Sus4,
            "add9" | "add2" => Self::Add9,
            "9" => Self::Dominant9,
            "maj9" | "M9" | "Δ9" => Self::Major9,
            "m9" | "min9" | "-9" => Self::Minor9,
            "7b9" => Self::Dominant7Flat9,
            "7#9" => Self::Dominant7Sharp9,
            "11" => Self::Dominant11,
            "13" => Self::Dominant13,
            _ => return None,
        })
    }
}

/// A root or bass: a letter with its alteration, from −1 to +1.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChordRoot {
    pub step: Step,
    pub alteration: i8,
}

impl ChordRoot {
    /// Every reading of `text` as a letter (C…B) or a solfège name (Do…Si)
    /// followed by ♭/b or ♯/#, with what follows it: «Fadd9» is F add 9,
    /// never Fa «dd9».
    fn parse(text: &str) -> impl Iterator<Item = (Self, &str)> {
        const NAMES: [(&str, Step); 14] = [
            ("sol", Step::G),
            ("do", Step::C),
            ("re", Step::D),
            ("mi", Step::E),
            ("fa", Step::F),
            ("la", Step::A),
            ("si", Step::B),
            ("C", Step::C),
            ("D", Step::D),
            ("E", Step::E),
            ("F", Step::F),
            ("G", Step::G),
            ("A", Step::A),
            ("B", Step::B),
        ];
        NAMES.iter().filter_map(move |(name, step)| {
            let head = text.get(..name.len())?;
            let solfege = name.chars().all(|c| c.is_lowercase());
            let matches = if solfege {
                head.eq_ignore_ascii_case(name)
            } else {
                head == *name
            };
            if !matches {
                return None;
            }
            let rest = &text[name.len()..];
            let mut chars = rest.chars();
            let (alteration, rest) = match chars.next() {
                Some('#' | '♯') => (1, chars.as_str()),
                // «b» right after the root is a flat: «Bb7» is B flat seven.
                Some('b' | '♭') => (-1, chars.as_str()),
                _ => (0, rest),
            };
            Some((
                Self {
                    step: *step,
                    alteration,
                },
                rest,
            ))
        })
    }

    fn label(self) -> String {
        let letter = match self.step {
            Step::C => 'C',
            Step::D => 'D',
            Step::E => 'E',
            Step::F => 'F',
            Step::G => 'G',
            Step::A => 'A',
            Step::B => 'B',
        };
        let sign = match self.alteration {
            -1 => "♭",
            1 => "♯",
            _ => "",
        };
        format!("{letter}{sign}")
    }

    /// Pitch class from C.
    pub fn semitone(self) -> u8 {
        ([0i8, 2, 4, 5, 7, 9, 11][self.step.index() as usize] + self.alteration).rem_euclid(12)
            as u8
    }

    /// Moved by `steps` letters, keeping its alteration.
    fn shifted(self, steps: i16) -> Self {
        const STEPS: [Step; 7] = [
            Step::C,
            Step::D,
            Step::E,
            Step::F,
            Step::G,
            Step::A,
            Step::B,
        ];
        Self {
            step: STEPS[(self.step.index() + steps).rem_euclid(7) as usize],
            ..self
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChordSymbol {
    pub root: ChordRoot,
    pub quality: ChordQuality,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bass: Option<ChordRoot>,
}

impl ChordSymbol {
    /// «Cmaj7», «F#m7b5», «Bb7/D», «Lam», «Sol7(b9)».
    pub fn parse(text: &str) -> Result<Self, ScoreError> {
        let invalid = || {
            ScoreError(
                "Cifrado no reconocido: escribe la fundamental (C, F#, Bb o Do, Fa#, Sib) y el tipo (m, 7, maj7…)",
            )
        };
        let text = text.trim();
        let (chord, bass) = match text.rsplit_once('/') {
            Some((chord, bass)) => (chord, Some(bass.trim())),
            None => (text, None),
        };
        let (root, quality) = ChordRoot::parse(chord.trim())
            .find_map(|(root, suffix)| Some((root, ChordQuality::parse(suffix)?)))
            .ok_or_else(invalid)?;
        let bass = match bass {
            Some(bass) => Some(
                ChordRoot::parse(bass)
                    .find_map(|(bass, rest)| rest.is_empty().then_some(bass))
                    .ok_or_else(invalid)?,
            ),
            None => None,
        };
        Ok(Self {
            root,
            quality,
            bass,
        })
    }

    pub fn label(&self) -> String {
        let bass = self
            .bass
            .map(|bass| format!("/{}", bass.label()))
            .unwrap_or_default();
        format!("{}{}{bass}", self.root.label(), self.quality.suffix())
    }

    /// Keys as written: the bass low, the chord in close position above it.
    pub fn keys(&self) -> Vec<u8> {
        let bass = self.bass.unwrap_or(self.root).semitone();
        let low = 36 + bass + if bass < 4 { 12 } else { 0 };
        let root = 48 + self.root.semitone();
        let root = if root < 52 { root + 12 } else { root };
        std::iter::once(low)
            .chain(self.quality.intervals().iter().map(|i| root + i))
            .collect()
    }

    fn shifted(self, steps: i16) -> Self {
        Self {
            root: self.root.shifted(steps),
            bass: self.bass.map(|bass| bass.shifted(steps)),
            ..self
        }
    }

    /// The triads on each degree of `key`, then the dominant seventh.
    pub fn diatonic(key: KeySignature) -> Vec<Self> {
        use ChordQuality::*;
        let (offset, qualities) = match key.mode() {
            KeyMode::Major => (0, [Major, Minor, Minor, Major, Major, Minor, Diminished]),
            KeyMode::Minor => (5, [Minor, Diminished, Major, Minor, Minor, Major, Major]),
        };
        let tonic = (4 * i16::from(key.fifths()) + offset).rem_euclid(7);
        let root = |degree: i16| {
            let step = ChordRoot {
                step: Step::C,
                alteration: 0,
            }
            .shifted(tonic + degree)
            .step;
            ChordRoot {
                step,
                alteration: match key.accidental(step) {
                    Some(Accidental::Sharp) => 1,
                    Some(Accidental::Flat) => -1,
                    _ => 0,
                },
            }
        };
        (0..7)
            .map(|degree| Self {
                root: root(degree),
                quality: qualities[degree as usize],
                bass: None,
            })
            .chain(std::iter::once(Self {
                root: root(4),
                quality: Dominant7,
                bass: None,
            }))
            .collect()
    }
}

impl Score {
    pub fn chords_sound(&self) -> bool {
        !self.silent_chords
    }
    /// Whether chord symbols play; returns whether that changed.
    pub fn set_chords_sound(&mut self, sound: bool) -> bool {
        let changed = self.silent_chords == sound;
        self.silent_chords = !sound;
        changed
    }

    /// Write a chord symbol at the note or rest `id`, or remove it.
    pub fn set_chord(
        &mut self,
        id: ElementId,
        chord: Option<ChordSymbol>,
    ) -> Result<(), ScoreError> {
        self.event(id)
            .ok_or(ScoreError("Selecciona una nota o silencio"))?;
        let notation = &mut Arc::make_mut(
            &mut self.part_mut(id.part)?.staff_mut(id.staff)?.measures[id.measure].events,
        )[id.event]
            .notation;
        notation.chord = chord;
        if chord.is_none() {
            notation.offsets.chord = None;
        }
        Ok(())
    }

    /// Move the chord symbol at `id` with its note, letter by letter.
    pub(crate) fn transpose_chord(&mut self, id: ElementId, steps: i16) -> Result<(), ScoreError> {
        if let Some(chord) = self.event(id).and_then(|event| event.notation.chord) {
            self.set_chord(id, Some(chord.shifted(steps)))?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(text: &str) -> ChordSymbol {
        ChordSymbol::parse(text).unwrap()
    }

    #[test]
    fn symbols_parse_in_letters_or_solfege_and_print_in_letters() {
        for (typed, printed) in [
            ("C", "C"),
            ("F#m7", "F♯m7"),
            ("Bb7/D", "B♭7/D"),
            ("Ebm7b5", "E♭m7♭5"),
            ("Gsus", "Gsus4"),
            ("A-7", "Am7"),
            ("DΔ7", "Dmaj7"),
            ("Lam", "Am"),
            ("Sib maj7", "B♭maj7"),
            ("sol7(b9)", "G7(♭9)"),
            ("C°7", "Cdim7"),
            ("Dø", "Dm7♭5"),
            ("Fadd9", "Fadd9"),
            ("Faug", "F+"),
            ("Fa", "F"),
        ] {
            assert_eq!(parse(typed).label(), printed, "{typed}");
        }
        for wrong in ["", "H7", "Cxyz", "C/", "C/Q", "7"] {
            assert!(ChordSymbol::parse(wrong).is_err(), "{wrong}");
        }
    }

    #[test]
    fn suggestions_follow_the_key() {
        let labels = |key| {
            ChordSymbol::diatonic(key)
                .iter()
                .map(ChordSymbol::label)
                .collect::<Vec<_>>()
        };
        assert_eq!(
            labels(KeySignature::default()),
            ["C", "Dm", "Em", "F", "G", "Am", "Bdim", "G7"]
        );
        assert_eq!(
            labels(KeySignature::new(-3, KeyMode::Minor).unwrap()),
            ["Cm", "Ddim", "E♭", "Fm", "Gm", "A♭", "B♭", "G7"]
        );
        assert_eq!(
            labels(KeySignature::new(2, KeyMode::Major).unwrap())[6],
            "C♯dim"
        );
    }

    #[test]
    fn a_symbol_sounds_its_bass_low_and_its_chord_above() {
        assert_eq!(parse("C").keys(), [48, 60, 64, 67]);
        assert_eq!(parse("G7/B").keys(), [47, 55, 59, 62, 65]);
        assert_eq!(parse("Am7").keys(), [45, 57, 60, 64, 67]);
    }

    #[test]
    fn chords_sit_on_notes_or_rests_and_move_with_a_transposition() {
        let mut score = Score::default();
        let id = score
            .insert(
                0,
                0,
                Event {
                    pitches: Pitches::single(Pitch::new(Step::C, 4)),
                    duration: Duration::Quarter,
                    notation: Notation::default(),
                },
            )
            .unwrap();
        score.set_chord(id, Some(parse("C/E"))).unwrap();
        score.transpose(id, 1).unwrap();
        let chord = score.event(id).unwrap().notation.chord.unwrap();
        assert_eq!(chord.label(), "D/F");
        score.erase(id).unwrap();
        assert!(score.event(id).unwrap().notation.chord.is_some());
        score.validate().unwrap();
    }
}
