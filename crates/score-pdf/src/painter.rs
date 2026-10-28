//! Draws a laid-out score onto a PDF canvas. Every backend shares this code, so
//! Quartz and the portable writer print the same marks in the same places.
use score_core::Score;
use score_layout::{Layout, LayoutEngine, Line, Point, ViewMode};
use ttf_parser::{Face, OutlineBuilder};

/// The four styles of the text serif.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Serif {
    Regular,
    Bold,
    Italic,
    BoldItalic,
}

/// A page to draw on, in a y-down coordinate space with the origin at the top
/// left of the paper. Marks are always black.
pub(crate) trait Canvas {
    fn save(&mut self);
    fn restore(&mut self);
    fn translate(&mut self, x: f64, y: f64);
    fn scale(&mut self, x: f64, y: f64);
    fn move_to(&mut self, x: f64, y: f64);
    fn line_to(&mut self, x: f64, y: f64);
    fn quad_to(&mut self, x1: f64, y1: f64, x: f64, y: f64);
    fn curve_to(&mut self, x1: f64, y1: f64, x2: f64, y2: f64, x: f64, y: f64);
    fn close(&mut self);
    /// Fills the current path and starts a new one.
    fn fill(&mut self);
    /// Strokes the current path with lines of this width and starts a new one.
    fn stroke(&mut self, width: f64);
    fn fill_rect(&mut self, x: f64, y: f64, width: f64, height: f64) {
        self.move_to(x, y);
        self.line_to(x + width, y);
        self.line_to(x + width, y + height);
        self.line_to(x, y + height);
        self.close();
        self.fill();
    }
    /// The advance width of one line of text.
    fn text_width(&mut self, text: &str, style: Serif, size: f64) -> f64;
    /// Draws one line of text upright, its baseline starting at the origin.
    fn show_text(&mut self, text: &str, style: Serif, size: f64);
}

/// Paper size in points: A3 for full scores of 12 parts or more, A4 otherwise.
pub(crate) fn paper(score: &Score, part: bool) -> (f64, f64) {
    if !part && score.parts().len() >= 12 {
        (841.8898, 1190.5512)
    } else {
        (595.2756, 841.8898)
    }
}

/// Print layout, independent of the screen's zoom and view.
pub(crate) fn layout(score: &Score, part: bool) -> Layout {
    let mut engine = LayoutEngine::default();
    if part {
        engine.layout_part(score, 1000., ViewMode::Page)
    } else {
        engine.layout_view(score, 1000., ViewMode::Page)
    }
}

pub(crate) fn bravura() -> Result<Face<'static>, String> {
    Face::parse(include_bytes!("../../../assets/fonts/Bravura.otf"), 0).map_err(|e| e.to_string())
}

pub(crate) fn engraving() -> serde_json::Value {
    serde_json::from_str(include_str!("../../../assets/fonts/bravura_metadata.json"))
        .expect("bundled Bravura metadata")
}

/// Draws page `index` of `layout`, centred and scaled to fit the paper.
#[allow(clippy::too_many_arguments)]
pub(crate) fn page<C: Canvas>(
    canvas: &mut C,
    score: &Score,
    layout: &Layout,
    index: usize,
    part: bool,
    (paper_width, paper_height): (f64, f64),
    face: &Face,
    engraving: &serde_json::Value,
) {
    let page = &layout.pages[index];
    canvas.save();
    let scale = (paper_width / f64::from(page.width)).min(paper_height / f64::from(page.height));
    let left = (paper_width - f64::from(page.width) * scale) / 2.;
    let top = (paper_height - f64::from(page.height) * scale) / 2.;
    canvas.translate(left, top);
    canvas.scale(scale, scale);
    canvas.translate(0., -f64::from(page.top));
    let mut painter = Painter {
        canvas: &mut *canvas,
        face,
        engraving,
    };
    if index == 0 {
        painter.header(score, page.width);
        painter.text(
            &score.credits().copyright,
            Serif::Regular,
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
            Serif::Bold,
            22.,
            f64::from(page.width / 2.),
            f64::from(page.top) + if index == 0 { 132. } else { 48. },
            Align::Center,
            Some(f64::from(page.width - 112.)),
        );
    }
    painter.text(
        &format!("{} / {}", index + 1, layout.pages.len()),
        Serif::Regular,
        13.,
        f64::from(page.width / 2.),
        f64::from(page.top + page.height - 24.),
        Align::Center,
        None,
    );
    painter.page(
        layout,
        page.first_measure,
        page.last_measure,
        page.top,
        page.height,
    );
    canvas.restore();
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

struct Painter<'a, C> {
    engraving: &'a serde_json::Value,
    canvas: &'a mut C,
    face: &'a Face<'a>,
}
impl<C: Canvas> Painter<'_, C> {
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
        &mut self,
        text: &str,
        style: Serif,
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
                    score_layout::TextRun::Text(t) => self.canvas.text_width(t, style, size),
                    score_layout::TextRun::Music(t) => {
                        let (_, h) = self.music_ink(t, 1.);
                        f64::from(self.music_ink(t, size as f32 * 0.85 / h).0) + size * 0.08
                    }
                })
                .collect();
            let width: f64 = widths.iter().sum();
            let factor = max_width.map_or(1., |max| (max / width.max(1.)).min(1.));
            self.canvas.save();
            self.canvas.translate(x - align.offset(width * factor), y);
            self.canvas.scale(factor, factor);
            let mut at = 0.;
            for (run, advance) in runs.iter().zip(widths) {
                match run {
                    score_layout::TextRun::Text(t) => {
                        self.text(t, style, size, at, 0., Align::Left, None);
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
            self.canvas.restore();
            return width * factor;
        }
        let width = self.canvas.text_width(text, style, size);
        let factor = max_width.map_or(1., |max| (max / width.max(1.)).min(1.));
        self.canvas.save();
        self.canvas.translate(x - align.offset(width * factor), y);
        self.canvas.scale(factor, factor);
        self.canvas.show_text(text, style, size);
        self.canvas.restore();
        width * factor
    }
    /// Title, subtitle, and the credit columns: lyricist left, composer and
    /// arranger right, as engraved scores lay them out.
    fn header(&mut self, score: &Score, width: f32) {
        let lines = self.title(score.title(), width);
        let credits = score.credits();
        let below = 64. + lines as f64 * 40.;
        self.text(
            &credits.subtitle,
            Serif::Regular,
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
                Serif::Regular,
                13.,
                f64::from(width - 56.),
                row + i as f64 * 17.,
                Align::Right,
                column,
            );
        }
        self.text(
            &credits.lyricist_line(),
            Serif::Regular,
            13.,
            56.,
            row,
            Align::Left,
            column,
        );
    }
    fn title(&mut self, title: &str, width: f32) -> usize {
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
                Serif::Regular,
                36.,
                f64::from(width / 2.),
                64. + i as f64 * 40.,
                Align::Center,
                Some(f64::from(width - 112.)),
            );
        }
        lines.len()
    }
    fn line(&mut self, line: &Line, offset: Point) {
        self.canvas.move_to(
            f64::from(line.from.x + offset.x),
            f64::from(line.from.y + offset.y),
        );
        self.canvas.line_to(
            f64::from(line.to.x + offset.x),
            f64::from(line.to.y + offset.y),
        );
        self.canvas.stroke(f64::from(line.thickness));
    }
    fn glyph(&mut self, text: &str, x: f32, y: f32, size: f32, rotated: bool) -> f32 {
        let face = self.face;
        let scale = size / f32::from(face.units_per_em());
        let mut advance = 0.;
        for ch in text.chars() {
            let glyph = face
                .glyph_index(ch)
                .expect("Bravura contains engraved glyph");
            let mut path = Outline {
                canvas: &mut *self.canvas,
                x,
                y,
                scale,
                advance,
                rotated,
            };
            face.outline_glyph(glyph, &mut path);
            advance += f32::from(face.glyph_hor_advance(glyph).unwrap_or_default()) * scale;
        }
        self.canvas.fill();
        advance
    }
    fn page(&mut self, layout: &Layout, first: usize, last: usize, top: f32, height: f32) {
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
                    Serif::Bold,
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
                    Serif::Bold,
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
                    Serif::BoldItalic,
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
                    Serif::Regular,
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
                let thickness = self.metric("bracketThickness") * 0.5;
                self.canvas.fill_rect(
                    bracket.x.into(),
                    bracket.top.into(),
                    thickness.into(),
                    (bracket.bottom - bracket.top).into(),
                );
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
                    for (i, point) in segment.corners.iter().enumerate() {
                        let (x, y) = (f64::from(point.x + offset.x), f64::from(point.y + offset.y));
                        if i == 0 {
                            self.canvas.move_to(x, y);
                        } else {
                            self.canvas.line_to(x, y);
                        }
                    }
                    self.canvas.close();
                    self.canvas.fill();
                }
            }
            for label in measure
                .content
                .techniques
                .iter()
                .chain(&measure.content.expressions)
            {
                let (style, size) = if matches!(label.mark, score_core::MarkRef::Chord(_)) {
                    (Serif::Regular, 18.)
                } else {
                    (Serif::Italic, 16.)
                };
                self.text(
                    label.text.as_str(),
                    style,
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
                    Serif::Regular,
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
                Serif::Regular,
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
                self.canvas
                    .move_to(a.x.into(), (a.y - endpoint / 2.).into());
                self.canvas.curve_to(
                    (a.x + span / 3.).into(),
                    (a.y + curve.bend - control).into(),
                    (b.x - span / 3.).into(),
                    (b.y + curve.bend - control).into(),
                    b.x.into(),
                    (b.y - endpoint / 2.).into(),
                );
                self.canvas
                    .line_to(b.x.into(), (b.y + endpoint / 2.).into());
                self.canvas.curve_to(
                    (b.x - span / 3.).into(),
                    (b.y + curve.bend + control).into(),
                    (a.x + span / 3.).into(),
                    (a.y + curve.bend + control).into(),
                    a.x.into(),
                    (a.y + endpoint / 2.).into(),
                );
                self.canvas.close();
                self.canvas.fill();
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
                                Serif::Bold,
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
                    Serif::Regular,
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
struct Outline<'a, C> {
    canvas: &'a mut C,
    x: f32,
    y: f32,
    scale: f32,
    advance: f32,
    rotated: bool,
}
impl<C> Outline<'_, C> {
    fn point(&self, x: f32, y: f32) -> (f64, f64) {
        let (x, y) = if self.rotated {
            (-y * self.scale, -(x * self.scale + self.advance))
        } else {
            (x * self.scale + self.advance, -y * self.scale)
        };
        ((self.x + x).into(), (self.y + y).into())
    }
}
impl<C: Canvas> OutlineBuilder for Outline<'_, C> {
    fn move_to(&mut self, x: f32, y: f32) {
        let (x, y) = self.point(x, y);
        self.canvas.move_to(x, y);
    }
    fn line_to(&mut self, x: f32, y: f32) {
        let (x, y) = self.point(x, y);
        self.canvas.line_to(x, y);
    }
    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        let (x1, y1) = self.point(x1, y1);
        let (x, y) = self.point(x, y);
        self.canvas.quad_to(x1, y1, x, y);
    }
    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        let (x1, y1) = self.point(x1, y1);
        let (x2, y2) = self.point(x2, y2);
        let (x, y) = self.point(x, y);
        self.canvas.curve_to(x1, y1, x2, y2, x, y);
    }
    fn close(&mut self) {
        self.canvas.close();
    }
}
