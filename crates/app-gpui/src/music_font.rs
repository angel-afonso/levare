//! GPUI 0.2.2 rejects fonts without a Latin 'm' on macOS. Paint the original
//! SMuFL outlines directly instead of depending on its text-font fallback.
use gpui::{Bounds, FillOptions, FillRule, Path, Pixels, Point, Window, point, px, rgb, size};
#[cfg(test)]
use gpui::{PathBuilder, PathStyle};
use lyon::tessellation::{BuffersBuilder, FillTessellator, FillVertex, VertexBuffers};
use std::{cell::RefCell, collections::HashMap, rc::Rc};
use ttf_parser::{Face, OutlineBuilder};

use score_layout::glyph_codes;
pub use score_layout::{TextRun, bravura_character, text_runs};

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

/// A glyph tessellated at one size and orientation. Every notehead of a
/// score shares one; painting only moves its triangles to the baseline.
struct Shape {
    start: Point<Pixels>,
    /// In GPUI's fill order.
    triangles: Vec<[Point<Pixels>; 3]>,
}
impl Shape {
    fn placed(&self, baseline: Point<Pixels>) -> impl Iterator<Item = [Point<Pixels>; 3]> {
        self.triangles
            .iter()
            .map(move |t| t.map(|vertex| vertex + baseline))
    }
    fn path(&self, baseline: Point<Pixels>) -> Path<Pixels> {
        let mut path = Path::new(self.start + baseline);
        for [a, b, c] in self.placed(baseline) {
            path.push_triangle((a, b, c), (point(0., 1.), point(0., 1.), point(0., 1.)));
        }
        path
    }
}

/// Same lyon path, options and triangle order as GPUI's `PathBuilder::fill`.
fn tessellate(outline: &Outline, unit: f32, rotated: bool, at: Point<Pixels>) -> Option<Shape> {
    let coordinate = |x: f32, y: f32| {
        let (x, y) = if rotated { (-y, -x) } else { (x, -y) };
        lyon::math::Point::from(at + point(px(x * unit), px(y * unit)))
    };
    let mut path = lyon::path::Path::builder().with_svg();
    for segment in &outline.segments {
        match *segment {
            Segment::Move(x, y) => {
                path.move_to(coordinate(x, y));
            }
            Segment::Line(x, y) => {
                path.line_to(coordinate(x, y));
            }
            Segment::Quad(x1, y1, x, y) => {
                path.quadratic_bezier_to(coordinate(x1, y1), coordinate(x, y));
            }
            Segment::Cubic(x1, y1, x2, y2, x, y) => {
                path.cubic_bezier_to(coordinate(x1, y1), coordinate(x2, y2), coordinate(x, y));
            }
            Segment::Close => path.close(),
        }
    }
    let mut buffers: VertexBuffers<lyon::math::Point, u16> = VertexBuffers::new();
    FillTessellator::new()
        .tessellate_path(
            &path.build(),
            &FillOptions::default().with_fill_rule(FillRule::NonZero),
            &mut BuffersBuilder::new(&mut buffers, |vertex: FillVertex| vertex.position()),
        )
        .ok()?;
    let start = buffers
        .vertices
        .first()
        .copied()
        .map_or_else(Point::default, Into::into);
    Some(Shape {
        start,
        triangles: buffers
            .indices
            .as_chunks::<3>()
            .0
            .iter()
            .map(|triangle| triangle.map(|index| buffers.vertices[usize::from(index)].into()))
            .collect(),
    })
}

/// Distinct sizes come from zoom steps and stretched braces; forgetting them
/// all now and then keeps a long session's cache small.
const MAX_SHAPES: usize = 4096;
/// Tessellations keyed by glyph, pixel size (exact bits) and rotation.
type Shapes = HashMap<(char, u32, bool), Option<Rc<Shape>>>;

pub struct MusicFont {
    outlines: HashMap<char, (Outline, f32, Bounds<Pixels>)>,
    scale: f32,
    shapes: RefCell<Shapes>,
    pub engraving: EngravingMetrics,
}

pub struct EngravingMetrics {
    pub bracket: f32,
    pub tie_endpoint: f32,
    pub tie_middle: f32,
    pub slur_endpoint: f32,
    pub slur_middle: f32,
}
impl MusicFont {
    pub fn new() -> Self {
        let face = Face::parse(include_bytes!("../../../assets/fonts/Bravura.otf"), 0)
            .expect("bundled Bravura font");
        let scale = score_layout::MUSIC_SIZE / f32::from(face.units_per_em());
        let mut outlines = HashMap::new();
        for code in glyph_codes() {
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
        let metadata: serde_json::Value =
            serde_json::from_str(include_str!("../../../assets/fonts/bravura_metadata.json"))
                .expect("bundled Bravura metadata");
        let metric = |name: &str| {
            metadata["engravingDefaults"][name]
                .as_f64()
                .expect("Bravura engraving default") as f32
                * score_layout::STAFF_SPACE
        };
        Self {
            outlines,
            scale,
            shapes: RefCell::default(),
            engraving: EngravingMetrics {
                bracket: metric("bracketThickness"),
                tie_endpoint: metric("tieEndpointThickness"),
                tie_middle: metric("tieMidpointThickness"),
                slur_endpoint: metric("slurEndpointThickness"),
                slur_middle: metric("slurMidpointThickness"),
            },
        }
    }

    /// Whether every character of `text` is among the preloaded outlines.
    #[cfg(test)]
    pub fn covers(&self, text: &str) -> bool {
        text.chars()
            .all(|character| self.outlines.contains_key(&character))
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
        baseline: Point<Pixels>,
        color: u32,
        size: f32,
        window: &mut Window,
    ) {
        self.paint_colored(text, baseline, rgb(color).into(), size, window);
    }

    pub fn paint_colored(
        &self,
        text: &str,
        baseline: Point<Pixels>,
        color: gpui::Hsla,
        size: f32,
        window: &mut Window,
    ) {
        self.paint_oriented(text, baseline, color, size, false, window);
    }

    /// `unit` is the on-screen size of one font unit, before display scaling.
    fn shape(&self, character: char, unit: f32, rotated: bool) -> Option<Rc<Shape>> {
        let mut shapes = self.shapes.borrow_mut();
        if shapes.len() >= MAX_SHAPES {
            shapes.clear();
        }
        shapes
            .entry((character, unit.to_bits(), rotated))
            .or_insert_with(|| {
                let (outline, _, _) = self.outlines.get(&character).expect("cached SMuFL glyph");
                tessellate(outline, unit, rotated, Point::default()).map(Rc::new)
            })
            .clone()
    }

    pub fn paint_oriented(
        &self,
        text: &str,
        mut baseline: Point<Pixels>,
        color: gpui::Hsla,
        size: f32,
        rotated: bool,
        window: &mut Window,
    ) {
        let factor = size / score_layout::MUSIC_SIZE;
        for character in text.chars() {
            let (_, advance, _) = self.outlines.get(&character).expect("cached SMuFL glyph");
            if let Some(shape) = self.shape(character, self.scale * factor, rotated) {
                window.paint_path(shape.path(baseline), color);
            }
            if rotated {
                baseline.y -= px(*advance * factor);
            } else {
                baseline.x += px(*advance * factor);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The pre-cache painting route: GPUI's own builder at the final position.
    fn builder_path(outline: &Outline, unit: f32, rotated: bool, at: Point<Pixels>) -> String {
        let coordinate = |x: f32, y: f32| {
            let (x, y) = if rotated { (-y, -x) } else { (x, -y) };
            at + point(px(x * unit), px(y * unit))
        };
        let mut path = PathBuilder::fill().with_style(PathStyle::Fill(
            FillOptions::default().with_fill_rule(FillRule::NonZero),
        ));
        for segment in &outline.segments {
            match *segment {
                Segment::Move(x, y) => path.move_to(coordinate(x, y)),
                Segment::Line(x, y) => path.line_to(coordinate(x, y)),
                Segment::Quad(x1, y1, x, y) => path.curve_to(coordinate(x, y), coordinate(x1, y1)),
                Segment::Cubic(x1, y1, x2, y2, x, y) => {
                    path.cubic_bezier_to(coordinate(x, y), coordinate(x1, y1), coordinate(x2, y2))
                }
                Segment::Close => path.close(),
            }
        }
        format!("{:?}", path.build().unwrap())
    }

    /// Covered area and bounding box of a triangle list.
    fn ink(triangles: &[[Point<Pixels>; 3]]) -> (f32, f32, f32, f32, f32) {
        let xy = |p: &Point<Pixels>| (f32::from(p.x), f32::from(p.y));
        let area = triangles
            .iter()
            .map(|t| {
                let ((x0, y0), (x1, y1), (x2, y2)) = (xy(&t[0]), xy(&t[1]), xy(&t[2]));
                ((x1 - x0) * (y2 - y0) - (x2 - x0) * (y1 - y0)).abs() / 2.
            })
            .sum();
        triangles.iter().flatten().map(xy).fold(
            (
                area,
                f32::INFINITY,
                f32::INFINITY,
                f32::NEG_INFINITY,
                f32::NEG_INFINITY,
            ),
            |(a, l, t, r, b), (x, y)| (a, l.min(x), t.min(y), r.max(x), b.max(y)),
        )
    }

    #[test]
    fn cached_glyph_shapes_match_gpui_tessellation_at_any_position() {
        let font = MusicFont::new();
        let at = point(px(312.37), px(-48.91));
        for (character, (outline, _, _)) in &font.outlines {
            for (unit, rotated) in [(font.scale, false), (font.scale * 0.37, true)] {
                // Same lyon path and options as GPUI's builder, triangle for triangle.
                let in_place = tessellate(outline, unit, rotated, at).unwrap();
                assert_eq!(
                    format!("{:?}", in_place.path(point(px(0.), px(0.)))),
                    builder_path(outline, unit, rotated, at),
                    "{character:?}"
                );
                // Tessellating at the baseline origin and moving the triangles covers
                // the same ink. Rounding may tip lyon to another diagonal on a near
                // tie, so compare covered area and extent rather than triangles.
                let moved = tessellate(outline, unit, rotated, Point::default()).unwrap();
                let moved: Vec<_> = moved.placed(at).collect();
                let (a, b) = (ink(&moved), ink(&in_place.triangles));
                assert!(
                    (a.0 - b.0).abs() <= b.0 * 1e-4,
                    "{character:?}: {a:?} {b:?}"
                );
                for (a, b) in [(a.1, b.1), (a.2, b.2), (a.3, b.3), (a.4, b.4)] {
                    assert!((a - b).abs() < 1e-3, "{character:?}");
                }
            }
        }
    }

    #[test]
    fn each_glyph_size_and_orientation_is_tessellated_once() {
        let font = MusicFont::new();
        let notehead = '\u{e0a4}';
        let first = font.shape(notehead, 1., false).unwrap();
        assert!(Rc::ptr_eq(
            &first,
            &font.shape(notehead, 1., false).unwrap()
        ));
        assert!(!Rc::ptr_eq(
            &first,
            &font.shape(notehead, 1.5, false).unwrap()
        ));
        assert!(!Rc::ptr_eq(
            &first,
            &font.shape(notehead, 1., true).unwrap()
        ));
        assert_eq!(font.shapes.borrow().len(), 3);
    }
    #[test]
    fn written_tempo_beats_all_use_loaded_bravura_outlines() {
        let font = MusicFont::new();
        for beat in score_core::TempoBeat::ALL {
            assert!(font.covers(beat.glyph()));
        }
    }

    #[test]
    fn bundled_font_contains_outlines_for_all_editor_symbols() {
        let font = MusicFont::new();
        assert_eq!(font.outlines.len(), 97);
        assert!(font.covers("\u{e069}\u{e044}\u{e047}\u{e048}"));
        // Thirty-second notes, flags and rests, and the grace note symbols.
        assert!(font.covers("\u{e1db}\u{e1dc}\u{e244}\u{e245}\u{e4e8}\u{e560}\u{e562}"));
        assert!(font.covers("\u{e511}\u{e51c}\u{e650}\u{e566}\u{eaa4}\u{e585}"));
        assert!(font.outlines.values().all(
            |(outline, advance, bounds)| !outline.segments.is_empty()
                && *advance > 0.
                && bounds.size.width > px(0.)
                && bounds.size.height > px(0.)
        ));
    }

    #[test]
    fn mixed_labels_route_all_notation_to_bravura_and_preserve_text() {
        assert_eq!(
            text_runs("Mi♭ mayor · F♯4 · B♮3"),
            vec![
                TextRun::Text("Mi".into()),
                TextRun::Music("\u{e260}".into()),
                TextRun::Text(" mayor · F".into()),
                TextRun::Music("\u{e262}".into()),
                TextRun::Text("4 · B".into()),
                TextRun::Music("\u{e261}".into()),
                TextRun::Text("3".into()),
            ]
        );
        let font = MusicFont::new();
        for mode in [score_core::KeyMode::Major, score_core::KeyMode::Minor] {
            for fifths in -7..=7 {
                let key = score_core::KeySignature::new(fifths, mode).unwrap();
                for run in text_runs(key.label()) {
                    match run {
                        TextRun::Music(text) => assert!(font.covers(&text)),
                        TextRun::Text(text) => {
                            assert!(!text.chars().any(|c| matches!(c, '♭' | '♮' | '♯')))
                        }
                    }
                }
            }
        }
        assert_eq!(
            text_runs("pizz. · arco · 2ª · Voz 1 · ⌘\u{f8ff}"),
            vec![TextRun::Text(
                "pizz. · arco · 2ª · Voz 1 · ⌘\u{f8ff}".into()
            )]
        );
        assert!(font.covers(&bravura_character('♩').unwrap().to_string()));
        assert!(font.covers(&bravura_character('𝄞').unwrap().to_string()));
    }

    #[test]
    fn every_catalog_mark_and_example_score_uses_loaded_bravura_outlines() {
        let font = MusicFont::new();
        for mark in score_core::MarkKind::ALL {
            if let Some(symbol) = mark.symbol() {
                assert!(font.covers(symbol), "{}", mark.name());
            }
        }
        let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples");
        let mut count = 0;
        for entry in std::fs::read_dir(directory).unwrap() {
            let path = entry.unwrap().path();
            if path
                .extension()
                .is_none_or(|extension| extension != "partitura")
            {
                continue;
            }
            count += 1;
            let score = score_io::load(&path).unwrap();
            for width in [500., 1420.] {
                let layout = score_layout::LayoutEngine::default().layout(&score, width);
                for measure in &layout.measures {
                    for glyph in &measure.content.glyphs {
                        assert!(
                            font.covers(&glyph.text),
                            "{}: {:?}",
                            path.display(),
                            glyph.text
                        );
                    }
                }
                for brace in &layout.braces {
                    let glyph = if brace.bottom - brace.top > 200. {
                        "\u{f402}"
                    } else {
                        "\u{f401}"
                    };
                    assert!(font.covers(glyph));
                }
                assert!(font.covers("\u{e003}\u{e004}"));
            }
        }
        assert!(count >= 13, "the complete example corpus must be checked");
    }
}
