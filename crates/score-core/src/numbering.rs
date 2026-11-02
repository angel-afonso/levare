//! Where bar and page numbers are printed. Both only label the music.
use super::*;

/// Which bars show their number, above the top staff of their system.
/// The first bar never does, as in Sibelius and MuseScore.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum BarNumbers {
    /// The first bar of every system.
    #[default]
    EverySystem,
    EveryBar,
    /// Every bar whose number is a multiple of this.
    Every(u8),
    Hidden,
}
impl BarNumbers {
    /// Whether bar `index` (from 0) shows its number when it is or isn't
    /// the first bar of its system.
    pub fn shows(self, index: usize, system_start: bool) -> bool {
        index > 0
            && match self {
                Self::EverySystem => system_start,
                Self::EveryBar => true,
                Self::Every(step) => (index + 1).is_multiple_of(usize::from(step.max(1))),
                Self::Hidden => false,
            }
    }
}

/// Where the page number goes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum PageNumbers {
    /// «2 / 5» centred at the foot of every page.
    #[default]
    Footer,
    /// The number alone at the top outside corner, from the second page on,
    /// as Sibelius prints it: right on odd pages, left on even ones.
    Corner,
    Hidden,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Numbering {
    pub bars: BarNumbers,
    pub pages: PageNumbers,
}
impl Numbering {
    pub(crate) fn is_default(&self) -> bool {
        *self == Self::default()
    }
}

impl Score {
    pub fn numbering(&self) -> Numbering {
        self.numbering
    }
    /// Whether anything changed.
    pub fn set_numbering(&mut self, numbering: Numbering) -> Result<bool, ScoreError> {
        if matches!(numbering.bars, BarNumbers::Every(step) if step < 2) {
            return Err(ScoreError("Numera cada dos compases o más"));
        }
        let changed = self.numbering != numbering;
        self.numbering = numbering;
        Ok(changed)
    }
}
