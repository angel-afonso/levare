//! Keystrokes independent of the UI toolkit: parsing, canonical text, labels
//! and translation to the characters the platform reports.

use std::fmt;

/// One key with its modifiers. Letters are stored lowercase; Shift is explicit.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Keystroke {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub cmd: bool,
    pub key: String,
}

/// Keys pressed one after another, such as `"cmd-k cmd-s"`.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct KeySequence(pub Vec<Keystroke>);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeyError(pub String);

impl fmt::Display for KeyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "combinación no válida: «{}»", self.0)
    }
}

impl std::error::Error for KeyError {}

const NAMED: &[&str] = &[
    "space",
    "enter",
    "escape",
    "backspace",
    "delete",
    "tab",
    "up",
    "down",
    "left",
    "right",
    "home",
    "end",
    "pageup",
    "pagedown",
    "f1",
    "f2",
    "f3",
    "f4",
    "f5",
    "f6",
    "f7",
    "f8",
    "f9",
    "f10",
    "f11",
    "f12",
];

/// Numeric keypad keys. They are part of the model so a desktop profile can
/// use them later, but GPUI 0.2.2 reports them as the main-row keys.
const NUMPAD: &[&str] = &[
    "numpad0",
    "numpad1",
    "numpad2",
    "numpad3",
    "numpad4",
    "numpad5",
    "numpad6",
    "numpad7",
    "numpad8",
    "numpad9",
    "numpadenter",
    "numpaddecimal",
    "numpadplus",
    "numpadminus",
    "numpadmultiply",
    "numpaddivide",
];

/// What Shift produces on the main number row of the active keyboard layout.
/// GPUI reports Shift+2 as the produced character (`"` on a Spanish layout,
/// `@` on a US one) and drops the Shift flag.
pub trait Layout {
    fn shifted_digit(&self, digit: u8) -> Option<String>;
}

/// The US ANSI layout, used when the platform layout cannot be read.
pub struct UsLayout;

impl Layout for UsLayout {
    fn shifted_digit(&self, digit: u8) -> Option<String> {
        ")!@#$%^&*("
            .chars()
            .nth(usize::from(digit))
            .map(String::from)
    }
}

impl Keystroke {
    pub fn parse(source: &str) -> Result<Self, KeyError> {
        let error = || KeyError(source.to_owned());
        let mut stroke = Self {
            ctrl: false,
            alt: false,
            shift: false,
            cmd: false,
            key: String::new(),
        };
        let mut rest = source.trim();
        if rest.is_empty() {
            return Err(error());
        }
        // A trailing "-" after a separator is the minus key itself: "cmd--".
        let key = if rest == "-" {
            rest = "";
            "-".to_owned()
        } else if let Some(prefix) = rest.strip_suffix("--") {
            rest = prefix;
            "-".to_owned()
        } else {
            let (prefix, key) = rest.rsplit_once('-').unwrap_or(("", rest));
            rest = prefix;
            key.to_owned()
        };
        for modifier in rest.split('-').filter(|part| !part.is_empty()) {
            match modifier.to_ascii_lowercase().as_str() {
                "ctrl" => stroke.ctrl = true,
                "alt" => stroke.alt = true,
                "shift" => stroke.shift = true,
                "cmd" => stroke.cmd = true,
                _ => return Err(error()),
            }
        }
        let mut chars = key.chars();
        stroke.key = match (chars.next(), chars.next()) {
            (Some(letter), None) if letter.is_ascii_uppercase() => {
                stroke.shift = true;
                letter.to_ascii_lowercase().to_string()
            }
            (Some(character), None) if !character.is_whitespace() => key,
            _ => {
                let named = key.to_ascii_lowercase();
                if !NAMED.contains(&named.as_str()) && !NUMPAD.contains(&named.as_str()) {
                    return Err(error());
                }
                named
            }
        };
        Ok(stroke)
    }

    /// Canonical text: `ctrl-alt-shift-cmd-key`.
    pub fn text(&self) -> String {
        let mut text = String::new();
        for (on, name) in [
            (self.ctrl, "ctrl-"),
            (self.alt, "alt-"),
            (self.shift, "shift-"),
            (self.cmd, "cmd-"),
        ] {
            if on {
                text.push_str(name);
            }
        }
        text.push_str(&self.key);
        text
    }

    pub fn is_numpad(&self) -> bool {
        self.key.starts_with("numpad")
    }

    fn digit(&self) -> Option<u8> {
        let mut chars = self.key.chars();
        match (chars.next(), chars.next()) {
            (Some(digit @ '0'..='9'), None) => Some(digit as u8 - b'0'),
            _ => None,
        }
    }

    /// The keystroke text GPUI matches, or `None` when this toolkit cannot
    /// report the key (numeric keypad) or the layout has no such character.
    pub fn to_platform(&self, layout: &dyn Layout) -> Option<String> {
        if self.is_numpad() {
            return None;
        }
        if self.shift
            && let Some(digit) = self.digit()
        {
            let produced = layout.shifted_digit(digit)?;
            return Some(
                Self {
                    shift: false,
                    key: produced,
                    ..self.clone()
                }
                .text(),
            );
        }
        Some(self.text())
    }

    /// Rebuilds a keystroke from what the platform reported, undoing the
    /// Shift+digit translation so that it can be stored layout-independently.
    pub fn from_platform(
        key: &str,
        ctrl: bool,
        alt: bool,
        shift: bool,
        cmd: bool,
        layout: &dyn Layout,
    ) -> Self {
        let digit = (!shift)
            .then(|| (0..=9).find(|digit| layout.shifted_digit(*digit).as_deref() == Some(key)))
            .flatten();
        Self {
            ctrl,
            alt,
            shift: shift || digit.is_some(),
            cmd,
            key: digit.map_or_else(|| key.to_owned(), |digit| digit.to_string()),
        }
    }

    /// macOS-style label: ⌃⌥⇧⌘ followed by the key.
    pub fn label(&self) -> String {
        let mut label = String::new();
        for (on, glyph) in [
            (self.ctrl, "⌃"),
            (self.alt, "⌥"),
            (self.shift, "⇧"),
            (self.cmd, "⌘"),
        ] {
            if on {
                label.push_str(glyph);
            }
        }
        let key = match self.key.as_str() {
            "space" => "Espacio".to_owned(),
            "enter" => "↩".to_owned(),
            "escape" => "Esc".to_owned(),
            "backspace" => "⌫".to_owned(),
            "delete" => "⌦".to_owned(),
            "tab" => "⇥".to_owned(),
            "up" => "↑".to_owned(),
            "down" => "↓".to_owned(),
            "left" => "←".to_owned(),
            "right" => "→".to_owned(),
            "home" => "↖".to_owned(),
            "end" => "↘".to_owned(),
            "pageup" => "⇞".to_owned(),
            "pagedown" => "⇟".to_owned(),
            "numpadenter" => "Num ↩".to_owned(),
            "numpaddecimal" => "Num .".to_owned(),
            "numpadplus" => "Num +".to_owned(),
            "numpadminus" => "Num −".to_owned(),
            "numpadmultiply" => "Num ×".to_owned(),
            "numpaddivide" => "Num ÷".to_owned(),
            key if key.starts_with("numpad") => format!("Num {}", &key[6..]),
            key if key.starts_with('f') && key.len() > 1 => key.to_ascii_uppercase(),
            key => key.to_uppercase(),
        };
        label.push_str(&key);
        label
    }
}

impl KeySequence {
    pub fn parse(source: &str) -> Result<Self, KeyError> {
        let strokes = source
            .split_whitespace()
            .map(Keystroke::parse)
            .collect::<Result<Vec<_>, _>>()?;
        if strokes.is_empty() {
            return Err(KeyError(source.to_owned()));
        }
        Ok(Self(strokes))
    }

    pub fn single(stroke: Keystroke) -> Self {
        Self(vec![stroke])
    }

    pub fn text(&self) -> String {
        self.0
            .iter()
            .map(Keystroke::text)
            .collect::<Vec<_>>()
            .join(" ")
    }

    pub fn label(&self) -> String {
        self.0
            .iter()
            .map(Keystroke::label)
            .collect::<Vec<_>>()
            .join(" ")
    }

    pub fn to_platform(&self, layout: &dyn Layout) -> Option<String> {
        self.0
            .iter()
            .map(|stroke| stroke.to_platform(layout))
            .collect::<Option<Vec<_>>>()
            .map(|strokes| strokes.join(" "))
    }
}

impl fmt::Display for KeySequence {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.text())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Spanish;
    impl Layout for Spanish {
        fn shifted_digit(&self, digit: u8) -> Option<String> {
            "=!\"·$%&/()"
                .chars()
                .nth(usize::from(digit))
                .map(String::from)
        }
    }

    #[test]
    fn parses_modifiers_named_keys_and_minus() {
        let stroke = Keystroke::parse("shift-cmd-z").unwrap();
        assert!(stroke.shift && stroke.cmd && !stroke.alt);
        assert_eq!(stroke.text(), "shift-cmd-z");
        assert_eq!(Keystroke::parse("cmd--").unwrap().key, "-");
        assert_eq!(Keystroke::parse("-").unwrap().key, "-");
        assert_eq!(Keystroke::parse("Enter").unwrap().key, "enter");
        assert_eq!(Keystroke::parse("C").unwrap().text(), "shift-c");
        assert_eq!(Keystroke::parse("alt-backspace").unwrap().label(), "⌥⌫");
        assert_eq!(Keystroke::parse("cmd-,").unwrap().label(), "⌘,");
        assert!(Keystroke::parse("hyper-a").is_err());
        assert!(Keystroke::parse("cmd-").is_err());
        assert!(Keystroke::parse("shift-banana").is_err());
        assert_eq!(
            KeySequence::parse("cmd-k   cmd-s").unwrap().text(),
            "cmd-k cmd-s"
        );
    }

    #[test]
    fn labels_follow_macos_modifier_order() {
        assert_eq!(
            Keystroke::parse("cmd-shift-alt-ctrl-b").unwrap().label(),
            "⌃⌥⇧⌘B"
        );
        assert_eq!(Keystroke::parse("space").unwrap().label(), "Espacio");
        assert_eq!(Keystroke::parse("numpad4").unwrap().label(), "Num 4");
        assert_eq!(Keystroke::parse("f7").unwrap().label(), "F7");
    }

    #[test]
    fn shifted_digits_use_the_layout_and_round_trip() {
        let interval = Keystroke::parse("shift-2").unwrap();
        assert_eq!(interval.to_platform(&UsLayout).as_deref(), Some("@"));
        assert_eq!(interval.to_platform(&Spanish).as_deref(), Some("\""));
        assert_eq!(
            Keystroke::from_platform("\"", false, false, false, false, &Spanish),
            interval
        );
        assert_eq!(
            Keystroke::from_platform("@", false, false, false, false, &UsLayout),
            interval
        );
        let letter = Keystroke::from_platform("a", false, false, true, false, &Spanish);
        assert_eq!(letter.text(), "shift-a");
        assert!(
            Keystroke::parse("numpad4")
                .unwrap()
                .to_platform(&UsLayout)
                .is_none()
        );
    }
}
