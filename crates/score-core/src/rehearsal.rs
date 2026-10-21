//! Boxed rehearsal labels, numbered in written bar order.
use super::*;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum RehearsalStyle {
    #[default]
    Letters,
    Numbers,
    BarNumbers,
}
impl RehearsalStyle {
    pub const ALL: [Self; 3] = [Self::Letters, Self::Numbers, Self::BarNumbers];
    pub const fn label(self) -> &'static str {
        match self {
            Self::Letters => "Letras",
            Self::Numbers => "Números",
            Self::BarNumbers => "Números de compás",
        }
    }
}
pub(crate) fn rehearsal_style_is_default(style: &RehearsalStyle) -> bool {
    *style == RehearsalStyle::Letters
}
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct RehearsalMark {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom: Option<String>,
}
pub fn letter_label(index: usize) -> String {
    std::iter::repeat_n(char::from(b'A' + (index % 26) as u8), index / 26 + 1).collect()
}
fn valid_custom(text: &str) -> bool {
    !text.trim().is_empty() && text.chars().count() <= 16 && !text.chars().any(char::is_control)
}
impl Score {
    pub fn rehearsal_style(&self) -> RehearsalStyle {
        self.rehearsal_style
    }
    pub fn set_rehearsal_style(&mut self, style: RehearsalStyle) -> bool {
        if self.rehearsal_style == style {
            return false;
        }
        self.rehearsal_style = style;
        true
    }
    pub fn rehearsal_marks(&self) -> impl Iterator<Item = (usize, &RehearsalMark)> {
        self.rehearsals.iter().map(|(&bar, mark)| (bar, mark))
    }
    pub fn has_rehearsal_mark(&self, bar: usize) -> bool {
        self.rehearsals.contains_key(&bar)
    }
    pub fn toggle_rehearsal_mark(&mut self, bar: usize) -> Result<bool, ScoreError> {
        if bar >= self.measures().len() {
            return Err(ScoreError("El compás no existe"));
        }
        if self.rehearsals.remove(&bar).is_none() {
            self.rehearsals.insert(bar, RehearsalMark::default());
        }
        Ok(true)
    }
    pub fn set_rehearsal_custom(
        &mut self,
        bar: usize,
        custom: Option<String>,
    ) -> Result<bool, ScoreError> {
        if bar >= self.measures().len() {
            return Err(ScoreError("El compás no existe"));
        }
        if custom.as_deref().is_some_and(|text| !valid_custom(text)) {
            return Err(ScoreError("Marca de ensayo inválida"));
        }
        let mark = RehearsalMark { custom };
        if self.rehearsals.get(&bar) == Some(&mark) {
            return Ok(false);
        }
        self.rehearsals.insert(bar, mark);
        Ok(true)
    }
    pub fn rehearsal_label(&self, bar: usize) -> Option<String> {
        let mark = self.rehearsals.get(&bar)?;
        if let Some(text) = &mark.custom {
            return Some(text.clone());
        }
        let index = self.rehearsals.keys().take_while(|&&key| key < bar).count();
        Some(match self.rehearsal_style {
            RehearsalStyle::Letters => letter_label(index),
            RehearsalStyle::Numbers => (index + 1).to_string(),
            RehearsalStyle::BarNumbers => (bar + 1).to_string(),
        })
    }
    pub(crate) fn validate_rehearsals(&self) -> Result<(), ScoreError> {
        if self.rehearsals.iter().any(|(&bar, mark)| {
            bar >= self.measures().len()
                || mark
                    .custom
                    .as_deref()
                    .is_some_and(|text| !valid_custom(text))
        }) {
            return Err(ScoreError("Marca de ensayo inválida"));
        }
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn score() -> Score {
        let mut s = Score::default();
        while s.measures().len() < 10 {
            s.insert_measure(s.measures().len()).unwrap();
        }
        s
    }
    #[test]
    fn letters_run_a_to_z_then_double() {
        assert_eq!(
            [
                letter_label(0),
                letter_label(25),
                letter_label(26),
                letter_label(27)
            ],
            ["A", "Z", "AA", "BB"]
        );
    }
    #[test]
    fn labels_follow_bar_order_and_relabel() {
        let mut s = score();
        s.toggle_rehearsal_mark(4).unwrap();
        s.toggle_rehearsal_mark(8).unwrap();
        assert_eq!(s.rehearsal_label(8).as_deref(), Some("B"));
        s.toggle_rehearsal_mark(2).unwrap();
        assert_eq!(
            [
                s.rehearsal_label(2),
                s.rehearsal_label(4),
                s.rehearsal_label(8)
            ],
            [Some("A".into()), Some("B".into()), Some("C".into())]
        );
    }
    #[test]
    fn numbers_and_bar_numbers_styles() {
        let mut s = score();
        for b in [4, 8] {
            s.toggle_rehearsal_mark(b).unwrap();
        }
        s.set_rehearsal_style(RehearsalStyle::Numbers);
        assert_eq!(s.rehearsal_label(8).as_deref(), Some("2"));
        s.set_rehearsal_style(RehearsalStyle::BarNumbers);
        assert_eq!(
            [s.rehearsal_label(4), s.rehearsal_label(8)],
            [Some("5".into()), Some("9".into())]
        );
    }
    #[test]
    fn custom_text_overrides_the_sequence() {
        let mut s = score();
        s.set_rehearsal_custom(0, Some("Trío".into())).unwrap();
        s.toggle_rehearsal_mark(4).unwrap();
        assert_eq!(s.rehearsal_label(0).as_deref(), Some("Trío"));
        assert_eq!(s.rehearsal_label(4).as_deref(), Some("B"));
        for text in ["", " ", "bad\n", "abcdefghijklmnopq"] {
            assert!(s.set_rehearsal_custom(0, Some(text.into())).is_err());
        }
    }
    #[test]
    fn marks_move_with_inserted_and_removed_bars() {
        let mut s = score();
        s.toggle_rehearsal_mark(4).unwrap();
        s.insert_measure(2).unwrap();
        assert!(s.has_rehearsal_mark(5));
        s.remove_measures(2, 1).unwrap();
        assert!(s.has_rehearsal_mark(4));
    }
}
