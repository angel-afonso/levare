//! Keystrokes independent of the UI toolkit: parsing, canonical text, labels
//! and translation to the characters the platform reports.

use std::fmt;

/// The operating system whose conventions a keystroke follows: which physical
/// key `secondary` is, how labels read and which defaults a profile uses.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Platform {
    MacOs,
    Windows,
    Linux,
}

impl Platform {
    pub const ALL: [Self; 3] = [Self::MacOs, Self::Windows, Self::Linux];

    /// The platform this build runs on. BSDs and other Unix desktops follow
    /// Linux.
    pub const fn current() -> Self {
        if cfg!(target_os = "macos") {
            Self::MacOs
        } else if cfg!(target_os = "windows") {
            Self::Windows
        } else {
            Self::Linux
        }
    }
}

/// A modifier key as the musician names it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Modifier {
    Ctrl,
    Alt,
    Shift,
    /// ⌘ on macOS, Ctrl elsewhere: the key of Save, Copy and Undo.
    Secondary,
}

impl Modifier {
    pub fn label(self) -> &'static str {
        self.label_for(Platform::current())
    }

    pub fn label_for(self, platform: Platform) -> &'static str {
        match (platform, self) {
            (Platform::MacOs, Self::Ctrl) => "⌃",
            (Platform::MacOs, Self::Alt) => "⌥",
            (Platform::MacOs, Self::Shift) => "⇧",
            (Platform::MacOs, Self::Secondary) => "⌘",
            (_, Self::Ctrl | Self::Secondary) => "Ctrl",
            (_, Self::Alt) => "Alt",
            (_, Self::Shift) => "Mayús",
        }
    }
}

/// One key with its modifiers. Letters are stored lowercase; Shift is explicit.
///
/// `secondary` is the platform's command key: ⌘ on macOS, Ctrl on Windows and
/// Linux. Off macOS it and `ctrl` are the same physical key.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Keystroke {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub secondary: bool,
    pub key: String,
}

/// Keys pressed one after another, such as `"secondary-k secondary-s"`.
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
            secondary: false,
            key: String::new(),
        };
        let mut rest = source.trim();
        if rest.is_empty() {
            return Err(error());
        }
        // A trailing "-" after a separator is the minus key itself: "secondary--".
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
                // Files saved when Levare only ran on macOS say "cmd".
                "secondary" | "cmd" => stroke.secondary = true,
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

    /// Canonical text, the same on every platform: `ctrl-alt-shift-secondary-key`.
    pub fn text(&self) -> String {
        let mut text = String::new();
        for (on, name) in [
            (self.ctrl, "ctrl-"),
            (self.alt, "alt-"),
            (self.shift, "shift-"),
            (self.secondary, "secondary-"),
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

    /// The keystroke text GPUI matches on this platform, or `None` when this
    /// toolkit cannot report the key (numeric keypad) or the layout has no
    /// such character.
    pub fn to_platform(&self, layout: &dyn Layout) -> Option<String> {
        self.to_platform_for(layout, Platform::current())
    }

    /// GPUI text with explicit modifiers: `secondary` becomes `cmd` on macOS
    /// and `ctrl` elsewhere, where GPUI reads `cmd` as the Windows or Super key.
    pub fn to_platform_for(&self, layout: &dyn Layout, platform: Platform) -> Option<String> {
        if self.is_numpad() {
            return None;
        }
        let mut stroke = self.clone();
        if stroke.shift
            && let Some(digit) = stroke.digit()
        {
            stroke.key = layout.shifted_digit(digit)?;
            stroke.shift = false;
        }
        let (ctrl, cmd) = match platform {
            Platform::MacOs => (stroke.ctrl, stroke.secondary),
            Platform::Windows | Platform::Linux => (stroke.ctrl || stroke.secondary, false),
        };
        let mut text = String::new();
        for (on, name) in [
            (ctrl, "ctrl-"),
            (stroke.alt, "alt-"),
            (stroke.shift, "shift-"),
            (cmd, "cmd-"),
        ] {
            if on {
                text.push_str(name);
            }
        }
        text.push_str(&stroke.key);
        Some(text)
    }

    /// Rebuilds a keystroke from what the platform reported, undoing the
    /// Shift+digit translation so that it can be stored layout-independently.
    /// `secondary` is GPUI's `Modifiers::secondary()`; `ctrl` is Control only
    /// when it is a different key (macOS).
    pub fn from_platform(
        key: &str,
        ctrl: bool,
        alt: bool,
        shift: bool,
        secondary: bool,
        layout: &dyn Layout,
    ) -> Self {
        let digit = (!shift)
            .then(|| (0..=9).find(|digit| layout.shifted_digit(*digit).as_deref() == Some(key)))
            .flatten();
        Self {
            ctrl,
            alt,
            shift: shift || digit.is_some(),
            secondary,
            key: digit.map_or_else(|| key.to_owned(), |digit| digit.to_string()),
        }
    }

    /// The label for this platform: `⌃⌥⇧⌘Z` on macOS, `Ctrl+Mayús+Alt+Z`
    /// elsewhere.
    pub fn label(&self) -> String {
        self.label_for(Platform::current())
    }

    pub fn label_for(&self, platform: Platform) -> String {
        if platform == Platform::MacOs {
            self.mac_label()
        } else {
            self.pc_label(platform)
        }
    }

    /// ⌃⌥⇧⌘ glyphs followed by the key, as macOS menus write it.
    fn mac_label(&self) -> String {
        let mut label = String::new();
        for (on, modifier) in [
            (self.ctrl, Modifier::Ctrl),
            (self.alt, Modifier::Alt),
            (self.shift, Modifier::Shift),
            (self.secondary, Modifier::Secondary),
        ] {
            if on {
                label.push_str(modifier.label_for(Platform::MacOs));
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

    /// Modifier names joined with `+`, as Windows and Linux write them, in
    /// Spanish like the rest of the interface.
    fn pc_label(&self, platform: Platform) -> String {
        let mut parts: Vec<&str> = [
            (self.ctrl || self.secondary, Modifier::Ctrl),
            (self.shift, Modifier::Shift),
            (self.alt, Modifier::Alt),
        ]
        .into_iter()
        .filter(|(on, _)| *on)
        .map(|(_, modifier)| modifier.label_for(platform))
        .collect();
        let key = match self.key.as_str() {
            "space" => "Espacio".to_owned(),
            "enter" => "Intro".to_owned(),
            "escape" => "Esc".to_owned(),
            "backspace" => "Retroceso".to_owned(),
            "delete" => "Supr".to_owned(),
            "tab" => "Tab".to_owned(),
            "up" => "↑".to_owned(),
            "down" => "↓".to_owned(),
            "left" => "←".to_owned(),
            "right" => "→".to_owned(),
            "home" => "Inicio".to_owned(),
            "end" => "Fin".to_owned(),
            "pageup" => "RePág".to_owned(),
            "pagedown" => "AvPág".to_owned(),
            "numpadenter" => "Num Intro".to_owned(),
            "numpaddecimal" => "Num .".to_owned(),
            "numpadplus" => "Num +".to_owned(),
            "numpadminus" => "Num −".to_owned(),
            "numpadmultiply" => "Num ×".to_owned(),
            "numpaddivide" => "Num ÷".to_owned(),
            key if key.starts_with("numpad") => format!("Num {}", &key[6..]),
            key if key.starts_with('f') && key.len() > 1 => key.to_ascii_uppercase(),
            key => key.to_uppercase(),
        };
        parts.push(&key);
        parts.join("+")
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
        self.label_for(Platform::current())
    }

    pub fn label_for(&self, platform: Platform) -> String {
        self.0
            .iter()
            .map(|stroke| stroke.label_for(platform))
            .collect::<Vec<_>>()
            .join(" ")
    }

    pub fn to_platform(&self, layout: &dyn Layout) -> Option<String> {
        self.to_platform_for(layout, Platform::current())
    }

    pub fn to_platform_for(&self, layout: &dyn Layout, platform: Platform) -> Option<String> {
        self.0
            .iter()
            .map(|stroke| stroke.to_platform_for(layout, platform))
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

    use Platform::{Linux, MacOs, Windows};

    fn label(text: &str, platform: Platform) -> String {
        KeySequence::parse(text).unwrap().label_for(platform)
    }

    #[test]
    fn parses_modifiers_named_keys_and_minus() {
        let stroke = Keystroke::parse("shift-secondary-z").unwrap();
        assert!(stroke.shift && stroke.secondary && !stroke.alt && !stroke.ctrl);
        assert_eq!(stroke.text(), "shift-secondary-z");
        assert_eq!(Keystroke::parse("secondary--").unwrap().key, "-");
        assert_eq!(Keystroke::parse("-").unwrap().key, "-");
        assert_eq!(Keystroke::parse("Enter").unwrap().key, "enter");
        assert_eq!(Keystroke::parse("C").unwrap().text(), "shift-c");
        assert!(Keystroke::parse("hyper-a").is_err());
        assert!(Keystroke::parse("secondary-").is_err());
        assert!(Keystroke::parse("shift-banana").is_err());
        assert_eq!(
            KeySequence::parse("secondary-k   secondary-s")
                .unwrap()
                .text(),
            "secondary-k secondary-s"
        );
    }

    #[test]
    fn keys_saved_with_cmd_still_mean_the_command_key() {
        let saved = KeySequence::parse("shift-cmd-z").unwrap();
        assert_eq!(saved, KeySequence::parse("secondary-shift-z").unwrap());
        assert_eq!(saved.text(), "shift-secondary-z");
        assert_eq!(
            saved.to_platform_for(&UsLayout, MacOs).as_deref(),
            Some("shift-cmd-z")
        );
    }

    #[test]
    fn macos_labels_use_glyphs_in_menu_order() {
        assert_eq!(label("secondary-shift-alt-ctrl-b", MacOs), "⌃⌥⇧⌘B");
        assert_eq!(label("alt-backspace", MacOs), "⌥⌫");
        assert_eq!(label("secondary-,", MacOs), "⌘,");
        assert_eq!(label("shift-secondary-z", MacOs), "⇧⌘Z");
        assert_eq!(label("secondary-backspace", MacOs), "⌘⌫");
        assert_eq!(label("enter", MacOs), "↩");
        assert_eq!(label("delete", MacOs), "⌦");
        assert_eq!(label("tab", MacOs), "⇥");
        assert_eq!(label("space", MacOs), "Espacio");
        assert_eq!(label("numpad4", MacOs), "Num 4");
        assert_eq!(label("f7", MacOs), "F7");
        assert_eq!(label("secondary-k secondary-s", MacOs), "⌘K ⌘S");
    }

    #[test]
    fn windows_and_linux_labels_spell_modifiers_in_spanish() {
        for platform in [Windows, Linux] {
            assert_eq!(label("secondary-shift-alt-z", platform), "Ctrl+Mayús+Alt+Z");
            assert_eq!(label("shift-secondary-z", platform), "Ctrl+Mayús+Z");
            assert_eq!(label("ctrl-secondary-z", platform), "Ctrl+Z");
            assert_eq!(label("secondary-,", platform), "Ctrl+,");
            assert_eq!(label("secondary--", platform), "Ctrl+-");
            assert_eq!(label("alt-backspace", platform), "Alt+Retroceso");
            assert_eq!(label("delete", platform), "Supr");
            assert_eq!(label("enter", platform), "Intro");
            assert_eq!(label("escape", platform), "Esc");
            assert_eq!(label("shift-tab", platform), "Mayús+Tab");
            assert_eq!(label("alt-left", platform), "Alt+←");
            assert_eq!(label("shift-space", platform), "Mayús+Espacio");
            assert_eq!(label("shift-3", platform), "Mayús+3");
            assert_eq!(label("numpadenter", platform), "Num Intro");
            assert_eq!(label("f11", platform), "F11");
            assert_eq!(label("secondary-k secondary-s", platform), "Ctrl+K Ctrl+S");
        }
        assert_eq!(Modifier::Secondary.label_for(Windows), "Ctrl");
        assert_eq!(Modifier::Secondary.label_for(MacOs), "⌘");
    }

    #[test]
    fn secondary_is_command_on_macos_and_control_elsewhere() {
        let undo = KeySequence::parse("shift-secondary-z").unwrap();
        assert_eq!(
            undo.to_platform_for(&UsLayout, MacOs).as_deref(),
            Some("shift-cmd-z")
        );
        assert_eq!(
            undo.to_platform_for(&UsLayout, Windows).as_deref(),
            Some("ctrl-shift-z")
        );
        assert_eq!(
            undo.to_platform_for(&UsLayout, Linux).as_deref(),
            Some("ctrl-shift-z")
        );
        let both = KeySequence::parse("ctrl-secondary-f").unwrap();
        assert_eq!(
            both.to_platform_for(&UsLayout, MacOs).as_deref(),
            Some("ctrl-cmd-f")
        );
        assert_eq!(
            both.to_platform_for(&UsLayout, Linux).as_deref(),
            Some("ctrl-f")
        );
        let tab = KeySequence::parse("secondary-shift-2").unwrap();
        assert_eq!(
            tab.to_platform_for(&Spanish, Windows).as_deref(),
            Some("ctrl-\"")
        );
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
        let tab = Keystroke::from_platform("\"", false, false, false, true, &Spanish);
        assert_eq!(tab.text(), "shift-secondary-2");
        assert!(
            Keystroke::parse("numpad4")
                .unwrap()
                .to_platform(&UsLayout)
                .is_none()
        );
    }
}
