//! Shared musical text mapping for screen, native menus and PDF export.
/// The single repertoire shared by score painting, UI symbols and native menus.
pub fn glyph_codes() -> impl Iterator<Item = u32> {
    [0xe000, 0xe003, 0xe004, 0xe014, 0xe030, 0xf401, 0xf402]
        .into_iter()
        .chain([0xe031, 0xe032, 0xe040, 0xe041, 0xe044, 0xe047, 0xe048])
        .chain([0xe050, 0xe05c, 0xe062, 0xe069])
        .chain(0xe080..=0xe089)
        .chain(0xe1d2..=0xe1dc)
        .chain(0xe0a2..=0xe0a4)
        .chain(0xe220..=0xe222)
        .chain(0xe240..=0xe245)
        .chain(0xe260..=0xe264)
        .chain([0xe614, 0xe0dd])
        .chain(0xe4e3..=0xe4e8)
        // Ribbon symbols for the acciaccatura and the appoggiatura.
        .chain([0xe560, 0xe562])
        // 8va, 8vb, Ped., trill, trill wiggle and glissando.
        .chain([0xe511, 0xe51c, 0xe650, 0xe566, 0xeaa4, 0xe585])
        .chain([0xe634, 0xe635, 0xe63c, 0xeaa9, 0xeaaa, 0xeaad, 0xeaae])
        .chain([0xe1e7, 0xe1fd, 0xe883, 0xe4c0, 0xe53e, 0xe53f])
        .chain([
            0xe4a0, 0xe4a1, 0xe4a2, 0xe4a3, 0xe4a4, 0xe4a5, 0xe4ac, 0xe4ad, 0xe4c1, 0xe520, 0xe521,
            0xe522,
        ])
}

/// Convert ordinary Unicode notation to the exact glyphs used in the score.
/// Other private-use characters (e.g. keyboard shortcuts) remain ordinary text.
pub fn bravura_character(character: char) -> Option<char> {
    let character = match character {
        '♭' => '\u{e260}',
        '♮' => '\u{e261}',
        '♯' => '\u{e262}',
        '♩' => '\u{e1d5}',
        '♪' => '\u{e1d7}',
        '𝄞' => '\u{e050}',
        '𝄢' => '\u{e062}',
        other => other,
    };
    glyph_codes()
        .any(|code| code == u32::from(character))
        .then_some(character)
}

#[derive(Debug, PartialEq, Eq)]
pub enum TextRun {
    Text(String),
    Music(String),
}

pub fn text_runs(text: &str) -> Vec<TextRun> {
    let mut runs = Vec::new();
    for character in text.chars() {
        match bravura_character(character) {
            Some(glyph) => match runs.last_mut() {
                Some(TextRun::Music(text)) => text.push(glyph),
                _ => runs.push(TextRun::Music(glyph.to_string())),
            },
            None => match runs.last_mut() {
                Some(TextRun::Text(text)) => text.push(character),
                _ => runs.push(TextRun::Text(character.to_string())),
            },
        }
    }
    runs
}
