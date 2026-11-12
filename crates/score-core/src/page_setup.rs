//! The paper a score is printed on and the size of its staves.
use super::*;

/// A paper size. `Automatic` keeps Levare's choice: A3 for full scores of
/// twelve parts or more, A4 otherwise.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Paper {
    #[default]
    Automatic,
    A4,
    Letter,
    A3,
    Tabloid,
}
impl Paper {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Automatic => "Automático",
            Self::A4 => "A4",
            Self::Letter => "Carta",
            Self::A3 => "A3",
            Self::Tabloid => "Tabloide",
        }
    }
    /// Portrait width and height in millimetres, after resolving `Automatic`.
    fn portrait(self, parts: usize, part: bool) -> (f32, f32) {
        match self {
            Self::Automatic if !part && parts >= 12 => Self::A3.portrait(parts, part),
            Self::Automatic | Self::A4 => (210., 297.),
            Self::Letter => (215.9, 279.4),
            Self::A3 => (297., 420.),
            Self::Tabloid => (279.4, 431.8),
        }
    }
}

/// Room around the music, added to Levare's own narrow margins.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Margins {
    #[default]
    Normal,
    Wide,
    ExtraWide,
}
impl Margins {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Normal => "Normales",
            Self::Wide => "Anchos",
            Self::ExtraWide => "Muy anchos",
        }
    }
    /// Millimetres added on every side of the page.
    pub const fn extra_mm(self) -> f32 {
        match self {
            Self::Normal => 0.,
            Self::Wide => 6.,
            Self::ExtraWide => 12.,
        }
    }
}

/// Staff heights Levare offers, in tenths of a millimetre, as Sibelius and
/// MuseScore offer theirs.
pub const STAFF_SIZES: [u8; 4] = [70, 60, 50, 40];

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct PageSetup {
    pub paper: Paper,
    pub landscape: bool,
    /// Staff height from top to bottom line in tenths of a millimetre;
    /// `None` lets the number of parts decide.
    pub staff: Option<u8>,
    /// Leave out, after the first system, the staves of a part that plays
    /// nothing in that system, as Sibelius' «Hide Empty Staves» does.
    pub hide_empty_staves: bool,
    pub margins: Margins,
    /// In parts, draw every empty bar on its own instead of gathering runs
    /// of them into multi-measure rests. Off by default: parts gather them.
    pub separate_empty_bars: bool,
}
impl PageSetup {
    pub(crate) fn is_default(&self) -> bool {
        *self == Self::default()
    }
    /// Width and height of the paper in millimetres, turned if landscape.
    /// `part` is true for an extracted part, which never goes on A3 by itself.
    pub fn paper_mm(&self, score: &Score, part: bool) -> (f32, f32) {
        let (width, height) = self.paper.portrait(score.parts().len(), part);
        if self.landscape {
            (height, width)
        } else {
            (width, height)
        }
    }
    pub(crate) fn validate(&self) -> Result<(), ScoreError> {
        if self.staff.is_some_and(|size| !(30..=100).contains(&size)) {
            return Err(ScoreError("Tamaño de pentagrama inválido"));
        }
        Ok(())
    }
}

impl Score {
    pub fn page_setup(&self) -> PageSetup {
        self.page_setup
    }
    /// Whether anything changed.
    pub fn set_page_setup(&mut self, setup: PageSetup) -> Result<bool, ScoreError> {
        setup.validate()?;
        let changed = self.page_setup != setup;
        self.page_setup = setup;
        Ok(changed)
    }
}
