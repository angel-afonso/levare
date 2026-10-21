//! Where the user ends a system or a page: after a bar, never inside one.
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum LayoutBreak {
    System,
    Page,
}

impl Score {
    /// The break after `bar`, if the user asked for one.
    pub fn layout_break(&self, bar: usize) -> Option<LayoutBreak> {
        self.breaks.get(&bar).copied()
    }
    pub fn layout_breaks(&self) -> impl Iterator<Item = (usize, LayoutBreak)> + '_ {
        self.breaks.iter().map(|(&bar, &kind)| (bar, kind))
    }
    /// Same kind again removes it; the other kind replaces it.
    pub fn toggle_layout_break(
        &mut self,
        bar: usize,
        kind: LayoutBreak,
    ) -> Result<bool, ScoreError> {
        if bar >= self.measures().len() {
            return Err(ScoreError("El compás no existe"));
        }
        if self.breaks.get(&bar) == Some(&kind) {
            self.breaks.remove(&bar);
        } else {
            self.breaks.insert(bar, kind);
        }
        Ok(true)
    }
    pub(crate) fn validate_breaks(&self) -> Result<(), ScoreError> {
        if self.breaks.keys().any(|&bar| bar >= self.measures().len()) {
            return Err(ScoreError("Salto de disposición inválido"));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn score() -> Score {
        let mut s = Score::default();
        while s.measures().len() < 6 {
            s.insert_measure(s.measures().len()).unwrap();
        }
        s
    }
    #[test]
    fn same_kind_removes_other_replaces() {
        let mut s = score();
        s.toggle_layout_break(2, LayoutBreak::System).unwrap();
        assert_eq!(s.layout_break(2), Some(LayoutBreak::System));
        s.toggle_layout_break(2, LayoutBreak::Page).unwrap();
        assert_eq!(s.layout_break(2), Some(LayoutBreak::Page));
        s.toggle_layout_break(2, LayoutBreak::Page).unwrap();
        assert_eq!(s.layout_break(2), None);
    }
    #[test]
    fn missing_bar_is_an_error() {
        let mut s = score();
        assert!(s.toggle_layout_break(99, LayoutBreak::System).is_err());
    }
    #[test]
    fn breaks_follow_bar_edits() {
        let mut s = score();
        s.toggle_layout_break(2, LayoutBreak::System).unwrap();
        s.insert_measure(0).unwrap();
        assert_eq!(s.layout_break(3), Some(LayoutBreak::System));
        assert_eq!(s.layout_break(2), None);
        s.remove_measures(3, 1).unwrap();
        assert_eq!(s.layout_breaks().count(), 0);
    }
}
