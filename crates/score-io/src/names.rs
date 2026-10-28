//! File and folder names made from text the musician typed, such as a title.
//!
//! The same rules apply on every platform, so a score saved or exported on one
//! system can be copied to any other.

/// A single path component built from `value` that macOS, Windows and Linux all
/// accept:
///
/// - `/ \ : < > " | ? *` and control characters become `_`;
/// - leading and trailing spaces and dots are trimmed (Windows drops trailing
///   ones, and a leading dot hides the file on macOS and Linux);
/// - a name Windows reserves for devices (`CON`, `NUL`, `COM1`…) gets a `_`;
/// - it is cut to `max_units` UTF-16 units, which is how Windows measures its
///   260-character path limit, and never beyond 255 UTF-8 bytes;
/// - an empty result becomes «Partitura».
///
/// Callers keep `max_units` small enough that the deepest path they build stays
/// under 260 characters below a reasonably deep folder.
pub fn file_name(value: &str, max_units: usize) -> String {
    let mut name = String::new();
    let mut units = 0;
    for ch in value.chars() {
        let ch = if ch.is_control() || FORBIDDEN.contains(&ch) {
            '_'
        } else {
            ch
        };
        units += ch.len_utf16();
        if units > max_units || name.len() + ch.len_utf8() > MAX_BYTES {
            break;
        }
        name.push(ch);
    }
    let name = name.trim_matches(|ch: char| ch.is_whitespace() || ch == '.');
    if name.is_empty() {
        return "Partitura".into();
    }
    let stem_end = name.find('.').unwrap_or(name.len());
    if is_reserved(name[..stem_end].trim_end()) {
        return format!("{}_{}", &name[..stem_end], &name[stem_end..]);
    }
    name.into()
}

const FORBIDDEN: [char; 9] = ['/', '\\', ':', '<', '>', '"', '|', '?', '*'];

/// One byte is left for the `_` a reserved name may receive.
const MAX_BYTES: usize = 254;

fn is_reserved(stem: &str) -> bool {
    let upper = stem.to_uppercase();
    if matches!(upper.as_str(), "CON" | "PRN" | "AUX" | "NUL") {
        return true;
    }
    let mut chars = upper.chars();
    let prefix: String = chars.by_ref().take(3).collect();
    let digit: String = chars.collect();
    matches!(prefix.as_str(), "COM" | "LPT")
        && matches!(
            digit.as_str(),
            "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "¹" | "²" | "³"
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn characters_windows_forbids_become_underscores() {
        assert_eq!(file_name("¿Quién?", 80), "¿Quién_");
        assert_eq!(
            file_name(r#"a<b>c:d"e/f\g|h?i*j"#, 80),
            "a_b_c_d_e_f_g_h_i_j"
        );
        assert_eq!(file_name("Coral\tnº\u{7}1", 80), "Coral_nº_1");
        assert_eq!(file_name("Sonata para violín", 80), "Sonata para violín");
    }

    #[test]
    fn edges_lose_spaces_and_dots() {
        assert_eq!(file_name("  Final...  ", 80), "Final");
        assert_eq!(file_name(".oculta", 80), "oculta");
        assert_eq!(file_name("Acto I. Escena 2", 80), "Acto I. Escena 2");
        assert_eq!(file_name(" .. ", 80), "Partitura");
        assert_eq!(file_name("", 80), "Partitura");
        assert_eq!(file_name("???", 80), "___");
    }

    #[test]
    fn device_names_windows_reserves_are_changed() {
        for reserved in [
            "CON", "con", "Prn", "AUX", "NUL", "COM1", "com9", "LPT3", "COM¹",
        ] {
            assert_eq!(file_name(reserved, 80), format!("{reserved}_"));
        }
        assert_eq!(file_name("nul.partitura", 80), "nul_.partitura");
        assert_eq!(file_name("CON .txt", 80), "CON _.txt");
        for fine in ["Concierto", "COM0", "COM10", "LPT", "Aux de piano", "NULO"] {
            assert_eq!(file_name(fine, 80), fine);
        }
    }

    #[test]
    fn length_is_bounded_in_utf16_units_and_bytes() {
        let cut = file_name(&"a".repeat(300), 64);
        assert_eq!(cut.len(), 64);
        // Each violin is two UTF-16 units and four bytes.
        let violins = file_name(&"🎻".repeat(300), 64);
        assert_eq!(violins.chars().count(), 32);
        let wide = file_name(&"é".repeat(300), 1000);
        assert!(wide.len() <= 255, "{}", wide.len());
        assert_eq!(file_name("Final final .", 7), "Final f");
        assert_eq!(file_name("Final  x", 7), "Final");
    }
}
