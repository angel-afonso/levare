//! Font families for score text and the bar counter on each OS.
//!
//! Score text is set in Times New Roman, whose widths score-layout's estimates
//! assume. macOS and Windows always have it. Linux rarely does, and cosmic-text
//! matches family names exactly, so there the bundled Liberation Serif takes its
//! place: same advance widths, so the layout holds.
use gpui::App;

/// The serif for titles, credits, labels, lyrics and every other score text.
#[cfg(any(target_os = "macos", target_os = "windows"))]
pub const SERIF: &str = "Times New Roman";
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub const SERIF: &str = "Liberation Serif";

/// The bar counter's monospaced digits. On Linux only the bundled bold weight is
/// registered, which the counter's semibold resolves to.
#[cfg(target_os = "macos")]
pub const MONO: &str = "Menlo";
#[cfg(target_os = "windows")]
pub const MONO: &str = "Consolas";
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub const MONO: &str = "Liberation Mono";

/// Registers the bundled fonts the families above need.
pub fn register(cx: &App) {
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let mut fonts: Vec<std::borrow::Cow<'static, [u8]>> = score_pdf::SERIF_FONTS
            .iter()
            .map(|font| std::borrow::Cow::Borrowed(*font))
            .collect();
        fonts.push(std::borrow::Cow::Borrowed(include_bytes!(
            "../../../assets/fonts/LiberationMono-Bold.ttf"
        )));
        if let Err(error) = cx.text_system().add_fonts(fonts) {
            eprintln!("No se pudieron cargar las fuentes incluidas: {error}");
        }
    }
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    let _ = cx;
}

#[cfg(test)]
mod tests {
    #[test]
    fn bundled_fonts_name_the_families_the_app_asks_for() {
        let family = |data: &[u8]| {
            let face = ttf_parser::Face::parse(data, 0).unwrap();
            face.names()
                .into_iter()
                .find(|name| name.name_id == ttf_parser::name_id::FAMILY && name.is_unicode())
                .and_then(|name| name.to_string())
                .unwrap()
        };
        for font in score_pdf::SERIF_FONTS {
            assert_eq!(family(font), "Liberation Serif");
        }
        assert_eq!(
            family(include_bytes!(
                "../../../assets/fonts/LiberationMono-Bold.ttf"
            )),
            "Liberation Mono"
        );
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        {
            assert_eq!(super::SERIF, "Liberation Serif");
            assert_eq!(super::MONO, "Liberation Mono");
        }
    }
}
