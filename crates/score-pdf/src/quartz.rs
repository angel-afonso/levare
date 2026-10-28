//! macOS backend: Quartz writes the PDF and CoreText sets text in Times New Roman.
use crate::painter::{self, Canvas, Serif};
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
use std::{path::Path, ptr};

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
    let paper = painter::paper(score, part);
    let media = CGRect::new(&CGPoint::new(0., 0.), &CGSize::new(paper.0, paper.1));
    let raw = unsafe { CGPDFContextCreateWithURL(url.as_concrete_TypeRef(), &media, ptr::null()) };
    if raw.is_null() {
        return Err("No se pudo crear el PDF".into());
    }
    // This owns the create-rule reference; dropping it releases the context.
    let context = unsafe { CGContext::from_ptr(raw) };
    let layout = painter::layout(score, part);
    let face = painter::bravura()?;
    let engraving = painter::engraving();
    for index in 0..layout.pages.len() {
        unsafe {
            CGPDFContextBeginPage(raw, ptr::null());
        }
        context.save();
        // Quartz is y-up; the painter draws y-down from the top of the paper.
        context.translate(0., paper.1);
        context.scale(1., -1.);
        context.set_rgb_fill_color(0., 0., 0., 1.);
        context.set_rgb_stroke_color(0., 0., 0., 1.);
        painter::page(
            &mut Quartz { context: &context },
            score,
            &layout,
            index,
            part,
            paper,
            &face,
            &engraving,
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
    if page_count(temp.path()) != Some(layout.pages.len()) {
        return Err("El PDF no contiene todas las páginas".into());
    }
    temp.persist(path).map_err(|e| e.to_string())?;
    Ok(layout.pages.len())
}

/// The number of pages Quartz reads from a PDF, or `None` if it can't open it.
pub(crate) fn page_count(path: &Path) -> Option<usize> {
    let url = CFURL::from_path(path, false)?;
    let pdf = unsafe { CGPDFDocumentCreateWithURL(url.as_concrete_TypeRef()) };
    if pdf.is_null() {
        return None;
    }
    let pages = unsafe { CGPDFDocumentGetNumberOfPages(pdf) };
    unsafe {
        CGPDFDocumentRelease(pdf);
    }
    Some(pages)
}

fn postscript_name(style: Serif) -> &'static str {
    match style {
        Serif::Regular => "TimesNewRomanPSMT",
        Serif::Bold => "TimesNewRomanPS-BoldMT",
        Serif::Italic => "TimesNewRomanPS-ItalicMT",
        Serif::BoldItalic => "TimesNewRomanPS-BoldItalicMT",
    }
}

fn text_line(text: &str, style: Serif, size: f64) -> CTLine {
    let font = font::new_from_name(postscript_name(style), size).expect("system Times font");
    let mut attributed = CFMutableAttributedString::new();
    attributed.replace_str(&CFString::new(text), CFRange::init(0, 0));
    attributed.set_attribute(
        CFRange::init(0, attributed.char_len()),
        unsafe { kCTFontAttributeName },
        &font,
    );
    CTLine::new_with_attributed_string(attributed.as_concrete_TypeRef())
}

struct Quartz<'a> {
    context: &'a CGContext,
}
impl Canvas for Quartz<'_> {
    fn save(&mut self) {
        self.context.save();
    }
    fn restore(&mut self) {
        self.context.restore();
    }
    fn translate(&mut self, x: f64, y: f64) {
        self.context.translate(x, y);
    }
    fn scale(&mut self, x: f64, y: f64) {
        self.context.scale(x, y);
    }
    fn move_to(&mut self, x: f64, y: f64) {
        self.context.move_to_point(x, y);
    }
    fn line_to(&mut self, x: f64, y: f64) {
        self.context.add_line_to_point(x, y);
    }
    fn quad_to(&mut self, x1: f64, y1: f64, x: f64, y: f64) {
        self.context.add_quad_curve_to_point(x1, y1, x, y);
    }
    fn curve_to(&mut self, x1: f64, y1: f64, x2: f64, y2: f64, x: f64, y: f64) {
        self.context.add_curve_to_point(x1, y1, x2, y2, x, y);
    }
    fn close(&mut self) {
        self.context.close_path();
    }
    fn fill(&mut self) {
        self.context.fill_path();
    }
    fn stroke(&mut self, width: f64) {
        self.context.set_line_width(width);
        self.context.stroke_path();
    }
    fn fill_rect(&mut self, x: f64, y: f64, width: f64, height: f64) {
        self.context.fill_rect(CGRect::new(
            &CGPoint::new(x, y),
            &CGSize::new(width, height),
        ));
    }
    fn text_width(&mut self, text: &str, style: Serif, size: f64) -> f64 {
        text_line(text, style, size).get_typographic_bounds().width
    }
    fn show_text(&mut self, text: &str, style: Serif, size: f64) {
        self.context.save();
        // CoreText draws y-up.
        self.context.scale(1., -1.);
        self.context
            .set_text_matrix(&CGAffineTransform::new(1., 0., 0., 1., 0., 0.));
        self.context.set_text_position(0., 0.);
        text_line(text, style, size).draw(self.context);
        self.context.restore();
    }
}
