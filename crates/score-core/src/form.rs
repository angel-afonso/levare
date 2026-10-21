//! Musical form is stored once; playback expands it into performed bars.
use super::*;
use std::collections::BTreeMap;

pub const MAX_REPEAT_TIMES: u8 = 8;
pub const MAX_ENDING_NUMBER: u8 = 8;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Barline {
    #[default]
    Single,
    Double,
    Final,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Jump {
    DaCapo,
    DaCapoAlFine,
    DaCapoAlCoda,
    DalSegno,
    DalSegnoAlFine,
    DalSegnoAlCoda,
}
impl Jump {
    pub const ALL: [Self; 6] = [
        Self::DaCapo,
        Self::DaCapoAlFine,
        Self::DaCapoAlCoda,
        Self::DalSegno,
        Self::DalSegnoAlFine,
        Self::DalSegnoAlCoda,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::DaCapo => "D.C.",
            Self::DaCapoAlFine => "D.C. al Fine",
            Self::DaCapoAlCoda => "D.C. al Coda",
            Self::DalSegno => "D.S.",
            Self::DalSegnoAlFine => "D.S. al Fine",
            Self::DalSegnoAlCoda => "D.S. al Coda",
        }
    }
    pub fn from_segno(self) -> bool {
        matches!(
            self,
            Self::DalSegno | Self::DalSegnoAlFine | Self::DalSegnoAlCoda
        )
    }
    pub fn ends_at_fine(self) -> bool {
        matches!(self, Self::DaCapoAlFine | Self::DalSegnoAlFine)
    }
    pub fn goes_to_coda(self) -> bool {
        matches!(self, Self::DaCapoAlCoda | Self::DalSegnoAlCoda)
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct BarForm {
    pub repeat_start: bool,
    pub repeat_end: Option<u8>,
    pub barline: Barline,
    pub segno: bool,
    pub coda: bool,
    pub to_coda: bool,
    pub fine: bool,
    pub jump: Option<Jump>,
}
impl BarForm {
    pub fn is_plain(&self) -> bool {
        *self == Self::default()
    }
    fn validate(&self) -> Result<(), ScoreError> {
        if self
            .repeat_end
            .is_some_and(|n| !(2..=MAX_REPEAT_TIMES).contains(&n))
        {
            return Err(ScoreError("Repetición o casilla inválida"));
        }
        Ok(())
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ending {
    pub start: usize,
    pub end: usize,
    pub numbers: Vec<u8>,
}
impl Ending {
    pub fn label(&self) -> String {
        self.numbers
            .iter()
            .map(|n| format!("{n}."))
            .collect::<Vec<_>>()
            .join(", ")
    }
    fn validate(&self) -> Result<(), ScoreError> {
        if self.start > self.end
            || self.numbers.is_empty()
            || self
                .numbers
                .iter()
                .any(|n| !(1..=MAX_ENDING_NUMBER).contains(n))
            || self.numbers.windows(2).any(|w| w[0] >= w[1])
        {
            return Err(ScoreError("Repetición o casilla inválida"));
        }
        Ok(())
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlayedBar {
    pub bar: usize,
    pub pass: u8,
}
impl Score {
    pub fn bar_form(&self, bar: usize) -> BarForm {
        self.form.get(&bar).copied().unwrap_or_default()
    }
    pub fn set_bar_form(&mut self, bar: usize, form: BarForm) -> Result<bool, ScoreError> {
        if bar >= self.measures().len() {
            return Err(ScoreError("El compás no existe"));
        }
        form.validate()?;
        if self.bar_form(bar) == form {
            return Ok(false);
        }
        if form.is_plain() {
            self.form.remove(&bar);
        } else {
            self.form.insert(bar, form);
        }
        Ok(true)
    }
    pub fn endings(&self) -> impl Iterator<Item = &Ending> {
        self.endings.iter()
    }
    pub fn ending_at(&self, bar: usize) -> Option<&Ending> {
        self.endings.iter().find(|e| e.start <= bar && bar <= e.end)
    }
    pub fn set_ending(&mut self, ending: Ending) -> Result<bool, ScoreError> {
        ending.validate()?;
        if ending.end >= self.measures().len() {
            return Err(ScoreError("El compás no existe"));
        }
        if self.endings.iter().any(|e| e == &ending) {
            return Ok(false);
        }
        self.remove_endings(ending.start, ending.end);
        let mut endings: Vec<_> = self.endings.iter().cloned().collect();
        endings.push(ending);
        endings.sort_by_key(|e| e.start);
        self.endings = endings.into_iter().collect();
        Ok(true)
    }
    pub fn remove_endings(&mut self, start: usize, end: usize) -> bool {
        let old = self.endings.len();
        self.endings = self
            .endings
            .iter()
            .filter(|e| e.end < start || e.start > end)
            .cloned()
            .collect();
        self.endings.len() != old
    }
    pub fn is_last_ending(&self, ending: &Ending) -> bool {
        !self
            .endings
            .iter()
            .any(|e| ending.end.checked_add(1) == Some(e.start))
    }
    pub(crate) fn validate_form(&self) -> Result<(), ScoreError> {
        for (&bar, form) in &self.form {
            if bar >= self.measures().len() {
                return Err(ScoreError("Repetición o casilla inválida"));
            }
            form.validate()?;
        }
        let mut previous = None;
        for e in &self.endings {
            e.validate()?;
            if e.end >= self.measures().len() || previous.is_some_and(|end| e.start <= end) {
                return Err(ScoreError("Repetición o casilla inválida"));
            }
            previous = Some(e.end);
        }
        Ok(())
    }
    fn repeat_target(&self, bar: usize) -> usize {
        for b in (0..=bar).rev() {
            let f = self.bar_form(b);
            if f.repeat_start {
                return b;
            }
            if b < bar && (f.repeat_end.is_some() || self.endings.iter().any(|e| e.end == b)) {
                return b + 1;
            }
        }
        0
    }
    pub fn play_order(&self, follow_repeats: bool) -> Vec<PlayedBar> {
        let len = self.measures().len();
        if !follow_repeats {
            return (0..len).map(|bar| PlayedBar { bar, pass: 1 }).collect();
        }
        let (mut bar, mut pass, mut jumped) = (0, 1, None::<Jump>);
        let mut taken = BTreeMap::<usize, u8>::new();
        let mut order = Vec::new();
        while bar < len && order.len() < len.saturating_mul(16) {
            if let Some(e) = self.ending_at(bar).filter(|e| e.start == bar)
                && !(if jumped.is_some() {
                    self.is_last_ending(e)
                } else {
                    e.numbers.contains(&pass)
                })
            {
                bar = e.end + 1;
                continue;
            }
            order.push(PlayedBar { bar, pass });
            let f = self.bar_form(bar);
            if let Some(j) = jumped {
                if f.fine && j.ends_at_fine() {
                    break;
                }
                if f.to_coda
                    && j.goes_to_coda()
                    && let Some(target) = (bar + 1..len).find(|&b| self.bar_form(b).coda)
                {
                    bar = target;
                    continue;
                }
            }
            if let Some(times) = f.repeat_end.filter(|_| jumped.is_none()) {
                let count = taken.entry(bar).or_default();
                if *count + 1 < times {
                    *count += 1;
                    pass += 1;
                    bar = self.repeat_target(bar);
                    continue;
                }
                pass = 1;
            }
            if let Some(j) = f.jump.filter(|_| jumped.is_none()) {
                let target = if j.from_segno() {
                    (0..len).find(|&b| self.bar_form(b).segno)
                } else {
                    Some(0)
                };
                if let Some(target) = target {
                    jumped = Some(j);
                    pass = 1;
                    bar = target;
                    continue;
                }
            }
            let last_ending = self
                .endings
                .iter()
                .any(|e| e.end == bar && self.is_last_ending(e));
            bar += 1;
            if last_ending || self.bar_form(bar).repeat_start {
                pass = 1;
            }
        }
        order
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn score() -> Score {
        Score::new("t".into(), &[Instrument::Piano], 6).unwrap()
    }
    fn bars(s: &Score) -> Vec<usize> {
        s.play_order(true).iter().map(|p| p.bar).collect()
    }
    fn mark(s: &mut Score, bar: usize, f: BarForm) {
        s.set_bar_form(bar, f).unwrap();
    }
    fn endings(s: &mut Score) {
        mark(
            s,
            3,
            BarForm {
                repeat_end: Some(2),
                ..Default::default()
            },
        );
        s.set_ending(Ending {
            start: 3,
            end: 3,
            numbers: vec![1],
        })
        .unwrap();
        s.set_ending(Ending {
            start: 4,
            end: 4,
            numbers: vec![2],
        })
        .unwrap();
    }
    #[test]
    fn plain_score_plays_every_bar_once() {
        assert_eq!(bars(&score()), vec![0, 1, 2, 3, 4, 5]);
    }
    #[test]
    fn repeat_without_start_returns_to_the_beginning() {
        let mut s = score();
        mark(
            &mut s,
            1,
            BarForm {
                repeat_end: Some(2),
                ..Default::default()
            },
        );
        assert_eq!(bars(&s), vec![0, 1, 0, 1, 2, 3, 4, 5]);
    }
    #[test]
    fn repeat_start_and_three_times() {
        let mut s = score();
        mark(
            &mut s,
            2,
            BarForm {
                repeat_start: true,
                ..Default::default()
            },
        );
        mark(
            &mut s,
            3,
            BarForm {
                repeat_end: Some(3),
                ..Default::default()
            },
        );
        assert_eq!(bars(&s), vec![0, 1, 2, 3, 2, 3, 2, 3, 4, 5]);
    }
    #[test]
    fn first_and_second_endings() {
        let mut s = score();
        endings(&mut s);
        assert_eq!(bars(&s), vec![0, 1, 2, 3, 0, 1, 2, 4, 5]);
        assert_eq!(s.play_order(true)[7].pass, 2);
    }
    #[test]
    fn da_capo_al_fine() {
        let mut s = score();
        mark(
            &mut s,
            1,
            BarForm {
                fine: true,
                ..Default::default()
            },
        );
        mark(
            &mut s,
            3,
            BarForm {
                jump: Some(Jump::DaCapoAlFine),
                ..Default::default()
            },
        );
        assert_eq!(bars(&s), vec![0, 1, 2, 3, 0, 1]);
    }
    #[test]
    fn dal_segno_al_coda() {
        let mut s = score();
        mark(
            &mut s,
            1,
            BarForm {
                segno: true,
                ..Default::default()
            },
        );
        mark(
            &mut s,
            2,
            BarForm {
                to_coda: true,
                ..Default::default()
            },
        );
        mark(
            &mut s,
            3,
            BarForm {
                jump: Some(Jump::DalSegnoAlCoda),
                ..Default::default()
            },
        );
        mark(
            &mut s,
            4,
            BarForm {
                coda: true,
                ..Default::default()
            },
        );
        assert_eq!(bars(&s), vec![0, 1, 2, 3, 1, 2, 4, 5]);
    }
    #[test]
    fn repeats_are_not_taken_after_a_jump_and_the_last_ending_plays() {
        let mut s = score();
        endings(&mut s);
        mark(
            &mut s,
            5,
            BarForm {
                jump: Some(Jump::DaCapo),
                ..Default::default()
            },
        );
        assert_eq!(bars(&s), vec![0, 1, 2, 3, 0, 1, 2, 4, 5, 0, 1, 2, 4, 5]);
    }
    #[test]
    fn dal_segno_without_segno_is_ignored() {
        let mut s = score();
        mark(
            &mut s,
            2,
            BarForm {
                jump: Some(Jump::DalSegno),
                ..Default::default()
            },
        );
        assert_eq!(bars(&s), vec![0, 1, 2, 3, 4, 5]);
    }
    #[test]
    fn second_repeated_section_restarts_its_passes() {
        let mut s = Score::new("t".into(), &[Instrument::Piano], 10).unwrap();
        for (start, end) in [(0, 3), (5, 8)] {
            mark(
                &mut s,
                start,
                BarForm {
                    repeat_start: true,
                    ..Default::default()
                },
            );
            mark(
                &mut s,
                end,
                BarForm {
                    repeat_end: Some(2),
                    ..Default::default()
                },
            );
            for (bar, n) in [(end, 1), (end + 1, 2)] {
                s.set_ending(Ending {
                    start: bar,
                    end: bar,
                    numbers: vec![n],
                })
                .unwrap();
            }
        }
        assert_eq!(
            bars(&s),
            vec![0, 1, 2, 3, 0, 1, 2, 4, 5, 6, 7, 8, 5, 6, 7, 9]
        );
    }
    #[test]
    fn follow_repeats_false_is_linear() {
        let mut s = score();
        endings(&mut s);
        assert_eq!(
            s.play_order(false)
                .iter()
                .map(|p| p.bar)
                .collect::<Vec<_>>(),
            vec![0, 1, 2, 3, 4, 5]
        );
    }
    #[test]
    fn set_bar_form_and_endings_validate() {
        let mut s = score();
        assert!(
            s.set_bar_form(
                1,
                BarForm {
                    repeat_end: Some(1),
                    ..Default::default()
                }
            )
            .is_err()
        );
        assert!(s.set_bar_form(6, BarForm::default()).is_err());
        for numbers in [vec![], vec![0], vec![2, 1], vec![9]] {
            assert!(
                s.set_ending(Ending {
                    start: 1,
                    end: 2,
                    numbers
                })
                .is_err()
            );
        }
        for (start, end) in [(1, 3), (2, 4)] {
            s.set_ending(Ending {
                start,
                end,
                numbers: vec![1],
            })
            .unwrap();
        }
        assert_eq!(s.endings().count(), 1);
        assert_eq!(s.endings().next().unwrap().start, 2);
        s.validate().unwrap();
    }
    #[test]
    fn structural_edits_keep_form_with_its_bars() {
        let mut s = score();
        mark(
            &mut s,
            2,
            BarForm {
                segno: true,
                ..Default::default()
            },
        );
        mark(
            &mut s,
            4,
            BarForm {
                fine: true,
                ..Default::default()
            },
        );
        s.set_ending(Ending {
            start: 2,
            end: 3,
            numbers: vec![1],
        })
        .unwrap();
        s.insert_measure(0).unwrap();
        assert!(s.bar_form(3).segno);
        s.insert_measure(4).unwrap();
        assert_eq!(
            (
                s.endings().next().unwrap().start,
                s.endings().next().unwrap().end
            ),
            (3, 5)
        );
        s.remove_measures(3, 3).unwrap();
        assert_eq!(s.endings().count(), 0);
        assert!(s.bar_form(3).fine);
        s.validate().unwrap();
    }
}
