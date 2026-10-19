//! Characters that Shift produces on the number row of the active keyboard
//! layout. GPUI reports Shift+2 as that character (`"` on a Spanish layout),
//! so bindings such as Sibelius' Shift+1–9 must be installed per layout.

use score_keymap::{Layout, UsLayout};

pub struct SystemLayout {
    digits: [Option<String>; 10],
}

impl SystemLayout {
    pub fn current() -> Self {
        Self {
            digits: std::array::from_fn(|digit| {
                let digit = digit as u8;
                platform::shifted_digit(digit).or_else(|| UsLayout.shifted_digit(digit))
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

    pub fn shifted_digit(digit: u8) -> Option<String> {
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

#[cfg(not(target_os = "macos"))]
mod platform {
    pub fn shifted_digit(_: u8) -> Option<String> {
        None
    }
}
