//! Pointer gestures that depend on held modifiers. They live next to the
//! shortcuts so hints name their modifiers the way the keymap does.

use crate::{Keystroke, Platform};

/// A line drawn by dragging from one note to another of the same voice.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LineDrag {
    Slur,
    Crescendo,
    Diminuendo,
}

impl LineDrag {
    pub const ALL: [Self; 3] = [Self::Slur, Self::Crescendo, Self::Diminuendo];

    /// Which line a press with these modifiers starts, or `None` when it is
    /// an ordinary press. `secondary` is the platform's command key (GPUI's
    /// `Modifiers::secondary()`).
    pub fn from_modifiers(
        platform: Platform,
        alt: bool,
        shift: bool,
        secondary: bool,
    ) -> Option<Self> {
        let (line, hairpin) = Self::keys(platform, alt, secondary);
        match (line, hairpin, shift) {
            (false, ..) => None,
            (true, false, _) => Some(Self::Slur),
            (true, true, false) => Some(Self::Crescendo),
            (true, true, true) => Some(Self::Diminuendo),
        }
    }

    /// Alt draws a slur and the command key turns it into a hairpin, as in
    /// Sibelius. Xfce, Cinnamon and MATE move the window on Alt+drag, so on
    /// Linux Ctrl draws the slur and Alt, never alone, the hairpin.
    fn keys(platform: Platform, alt: bool, secondary: bool) -> (bool, bool) {
        match platform {
            Platform::MacOs | Platform::Windows => (alt, secondary),
            Platform::Linux => (secondary, alt),
        }
    }

    /// The modifiers to hold, as the keymap labels them: `⌥⌘` on macOS,
    /// `Ctrl+Alt` on Linux.
    pub fn label(self) -> String {
        self.label_for(Platform::current())
    }

    pub fn label_for(self, platform: Platform) -> String {
        let (hairpin, shift) = match self {
            Self::Slur => (false, false),
            Self::Crescendo => (true, false),
            Self::Diminuendo => (true, true),
        };
        let (alt, secondary) = match platform {
            Platform::MacOs | Platform::Windows => (true, hairpin),
            Platform::Linux => (hairpin, true),
        };
        Keystroke {
            ctrl: false,
            alt,
            shift,
            secondary,
            key: String::new(),
        }
        .modifiers_label_for(platform)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use Platform::{Linux, MacOs, Windows};

    #[test]
    fn macos_and_windows_hold_alt_as_sibelius_does() {
        for platform in [MacOs, Windows] {
            let drag =
                |alt, shift, secondary| LineDrag::from_modifiers(platform, alt, shift, secondary);
            assert_eq!(drag(true, false, false), Some(LineDrag::Slur));
            assert_eq!(drag(true, true, false), Some(LineDrag::Slur));
            assert_eq!(drag(true, false, true), Some(LineDrag::Crescendo));
            assert_eq!(drag(true, true, true), Some(LineDrag::Diminuendo));
            assert_eq!(drag(false, false, true), None);
            assert_eq!(drag(false, true, true), None);
        }
        let labels = |platform| LineDrag::ALL.map(|line| line.label_for(platform));
        assert_eq!(labels(MacOs), ["⌥", "⌥⌘", "⌥⇧⌘"]);
        assert_eq!(labels(Windows), ["Alt", "Ctrl+Alt", "Ctrl+Mayús+Alt"]);
    }

    #[test]
    fn linux_never_drags_with_alt_alone() {
        let drag = |alt, shift, secondary| LineDrag::from_modifiers(Linux, alt, shift, secondary);
        // Alt+drag moves the window on Xfce, Cinnamon and MATE.
        assert_eq!(drag(true, false, false), None);
        assert_eq!(drag(true, true, false), None);
        assert_eq!(drag(false, false, true), Some(LineDrag::Slur));
        assert_eq!(drag(true, false, true), Some(LineDrag::Crescendo));
        assert_eq!(drag(true, true, true), Some(LineDrag::Diminuendo));
        for line in LineDrag::ALL {
            assert!(line.label_for(Linux).starts_with("Ctrl"), "{line:?}");
        }
        assert_eq!(
            LineDrag::ALL.map(|line| line.label_for(Linux)),
            ["Ctrl", "Ctrl+Alt", "Ctrl+Mayús+Alt"]
        );
    }
}
