use core_foundation::{
    attributed_string::CFMutableAttributedString,
    base::{CFRange, TCFType},
    dictionary::CFDictionaryRef,
    string::CFString,
    url::{CFURL, CFURLRef},
};
use core_graphics::{
    context::CGContext,
    geometry::{CGAffineTransform, CGPoint, CGRect, CGSize},
};
use core_text::{font, line::CTLine, string_attributes::kCTFontAttributeName};
use foreign_types::ForeignType;
use score_core::Score;
use score_layout::{Layout, LayoutEngine, Line, Point, ViewMode};
use std::{path::Path, ptr};
use ttf_parser::{Face, OutlineBuilder};

#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    fn CGPDFContextCreateWithURL(
        url: CFURLRef,
        media_box: *const CGRect,
        info: CFDictionaryRef,
    ) -> *mut core_graphics::sys::CGContext;
    fn CGPDFContextBeginPage(context: *mut core_graphics::sys::CGContext, info: CFDictionaryRef);
    fn CGPDFContextEndPage(context: *mut core_graphics::sys::CGContext);
    fn CGPDFContextClose(context: *mut core_graphics::sys::CGContext);
    fn CGPDFDocumentCreateWithURL(url: CFURLRef) -> *mut std::ffi::c_void;
    fn CGPDFDocumentGetNumberOfPages(document: *mut std::ffi::c_void) -> usize;
    fn CGPDFDocumentRelease(document: *mut std::ffi::c_void);
}

pub fn export(score: &Score, path: &Path, part: bool) -> Result<usize, String> {
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let temp = tempfile::NamedTempFile::new_in(parent).map_err(|e| e.to_string())?;
    let url = CFURL::from_path(temp.path(), false).ok_or("Ruta de PDF inválida")?;
    let (paper_width, paper_height) = if !part && score.parts().len() >= 12 {
        (841.8898, 1190.5512)
    } else {
        (595.2756, 841.8898)
    };
    let media = CGRect::new(
        &CGPoint::new(0., 0.),
        &CGSize::new(paper_width, paper_height),
    );
    let raw = unsafe { CGPDFContextCreateWithURL(url.as_concrete_TypeRef(), &media, ptr::null()) };
    if raw.is_null() {
        return Err("No se pudo crear el PDF".into());
    }
    // This owns the create-rule reference; dropping it releases the context.
    let context = unsafe { CGContext::from_ptr(raw) };
    let mut engine = LayoutEngine::default();
    let layout = if part {
        engine.layout_part(score, 1000., ViewMode::Page)
    } else {
        engine.layout_view(score, 1000., ViewMode::Page)
    };
    let face = Face::parse(include_bytes!("../../../assets/fonts/Bravura.otf"), 0)
        .map_err(|e| e.to_string())?;
    let engraving: serde_json::Value =
        serde_json::from_str(include_str!("../../../assets/fonts/bravura_metadata.json"))
            .expect("bundled Bravura metadata");
    for (index, page) in layout.pages.iter().enumerate() {
        unsafe {
            CGPDFContextBeginPage(raw, ptr::null());
        }
        context.save();
        let scale = (media.size.width / f64::from(page.width))
            .min(media.size.height / f64::from(page.height));
        let left = (media.size.width - f64::from(page.width) * scale) / 2.;
        let top = (media.size.height - f64::from(page.height) * scale) / 2.;
        context.translate(left, media.size.height - top);
        context.scale(scale, -scale);
        context.translate(0., -f64::from(page.top));
        context.set_rgb_fill_color(0., 0., 0., 1.);
        context.set_rgb_stroke_color(0., 0., 0., 1.);
        let painter = Painter {
            context: &context,
            face: &face,
            engraving: &engraving,
        };
        if index == 0 {
            painter.header(score, page.width);
            painter.text(
                &score.credits().copyright,
                "TimesNewRomanPSMT",
                10.,
                f64::from(page.width / 2.),
                f64::from(page.top + page.height - 42.),
                Align::Center,
                Some(f64::from(page.width - 112.)),
            );
        }
        if part {
            painter.text(
                &score.part_name(score.parts()[0].id()),
                "TimesNewRomanPS-BoldMT",
                22.,
                f64::from(page.width / 2.),
                f64::from(page.top) + if index == 0 { 132. } else { 48. },
                Align::Center,
                Some(f64::from(page.width - 112.)),
            );
        }
        painter.text(
            &format!("{} / {}", index + 1, layout.pages.len()),
            "TimesNewRomanPSMT",
            13.,
            f64::from(page.width / 2.),
            f64::from(page.top + page.height - 24.),
            Align::Center,
            None,
        );
        painter.page(
            &layout,
            page.first_measure,
            page.last_measure,
            page.top,
            page.height,
        );
        context.restore();
        unsafe {
            CGPDFContextEndPage(raw);
        }
    }
    unsafe {
        CGPDFContextClose(raw);
    }
    drop(context);
    temp.as_file().sync_all().map_err(|e| e.to_string())?;
    // Quartz reports errors by failing to produce a readable document.
    let pdf_url = CFURL::from_path(temp.path(), false).ok_or("Ruta de PDF inválida")?;
    let pdf = unsafe { CGPDFDocumentCreateWithURL(pdf_url.as_concrete_TypeRef()) };
    if pdf.is_null() {
        return Err("No se pudo finalizar el PDF".into());
    }
    let page_count = unsafe { CGPDFDocumentGetNumberOfPages(pdf) };
    unsafe {
        CGPDFDocumentRelease(pdf);
    }
    if page_count != layout.pages.len() {
        return Err("El PDF no contiene todas las páginas".into());
    }
    temp.persist(path).map_err(|e| e.to_string())?;
    Ok(layout.pages.len())
}

#[derive(Clone, Copy)]
enum Align {
    Left,
    Center,
    Right,
}
impl Align {
    /// How far left of the anchor a line of this width starts.
    fn offset(self, width: f64) -> f64 {
        match self {
            Self::Left => 0.,
            Self::Center => width / 2.,
            Self::Right => width,
        }
    }
}
fn text_line(text: &str, name: &str, size: f64) -> CTLine {
    let font = font::new_from_name(name, size).expect("system Times font");
    let mut attributed = CFMutableAttributedString::new();
    attributed.replace_str(&CFString::new(text), CFRange::init(0, 0));
    attributed.set_attribute(
        CFRange::init(0, attributed.char_len()),
        unsafe { kCTFontAttributeName },
        &font,
    );
    CTLine::new_with_attributed_string(attributed.as_concrete_TypeRef())
}
struct Painter<'a> {
    engraving: &'a serde_json::Value,
    context: &'a CGContext,
    face: &'a Face<'a>,
}
impl Painter<'_> {
    fn metric(&self, key: &str) -> f32 {
        self.engraving["engravingDefaults"][key]
            .as_f64()
            .expect("Bravura engraving default") as f32
            * score_layout::STAFF_SPACE
    }
    fn music_ink(&self, text: &str, size: f32) -> (f32, f32) {
        let scale = size / f32::from(self.face.units_per_em());
        let (mut left, mut right, mut top, mut bottom) = (
            f32::INFINITY,
            f32::NEG_INFINITY,
            f32::INFINITY,
            f32::NEG_INFINITY,
        );
        let mut advance = 0.;
        for ch in text.chars() {
            let id = self.face.glyph_index(ch).unwrap();
            if let Some(rect) = self.face.glyph_bounding_box(id) {
                left = left.min(f32::from(rect.x_min) * scale + advance);
                right = right.max(f32::from(rect.x_max) * scale + advance);
                top = top.min(-f32::from(rect.y_max) * scale);
                bottom = bottom.max(-f32::from(rect.y_min) * scale);
            }
            advance += f32::from(self.face.glyph_hor_advance(id).unwrap_or_default()) * scale;
        }
        (right - left, bottom - top)
    }
    #[allow(clippy::too_many_arguments)]
    fn text(
        &self,
        text: &str,
        name: &str,
        size: f64,
        x: f64,
        y: f64,
        align: Align,
        max_width: Option<f64>,
    ) -> f64 {
        if text.is_empty() {
            return 0.;
        }
        let runs = score_layout::text_runs(text);
        if runs
            .iter()
            .any(|r| matches!(r, score_layout::TextRun::Music(_)))
        {
            let widths: Vec<_> = runs
                .iter()
                .map(|r| match r {
                    score_layout::TextRun::Text(t) => {
                        text_line(t, name, size).get_typographic_bounds().width
                    }
                    score_layout::TextRun::Music(t) => {
                        let (_, h) = self.music_ink(t, 1.);
                        f64::from(self.music_ink(t, size as f32 * 0.85 / h).0) + size * 0.08
                    }
                })
                .collect();
            let width: f64 = widths.iter().sum();
            let factor = max_width.map_or(1., |max| (max / width.max(1.)).min(1.));
            self.context.save();
            self.context.translate(x - align.offset(width * factor), y);
            self.context.scale(factor, factor);
            let mut at = 0.;
            for (run, advance) in runs.iter().zip(widths) {
                match run {
                    score_layout::TextRun::Text(t) => {
                        self.text(t, name, size, at, 0., Align::Left, None);
                    }
                    score_layout::TextRun::Music(t) => {
                        let (_, height) = self.music_ink(t, 1.);
                        let music_size = size as f32 * 0.85 / height;
                        let bottom = t
                            .chars()
                            .filter_map(|c| self.face.glyph_bounding_box(self.face.glyph_index(c)?))
                            .map(|b| {
                                -f32::from(b.y_min) * music_size
                                    / f32::from(self.face.units_per_em())
                            })
                            .fold(0., f32::max);
                        self.glyph(t, at as f32, size as f32 * 0.08 - bottom, music_size, false);
                    }
                }
                at += advance;
            }
            self.context.restore();
            return width * factor;
        }
        let line = text_line(text, name, size);
        let width = line.get_typographic_bounds().width;
        let factor = max_width.map_or(1., |max| (max / width.max(1.)).min(1.));
        self.context.save();
        self.context.translate(x - align.offset(width * factor), y);
        self.context.scale(factor, -factor);
        self.context
            .set_text_matrix(&CGAffineTransform::new(1., 0., 0., 1., 0., 0.));
        self.context.set_text_position(0., 0.);
        line.draw(self.context);
        self.context.restore();
        width * factor
    }
    /// Title, subtitle, and the credit columns: lyricist left, composer and
    /// arranger right, as engraved scores lay them out.
    fn header(&self, score: &Score, width: f32) {
        let lines = self.title(score.title(), width);
        let credits = score.credits();
        let below = 64. + lines as f64 * 40.;
        self.text(
            &credits.subtitle,
            "TimesNewRomanPSMT",
            18.,
            f64::from(width / 2.),
            below - 8.,
            Align::Center,
            Some(f64::from(width - 112.)),
        );
        // Fixed rows keep the credits inside the header whatever the title.
        let column = Some(f64::from(width / 2. - 72.));
        let row = 130.;
        for (i, line) in [credits.composer_line(), credits.arranger_line()]
            .iter()
            .filter(|line| !line.is_empty())
            .enumerate()
        {
            self.text(
                line,
                "TimesNewRomanPSMT",
                13.,
                f64::from(width - 56.),
                row + i as f64 * 17.,
                Align::Right,
                column,
            );
        }
        self.text(
            &credits.lyricist_line(),
            "TimesNewRomanPSMT",
            13.,
            56.,
            row,
            Align::Left,
            column,
        );
    }
    fn title(&self, title: &str, width: f32) -> usize {
        // Two lines keep long titles legible while staying inside the header.
        let words: Vec<_> = title.split_whitespace().collect();
        let split = if title.chars().count() > 48 && words.len() > 1 {
            Some(words.len() / 2)
        } else {
            None
        };
        let lines = split.map_or_else(
            || vec![title.to_owned()],
            |i| vec![words[..i].join(" "), words[i..].join(" ")],
        );
        for (i, line) in lines.iter().enumerate() {
            self.text(
                line,
                "TimesNewRomanPSMT",
                36.,
                f64::from(width / 2.),
                64. + i as f64 * 40.,
                Align::Center,
                Some(f64::from(width - 112.)),
            );
        }
        lines.len()
    }
    fn line(&self, line: &Line, offset: Point) {
        self.context.begin_path();
        self.context.set_line_width(f64::from(line.thickness));
        self.context.move_to_point(
            f64::from(line.from.x + offset.x),
            f64::from(line.from.y + offset.y),
        );
        self.context.add_line_to_point(
            f64::from(line.to.x + offset.x),
            f64::from(line.to.y + offset.y),
        );
        self.context.stroke_path();
    }
    fn glyph(&self, text: &str, x: f32, y: f32, size: f32, rotated: bool) -> f32 {
        let scale = size / f32::from(self.face.units_per_em());
        let mut advance = 0.;
        self.context.begin_path();
        for ch in text.chars() {
            let glyph = self
                .face
                .glyph_index(ch)
                .expect("Bravura contains engraved glyph");
            let mut path = Outline {
                context: self.context,
                x,
                y,
                scale,
                advance,
                rotated,
            };
            self.face.outline_glyph(glyph, &mut path);
            advance += f32::from(self.face.glyph_hor_advance(glyph).unwrap_or_default()) * scale;
        }
        self.context.fill_path();
        advance
    }
    fn page(&self, layout: &Layout, first: usize, last: usize, top: f32, height: f32) {
        let on_page = |y: f32| y >= top && y < top + height;
        let zero = Point::default();
        for line in &layout.connectors {
            if on_page(line.from.y) {
                self.line(line, zero);
            }
        }
        for ending in &layout.rehearsals {
            for line in &ending.frame {
                if on_page(line.from.y) {
                    self.line(line, zero);
                }
            }
            if !ending.text.is_empty() && on_page(ending.position.y) {
                self.text(
                    &ending.text,
                    "TimesNewRomanPS-BoldMT",
                    18.,
                    ending.position.x.into(),
                    ending.position.y.into(),
                    Align::Left,
                    None,
                );
            }
        }
        for ending in &layout.endings {
            for line in &ending.lines {
                if on_page(line.from.y) {
                    self.line(line, zero);
                }
            }
            if !ending.label.is_empty() && on_page(ending.label_position.y) {
                self.text(
                    &ending.label,
                    "TimesNewRomanPS-BoldMT",
                    14.,
                    ending.label_position.x.into(),
                    ending.label_position.y.into(),
                    Align::Left,
                    None,
                );
            }
        }
        for label in &layout.navigation {
            if on_page(label.position.y) {
                self.text(
                    &label.text,
                    "TimesNewRomanPS-BoldItalicMT",
                    label.size.into(),
                    label.position.x.into(),
                    label.position.y.into(),
                    if label.right_aligned {
                        Align::Right
                    } else {
                        Align::Left
                    },
                    None,
                );
            }
        }
        for label in &layout.labels {
            if on_page(label.position.y) {
                self.text(
                    &label.text,
                    "TimesNewRomanPSMT",
                    score_layout::STAFF_LABEL_SIZE.into(),
                    label.position.x.into(),
                    label.position.y.into(),
                    Align::Left,
                    None,
                );
            }
        }
        for bracket in &layout.brackets {
            if on_page(bracket.top) {
                self.glyph("\u{e003}", bracket.x, bracket.top, 20., false);
                self.glyph("\u{e004}", bracket.x, bracket.bottom, 20., false);
                self.context.fill_rect(CGRect::new(
                    &CGPoint::new(bracket.x.into(), bracket.top.into()),
                    &CGSize::new(
                        f64::from(self.metric("bracketThickness") * 0.5),
                        (bracket.bottom - bracket.top).into(),
                    ),
                ));
            }
        }
        for brace in &layout.braces {
            if on_page(brace.top) {
                let glyph = if brace.bottom - brace.top > 200. {
                    '\u{f402}'
                } else {
                    '\u{f401}'
                };
                let id = self.face.glyph_index(glyph).unwrap();
                let rect = self.face.glyph_bounding_box(id).unwrap();
                let scale = (brace.bottom - brace.top) / f32::from(rect.y_max - rect.y_min);
                self.glyph(
                    &glyph.to_string(),
                    brace.x + 10. - f32::from(rect.x_max) * scale,
                    brace.top + f32::from(rect.y_max) * scale,
                    scale * f32::from(self.face.units_per_em()),
                    false,
                );
            }
        }
        for measure in layout
            .measures
            .iter()
            .filter(|m| (first..=last).contains(&m.index))
        {
            let offset = measure.origin;
            for line in &measure.content.lines {
                self.line(line, offset);
            }
            for group in &measure.content.beams {
                for segment in &group.segments {
                    self.context.begin_path();
                    for (i, point) in segment.corners.iter().enumerate() {
                        let (x, y) = (f64::from(point.x + offset.x), f64::from(point.y + offset.y));
                        if i == 0 {
                            self.context.move_to_point(x, y);
                        } else {
                            self.context.add_line_to_point(x, y);
                        }
                    }
                    self.context.close_path();
                    self.context.fill_path();
                }
            }
            for label in measure
                .content
                .techniques
                .iter()
                .chain(&measure.content.expressions)
            {
                let (font, size) = if matches!(label.mark, score_core::MarkRef::Chord(_)) {
                    ("TimesNewRomanPSMT", 18.)
                } else {
                    ("TimesNewRomanPS-ItalicMT", 16.)
                };
                self.text(
                    label.text.as_str(),
                    font,
                    size,
                    (label.position.x + offset.x).into(),
                    (label.position.y + offset.y).into(),
                    Align::Left,
                    None,
                );
            }
            for lyric in &measure.content.lyrics {
                self.text(
                    lyric.syllable.text(),
                    "TimesNewRomanPSMT",
                    16.,
                    (lyric.position.x + offset.x).into(),
                    (lyric.position.y + offset.y).into(),
                    Align::Left,
                    None,
                );
            }
            for glyph in &measure.content.glyphs {
                self.glyph(
                    &glyph.text,
                    offset.x + glyph.position.x,
                    offset.y + glyph.position.y,
                    score_layout::MUSIC_SIZE,
                    glyph.rotated,
                );
            }
            for cue in &measure.content.cue {
                self.glyph(
                    &cue.glyph.text,
                    offset.x + cue.glyph.position.x,
                    offset.y + cue.glyph.position.y,
                    score_layout::MUSIC_SIZE * score_layout::CUE,
                    false,
                );
            }
            self.text(
                &(measure.index + 1).to_string(),
                "TimesNewRomanPSMT",
                11.,
                offset.x.into(),
                (offset.y - 18.).into(),
                Align::Left,
                None,
            );
        }
        for mark in &layout.piano_arpeggios {
            if on_page(mark.glyph.position.y) {
                self.glyph(
                    &mark.glyph.text,
                    mark.glyph.position.x,
                    mark.glyph.position.y,
                    score_layout::MUSIC_SIZE,
                    true,
                );
            }
        }
        for hairpin in &layout.hairpins {
            if on_page(hairpin.upper.from.y) {
                self.line(&hairpin.upper, zero);
                self.line(&hairpin.lower, zero);
            }
        }
        for line in &layout.lyric_lines {
            if on_page(line.from.y) {
                self.line(line, zero);
            }
        }
        for line in &layout.line_marks {
            if on_page(line.from.y) {
                for stroke in &line.strokes {
                    self.line(stroke, zero);
                }
                for glyph in &line.glyphs {
                    self.glyph(
                        &glyph.text,
                        glyph.position.x,
                        glyph.position.y,
                        score_layout::MUSIC_SIZE,
                        false,
                    );
                }
            }
        }
        for curve in &layout.curves {
            if on_page(curve.from.y) {
                let (endpoint, middle) = if curve.connection.kind == score_core::ConnectionKind::Tie
                {
                    (
                        self.metric("tieEndpointThickness"),
                        self.metric("tieMidpointThickness"),
                    )
                } else {
                    (
                        self.metric("slurEndpointThickness"),
                        self.metric("slurMidpointThickness"),
                    )
                };
                let control = (middle - endpoint * 0.25) / 1.5;
                let span = curve.to.x - curve.from.x;
                let (a, b) = (curve.from, curve.to);
                self.context.begin_path();
                self.context
                    .move_to_point(a.x.into(), (a.y - endpoint / 2.).into());
                self.context.add_curve_to_point(
                    (a.x + span / 3.).into(),
                    (a.y + curve.bend - control).into(),
                    (b.x - span / 3.).into(),
                    (b.y + curve.bend - control).into(),
                    b.x.into(),
                    (b.y - endpoint / 2.).into(),
                );
                self.context
                    .add_line_to_point(b.x.into(), (b.y + endpoint / 2.).into());
                self.context.add_curve_to_point(
                    (b.x - span / 3.).into(),
                    (b.y + curve.bend + control).into(),
                    (a.x + span / 3.).into(),
                    (a.y + curve.bend + control).into(),
                    a.x.into(),
                    (a.y + endpoint / 2.).into(),
                );
                self.context.close_path();
                self.context.fill_path();
            }
        }
        for tempo in &layout.tempos {
            if (first..=last).contains(&tempo.measure) {
                let mut x = tempo.position.x;
                if !tempo.mark.text.is_empty() {
                    for run in score_layout::text_runs(&tempo.mark.text) {
                        x += match run {
                            score_layout::TextRun::Music(text) => {
                                let (_, height) = self.music_ink(&text, 1.);
                                let size = 18. / height;
                                self.glyph(&text, x, tempo.position.y, size, false);
                                self.music_ink(&text, size).0 + 1.
                            }
                            score_layout::TextRun::Text(text) => self.text(
                                &text,
                                "TimesNewRomanPS-BoldMT",
                                18.,
                                x.into(),
                                tempo.position.y.into(),
                                Align::Left,
                                None,
                            )
                                as f32,
                        };
                    }
                    x += 12.;
                }
                self.glyph(tempo.mark.beat.glyph(), x, tempo.position.y, 28., false);
                x += self.music_ink(tempo.mark.beat.glyph(), 28.).0 + 8.;
                self.text(
                    &format!("= {}", tempo.mark.bpm),
                    "TimesNewRomanPSMT",
                    18.,
                    x.into(),
                    tempo.position.y.into(),
                    Align::Left,
                    None,
                );
            }
        }
    }
}
struct Outline<'a> {
    context: &'a CGContext,
    x: f32,
    y: f32,
    scale: f32,
    advance: f32,
    rotated: bool,
}
impl Outline<'_> {
    fn point(&self, x: f32, y: f32) -> (f64, f64) {
        let (x, y) = if self.rotated {
            (-y * self.scale, -(x * self.scale + self.advance))
        } else {
            (x * self.scale + self.advance, -y * self.scale)
        };
        ((self.x + x).into(), (self.y + y).into())
    }
}
impl OutlineBuilder for Outline<'_> {
    fn move_to(&mut self, x: f32, y: f32) {
        let (x, y) = self.point(x, y);
        self.context.move_to_point(x, y);
    }
    fn line_to(&mut self, x: f32, y: f32) {
        let (x, y) = self.point(x, y);
        self.context.add_line_to_point(x, y);
    }
    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        let (x1, y1) = self.point(x1, y1);
        let (x, y) = self.point(x, y);
        self.context.add_quad_curve_to_point(x1, y1, x, y);
    }
    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        let (x1, y1) = self.point(x1, y1);
        let (x2, y2) = self.point(x2, y2);
        let (x, y) = self.point(x, y);
        self.context.add_curve_to_point(x1, y1, x2, y2, x, y);
    }
    fn close(&mut self) {
        self.context.close_path();
    }
}
