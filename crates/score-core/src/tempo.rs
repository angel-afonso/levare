//! Written conductor tempo, shared by every instrument from a bar onward.
use super::*;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum TempoBeat {
    Half,
    #[default]
    Quarter,
    Eighth,
    DottedHalf,
    DottedQuarter,
    DottedEighth,
}
impl TempoBeat {
    pub const ALL: [Self; 6] = [
        Self::Half,
        Self::Quarter,
        Self::Eighth,
        Self::DottedHalf,
        Self::DottedQuarter,
        Self::DottedEighth,
    ];
    pub fn ticks(self) -> u16 {
        match self {
            Self::Half => 96,
            Self::Quarter => 48,
            Self::Eighth => 24,
            Self::DottedHalf => 144,
            Self::DottedQuarter => 72,
            Self::DottedEighth => 36,
        }
    }
    pub fn glyph(self) -> &'static str {
        match self {
            Self::Half => "\u{e1d3}",
            Self::Quarter => "\u{e1d5}",
            Self::Eighth => "\u{e1d7}",
            Self::DottedHalf => "\u{e1d3}\u{e1e7}",
            Self::DottedQuarter => "\u{e1d5}\u{e1e7}",
            Self::DottedEighth => "\u{e1d7}\u{e1e7}",
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Half => "Blanca",
            Self::Quarter => "Negra",
            Self::Eighth => "Corchea",
            Self::DottedHalf => "Blanca con puntillo",
            Self::DottedQuarter => "Negra con puntillo",
            Self::DottedEighth => "Corchea con puntillo",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TempoMark {
    pub bpm: u16,
    #[serde(default)]
    pub beat: TempoBeat,
    #[serde(default)]
    pub text: String,
}
impl Default for TempoMark {
    fn default() -> Self {
        Self {
            bpm: 120,
            beat: TempoBeat::Quarter,
            text: String::new(),
        }
    }
}
impl TempoMark {
    pub fn validate(&self) -> Result<(), ScoreError> {
        if !(20..=400).contains(&self.bpm)
            || self.text.chars().count() > 80
            || self.text.chars().any(char::is_control)
        {
            return Err(ScoreError(
                "Tempo inválido: usa de 20 a 400 pulsos por minuto y un texto de hasta 80 caracteres",
            ));
        }
        Ok(())
    }
    pub fn label(&self) -> String {
        format!(
            "{}{}{} = {}",
            self.text,
            if self.text.is_empty() { "" } else { "  " },
            self.beat.glyph(),
            self.bpm
        )
    }
}
impl Score {
    pub fn tempo_at(&self, measure: usize) -> &TempoMark {
        self.tempo_changes
            .range(..=measure)
            .next_back()
            .map_or(&self.tempo, |(_, mark)| mark)
    }
    pub fn tempo_change_at(&self, measure: usize) -> Option<&TempoMark> {
        if measure == 0 {
            Some(&self.tempo)
        } else {
            self.tempo_changes.get(&measure)
        }
    }
    pub fn tempo_marks(&self) -> impl Iterator<Item = (usize, &TempoMark)> {
        std::iter::once((0, &self.tempo))
            .chain(self.tempo_changes.iter().map(|(&bar, mark)| (bar, mark)))
    }
    pub fn set_tempo_at(&mut self, measure: usize, mark: TempoMark) -> Result<bool, ScoreError> {
        mark.validate()?;
        if measure >= self.measures().len() {
            return Err(ScoreError("El compás no existe"));
        }
        if self.tempo_at(measure) == &mark {
            return Ok(false);
        }
        if measure == 0 {
            self.tempo = mark;
        } else {
            self.tempo_changes.insert(measure, mark);
        }
        Ok(true)
    }
    pub fn remove_tempo_change(&mut self, measure: usize) -> bool {
        self.tempo_changes.remove(&measure).is_some()
    }
}
