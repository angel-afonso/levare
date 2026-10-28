//! Pure-Rust backend for every OS: krilla writes the PDF, rustybuzz shapes text, and
//! the bundled Liberation Serif, metric-compatible with Times New Roman, is embedded
//! as a subset.
use crate::painter::{self, Canvas, Serif};
use krilla::{
    Document,
    geom::{PathBuilder, Point, Transform},
    metadata::Metadata,
    page::PageSettings,
    paint::{Fill, Stroke},
    surface::Surface,
    text::{Font, GlyphId, KrillaGlyph},
};
use score_core::Score;
use std::{io::Write, path::Path};

pub fn export(score: &Score, path: &Path, part: bool) -> Result<usize, String> {
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let mut temp = tempfile::NamedTempFile::new_in(parent).map_err(|e| e.to_string())?;
    let (bytes, pages) = render(score, part)?;
    temp.write_all(&bytes).map_err(|e| e.to_string())?;
    temp.as_file().sync_all().map_err(|e| e.to_string())?;
    temp.persist(path).map_err(|e| e.to_string())?;
    Ok(pages)
}

/// The finished PDF document and its page count, one page per layout page.
pub(crate) fn render(score: &Score, part: bool) -> Result<(Vec<u8>, usize), String> {
    let layout = painter::layout(score, part);
    if layout.pages.is_empty() {
        return Err("El PDF no contiene todas las páginas".into());
    }
    let paper = painter::paper(score, part);
    let face = painter::bravura()?;
    let engraving = painter::engraving();
    let fonts = Fonts::new()?;
    let mut document = Document::new();
    let mut metadata = Metadata::new().creator("Levare".into());
    if !score.title().is_empty() {
        metadata = metadata.title(score.title().into());
    }
    document.set_metadata(metadata);
    let settings =
        PageSettings::from_wh(paper.0 as f32, paper.1 as f32).ok_or("Tamaño de página inválido")?;
    for index in 0..layout.pages.len() {
        let mut page = document.start_page_with(settings.clone());
        let mut canvas = Krilla {
            surface: page.surface(),
            fonts: &fonts,
            path: PathBuilder::new(),
            pushed: vec![0],
        };
        painter::page(
            &mut canvas,
            score,
            &layout,
            index,
            part,
            paper,
            &face,
            &engraving,
        );
        canvas.finish();
        page.finish();
    }
    let bytes = document
        .finish()
        .map_err(|e| format!("No se pudo crear el PDF: {e:?}"))?;
    Ok((bytes, layout.pages.len()))
}

struct Style {
    shaper: rustybuzz::Face<'static>,
    font: Font,
}

/// The four Liberation Serif styles, ready to shape and embed.
struct Fonts([Style; 4]);
impl Fonts {
    fn new() -> Result<Self, String> {
        let load = |data: &'static [u8]| {
            Some(Style {
                shaper: rustybuzz::Face::from_slice(data, 0)?,
                font: Font::new(data.into(), 0)?,
            })
        };
        let [regular, bold, italic, bold_italic] = crate::SERIF_FONTS;
        Ok(Self([
            load(regular).ok_or("Fuente del texto dañada")?,
            load(bold).ok_or("Fuente del texto dañada")?,
            load(italic).ok_or("Fuente del texto dañada")?,
            load(bold_italic).ok_or("Fuente del texto dañada")?,
        ]))
    }
    fn get(&self, style: Serif) -> &Style {
        &self.0[match style {
            Serif::Regular => 0,
            Serif::Bold => 1,
            Serif::Italic => 2,
            Serif::BoldItalic => 3,
        }]
    }
    /// Shaped glyphs with advances in em, and the line's width in em.
    fn shape(&self, text: &str, style: Serif) -> (Vec<KrillaGlyph>, f32) {
        let face = &self.get(style).shaper;
        let mut buffer = rustybuzz::UnicodeBuffer::new();
        buffer.push_str(text);
        buffer.guess_segment_properties();
        let output = rustybuzz::shape(face, &[], buffer);
        let em = face.units_per_em() as f32;
        // A cluster ends where the next one in the text begins, whatever the direction.
        let mut starts: Vec<usize> = output
            .glyph_infos()
            .iter()
            .map(|info| info.cluster as usize)
            .collect();
        starts.sort_unstable();
        starts.dedup();
        let mut width = 0.;
        let glyphs = output
            .glyph_infos()
            .iter()
            .zip(output.glyph_positions())
            .map(|(info, position)| {
                let start = info.cluster as usize;
                let end = starts
                    .iter()
                    .find(|&&next| next > start)
                    .copied()
                    .unwrap_or(text.len());
                width += position.x_advance as f32 / em;
                KrillaGlyph::new(
                    GlyphId::new(info.glyph_id),
                    position.x_advance as f32 / em,
                    position.x_offset as f32 / em,
                    position.y_offset as f32 / em,
                    position.y_advance as f32 / em,
                    start..end,
                    None,
                )
            })
            .collect();
        (glyphs, width)
    }
}

struct Krilla<'s, 'f> {
    surface: Surface<'s>,
    fonts: &'f Fonts,
    path: PathBuilder,
    /// Transforms pushed since each open `save`; the first entry is the page's own.
    pushed: Vec<usize>,
}
impl Krilla<'_, '_> {
    fn push(&mut self, transform: Transform) {
        self.surface.push_transform(&transform);
        *self.pushed.last_mut().expect("page transform level") += 1;
    }
    fn draw(&mut self, fill: Option<Fill>, stroke: Option<Stroke>) {
        let Some(path) = std::mem::take(&mut self.path).finish() else {
            return;
        };
        self.surface.set_fill(fill);
        self.surface.set_stroke(stroke);
        self.surface.draw_path(&path);
    }
    fn finish(mut self) {
        while !self.pushed.is_empty() {
            self.restore();
        }
        self.surface.finish();
    }
}
impl Canvas for Krilla<'_, '_> {
    fn save(&mut self) {
        self.pushed.push(0);
    }
    fn restore(&mut self) {
        for _ in 0..self.pushed.pop().unwrap_or_default() {
            self.surface.pop();
        }
    }
    fn translate(&mut self, x: f64, y: f64) {
        self.push(Transform::from_translate(x as f32, y as f32));
    }
    fn scale(&mut self, x: f64, y: f64) {
        self.push(Transform::from_scale(x as f32, y as f32));
    }
    fn move_to(&mut self, x: f64, y: f64) {
        self.path.move_to(x as f32, y as f32);
    }
    fn line_to(&mut self, x: f64, y: f64) {
        self.path.line_to(x as f32, y as f32);
    }
    fn quad_to(&mut self, x1: f64, y1: f64, x: f64, y: f64) {
        self.path.quad_to(x1 as f32, y1 as f32, x as f32, y as f32);
    }
    fn curve_to(&mut self, x1: f64, y1: f64, x2: f64, y2: f64, x: f64, y: f64) {
        self.path.cubic_to(
            x1 as f32, y1 as f32, x2 as f32, y2 as f32, x as f32, y as f32,
        );
    }
    fn close(&mut self) {
        self.path.close();
    }
    fn fill(&mut self) {
        self.draw(Some(Fill::default()), None);
    }
    fn stroke(&mut self, width: f64) {
        self.draw(
            None,
            Some(Stroke {
                width: width as f32,
                ..Stroke::default()
            }),
        );
    }
    fn text_width(&mut self, text: &str, style: Serif, size: f64) -> f64 {
        f64::from(self.fonts.shape(text, style).1) * size
    }
    fn show_text(&mut self, text: &str, style: Serif, size: f64) {
        let (glyphs, _) = self.fonts.shape(text, style);
        self.surface.set_fill(Some(Fill::default()));
        self.surface.set_stroke(None);
        self.surface.draw_glyphs(
            Point::from_xy(0., 0.),
            &glyphs,
            self.fonts.get(style).font.clone(),
            text,
            size as f32,
            false,
        );
    }
}
