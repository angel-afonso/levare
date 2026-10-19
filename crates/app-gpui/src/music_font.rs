//! GPUI 0.2.2 rejects fonts without a Latin 'm' on macOS. Paint the original
//! SMuFL outlines directly instead of depending on its text-font fallback.
use gpui::{
    Bounds, FillOptions, FillRule, PathBuilder, PathStyle, Pixels, Point, Window, point, px, rgb,
    size,
};
use std::collections::HashMap;
use ttf_parser::{Face, OutlineBuilder};

enum Segment {
    Move(f32, f32),
    Line(f32, f32),
    Quad(f32, f32, f32, f32),
    Cubic(f32, f32, f32, f32, f32, f32),
    Close,
}

#[derive(Default)]
struct Outline {
    segments: Vec<Segment>,
}
impl OutlineBuilder for Outline {
    fn move_to(&mut self, x: f32, y: f32) {
        self.segments.push(Segment::Move(x, y));
    }
    fn line_to(&mut self, x: f32, y: f32) {
        self.segments.push(Segment::Line(x, y));
    }
    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        self.segments.push(Segment::Quad(x1, y1, x, y));
    }
    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        self.segments.push(Segment::Cubic(x1, y1, x2, y2, x, y));
    }
    fn close(&mut self) {
        self.segments.push(Segment::Close);
    }
}

pub struct MusicFont {
    outlines: HashMap<char, (Outline, f32, Bounds<Pixels>)>,
    scale: f32,
}
impl MusicFont {
    pub fn new() -> Self {
        let face = Face::parse(include_bytes!("../../../assets/fonts/Bravura.otf"), 0)
            .expect("bundled Bravura font");
        let scale = score_layout::MUSIC_SIZE / f32::from(face.units_per_em());
        let mut outlines = HashMap::new();
        for code in [0xe050, 0xe05c, 0xe062]
            .into_iter()
            .chain(0xe080..=0xe089)
            .chain(0xe1d2..=0xe1da)
            .chain(0xe0a2..=0xe0a4)
            .chain(0xe240..=0xe243)
            .chain(0xe260..=0xe262)
            .chain(0xe4e3..=0xe4e7)
            .chain([0xe1e7, 0xe883, 0xe4c0, 0xe53e, 0xe53f])
            .chain([0xe4a0, 0xe4a2, 0xe4a4, 0xe4ac, 0xe520, 0xe521, 0xe522])
        {
            let character = char::from_u32(code).unwrap();
            let id = face.glyph_index(character).expect("required SMuFL glyph");
            let mut outline = Outline::default();
            face.outline_glyph(id, &mut outline)
                .expect("SMuFL glyph outline");
            let ink = face.glyph_bounding_box(id).expect("SMuFL glyph bounds");
            outlines.insert(
                character,
                (
                    outline,
                    f32::from(face.glyph_hor_advance(id).expect("glyph advance")) * scale,
                    Bounds::new(
                        point(
                            px(f32::from(ink.x_min) * scale),
                            px(-f32::from(ink.y_max) * scale),
                        ),
                        size(
                            px(f32::from(ink.x_max - ink.x_min) * scale),
                            px(f32::from(ink.y_max - ink.y_min) * scale),
                        ),
                    ),
                ),
            );
        }
        Self { outlines, scale }
    }

    /// Visible ink relative to the baseline, including bearings and all glyphs.
    /// UI controls use this to center symbols without clipping forte descenders
    /// or assuming a text font's ascender and line height.
    pub fn bounds(&self, text: &str, size: f32) -> Bounds<Pixels> {
        let factor = size / score_layout::MUSIC_SIZE;
        let mut advance = px(0.);
        let mut bounds: Option<Bounds<Pixels>> = None;
        for character in text.chars() {
            let (_, width, ink) = self.outlines.get(&character).expect("cached SMuFL glyph");
            let ink = Bounds::new(
                point(advance + ink.origin.x * factor, ink.origin.y * factor),
                gpui::size(ink.size.width * factor, ink.size.height * factor),
            );
            bounds = Some(bounds.map_or(ink, |bounds| bounds.union(&ink)));
            advance += px(*width * factor);
        }
        bounds.unwrap_or_default()
    }

    pub fn paint_sized(
        &self,
        text: &str,
        mut baseline: Point<Pixels>,
        color: u32,
        size: f32,
        window: &mut Window,
    ) {
        let factor = size / score_layout::MUSIC_SIZE;
        for character in text.chars() {
            let (outline, advance, _) = self.outlines.get(&character).expect("cached SMuFL glyph");
            let coordinate = |x: f32, y: f32| {
                baseline + point(px(x * self.scale * factor), px(-y * self.scale * factor))
            };
            let mut path = PathBuilder::fill().with_style(PathStyle::Fill(
                FillOptions::default().with_fill_rule(FillRule::NonZero),
            ));
            for segment in &outline.segments {
                match *segment {
                    Segment::Move(x, y) => path.move_to(coordinate(x, y)),
                    Segment::Line(x, y) => path.line_to(coordinate(x, y)),
                    Segment::Quad(x1, y1, x, y) => {
                        path.curve_to(coordinate(x, y), coordinate(x1, y1))
                    }
                    Segment::Cubic(x1, y1, x2, y2, x, y) => path.cubic_bezier_to(
                        coordinate(x, y),
                        coordinate(x1, y1),
                        coordinate(x2, y2),
                    ),
                    Segment::Close => path.close(),
                }
            }
            if let Ok(path) = path.build() {
                window.paint_path(path, rgb(color));
            }
            baseline.x += px(*advance * factor);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bundled_font_contains_outlines_for_all_editor_symbols() {
        let font = MusicFont::new();
        assert_eq!(font.outlines.len(), 49);
        assert!(font.outlines.values().all(
            |(outline, advance, bounds)| !outline.segments.is_empty()
                && *advance > 0.
                && bounds.size.width > px(0.)
                && bounds.size.height > px(0.)
        ));
    }
}
