//! Characters that Shift produces on the number row of the active keyboard
//! layout. GPUI reports Shift+2 as that character (`"` on a Spanish layout),
//! so bindings such as Sibelius' Shift+1–9 must be installed per layout.

use score_keymap::{Layout, UsLayout};

pub struct SystemLayout {
    digits: [Option<String>; 10],
}

impl SystemLayout {
    /// `name` is GPUI's name for the active layout (`cx.keyboard_layout()`);
    /// only Linux needs it, because there the layout is read from a table.
    pub fn current(name: &str) -> Self {
        Self {
            digits: std::array::from_fn(|digit| {
                let digit = digit as u8;
                platform::shifted_digit(digit, name).or_else(|| UsLayout.shifted_digit(digit))
            }),
        }
    }
}

impl Layout for SystemLayout {
    fn shifted_digit(&self, digit: u8) -> Option<String> {
        self.digits.get(usize::from(digit)).cloned().flatten()
    }
}

#[cfg(target_os = "macos")]
mod platform {
    use std::ffi::c_void;

    type CFTypeRef = *const c_void;

    #[link(name = "Carbon", kind = "framework")]
    unsafe extern "C" {
        fn TISCopyCurrentKeyboardLayoutInputSource() -> CFTypeRef;
        fn TISGetInputSourceProperty(source: CFTypeRef, key: CFTypeRef) -> CFTypeRef;
        static kTISPropertyUnicodeKeyLayoutData: CFTypeRef;
        fn LMGetKbdType() -> u8;
        fn UCKeyTranslate(
            layout: *const c_void,
            virtual_key: u16,
            action: u16,
            modifiers: u32,
            keyboard_type: u32,
            options: u32,
            dead_key_state: *mut u32,
            max_length: usize,
            actual_length: *mut usize,
            characters: *mut u16,
        ) -> i32;
    }

    #[link(name = "CoreFoundation", kind = "framework")]
    unsafe extern "C" {
        fn CFDataGetBytePtr(data: CFTypeRef) -> *const u8;
        fn CFRelease(value: CFTypeRef);
    }

    /// ANSI virtual key codes of the main-row digits 0–9.
    const KEY_CODES: [u16; 10] = [29, 18, 19, 20, 21, 23, 22, 26, 28, 25];
    const KEY_DOWN: u16 = 0;
    /// `shiftKey >> 8`, the modifier format UCKeyTranslate expects.
    const SHIFT: u32 = 0x0200 >> 8;
    const NO_DEAD_KEYS: u32 = 1;

    pub fn shifted_digit(digit: u8, _: &str) -> Option<String> {
        let code = *KEY_CODES.get(usize::from(digit))?;
        // SAFETY: the input source is owned (Copy rule) and released below; the
        // layout data follows the Get rule and is only read while the source
        // is alive. UCKeyTranslate writes at most `characters.len()` units.
        unsafe {
            let source = TISCopyCurrentKeyboardLayoutInputSource();
            if source.is_null() {
                return None;
            }
            let data = TISGetInputSourceProperty(source, kTISPropertyUnicodeKeyLayoutData);
            let produced = (!data.is_null())
                .then(|| {
                    let mut dead_keys = 0;
                    let mut length = 0;
                    let mut characters = [0u16; 4];
                    let status = UCKeyTranslate(
                        CFDataGetBytePtr(data).cast(),
                        code,
                        KEY_DOWN,
                        SHIFT,
                        u32::from(LMGetKbdType()),
                        NO_DEAD_KEYS,
                        &mut dead_keys,
                        characters.len(),
                        &mut length,
                        characters.as_mut_ptr(),
                    );
                    (status == 0).then(|| String::from_utf16_lossy(&characters[..length]))
                })
                .flatten();
            CFRelease(source);
            produced.filter(|text| {
                let mut chars = text.chars();
                matches!((chars.next(), chars.next()), (Some(c), None) if !c.is_whitespace() && !c.is_control())
            })
        }
    }
}

/// Asks the active layout what Shift plus each digit key types, the way GPUI
/// itself translates keys (`ToUnicode` on the key's scan code).
#[cfg(target_os = "windows")]
mod platform {
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        GetKeyboardLayout, MAPVK_VK_TO_VSC, MapVirtualKeyExW, ToUnicodeEx, VK_SHIFT,
    };

    /// Leave the keyboard state, dead keys included, as it was.
    const NO_STATE_CHANGE: u32 = 1 << 2;

    pub fn shifted_digit(digit: u8, _: &str) -> Option<String> {
        if digit > 9 {
            return None;
        }
        // VK_0 to VK_9 are the ASCII digits.
        let virtual_key = u32::from(b'0' + digit);
        let mut state = [0u8; 256];
        state[usize::from(VK_SHIFT.0)] = 0x80;
        let mut characters = [0u16; 8];
        // SAFETY: plain calls on the calling thread's layout; ToUnicodeEx
        // writes at most `characters.len()` units and reads 256 key states.
        let length = unsafe {
            let layout = GetKeyboardLayout(0);
            let scan_code = MapVirtualKeyExW(virtual_key, MAPVK_VK_TO_VSC, Some(layout));
            if scan_code == 0 {
                return None;
            }
            ToUnicodeEx(
                virtual_key,
                scan_code,
                &state,
                &mut characters,
                NO_STATE_CHANGE,
                Some(layout),
            )
        };
        // A negative length is a dead key, which GPUI does not report as a key.
        let length = usize::try_from(length).ok().filter(|length| *length > 0)?;
        super::single_character(String::from_utf16(&characters[..length]).ok()?)
    }
}

/// GPUI keeps its xkb state to itself, so the layout is recognized by the
/// name it reports and looked up in a table of common layouts. Unknown
/// layouts fall back to US.
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
mod platform {
    pub fn shifted_digit(digit: u8, name: &str) -> Option<String> {
        super::xkb_shifted_digits(name)?
            .get(usize::from(digit))
            .map(|key| (*key).to_owned())
    }
}

/// Keys GPUI reports on X11 and Wayland for Shift+0…9 under the layout named
/// `name` (xkeyboard-config's description, which GPUI passes on). Characters
/// outside ASCII arrive as their keysym names, such as `periodcentered`.
#[cfg(any(not(any(target_os = "macos", target_os = "windows")), test))]
fn xkb_shifted_digits(name: &str) -> Option<[&'static str; 10]> {
    // Regional layouts that share a language name but not its number row.
    const OTHER_REGIONS: &[&str] = &[
        "Spanish (Latin American",
        "French (Canada",
        "French (Switzerland",
        "German (Switzerland",
        "Portuguese (Brazil",
        "English (UK, Macintosh",
    ];
    if OTHER_REGIONS.iter().any(|region| name.starts_with(region)) {
        return (name.starts_with("Spanish (Latin American"))
            .then_some(["=", "!", "\"", "#", "$", "%", "&", "/", "(", ")"]);
    }
    let base = name.split(" (").next().unwrap_or(name);
    let united_kingdom = name.starts_with("English (UK");
    Some(match base {
        "English" if united_kingdom => [")", "!", "\"", "sterling", "$", "%", "^", "&", "*", "("],
        "English" => [")", "!", "@", "#", "$", "%", "^", "&", "*", "("],
        "Spanish" => [
            "=",
            "!",
            "\"",
            "periodcentered",
            "$",
            "%",
            "&",
            "/",
            "(",
            ")",
        ],
        "French" => ["0", "1", "2", "3", "4", "5", "6", "7", "8", "9"],
        "German" => ["=", "!", "\"", "section", "$", "%", "&", "/", "(", ")"],
        "Italian" => ["=", "!", "\"", "sterling", "$", "%", "&", "/", "(", ")"],
        "Portuguese" => ["=", "!", "\"", "#", "$", "%", "&", "/", "(", ")"],
        _ => return None,
    })
}

/// One printable character, as a key binding can name it.
#[cfg(target_os = "windows")]
fn single_character(text: String) -> Option<String> {
    let mut chars = text.chars();
    matches!((chars.next(), chars.next()), (Some(c), None) if !c.is_whitespace() && !c.is_control())
        .then_some(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linux_layouts_are_found_by_the_name_gpui_reports() {
        let spanish = xkb_shifted_digits("Spanish").unwrap();
        assert_eq!(spanish[2], "\"");
        assert_eq!(spanish[3], "periodcentered");
        assert_eq!(xkb_shifted_digits("Spanish (no dead keys)"), Some(spanish));
        assert_eq!(xkb_shifted_digits("English (US)").unwrap()[2], "@");
        let uk = xkb_shifted_digits("English (UK)").unwrap();
        assert_eq!((uk[0], uk[2], uk[3]), (")", "\"", "sterling"));
        assert_eq!(xkb_shifted_digits("German").unwrap()[3], "section");
        assert_eq!(xkb_shifted_digits("French").unwrap()[1], "1");
        assert_eq!(xkb_shifted_digits("Italian").unwrap()[3], "sterling");
        assert_eq!(xkb_shifted_digits("Portuguese").unwrap()[3], "#");
        assert_eq!(
            xkb_shifted_digits("Spanish (Latin American)").unwrap()[3],
            "#"
        );
        assert_eq!(xkb_shifted_digits("Portuguese (Brazil)"), None);
        assert_eq!(xkb_shifted_digits("German (Switzerland)"), None);
        assert_eq!(xkb_shifted_digits("unknown"), None);
    }
}
