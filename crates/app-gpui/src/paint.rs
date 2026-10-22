use gpui::{prelude::*, *};
use score_core::{BarMark, BarMarkRef, ElementId, MarkRef};
use score_edit::Cursor;
use score_layout::{Hit, Layout, Point as ScorePoint};
use std::rc::Rc;
mod range_warning;

pub struct CanvasState {
    pub show_ranges: bool,
    pub score: score_core::Score,
    pub selection: Option<ElementId>,
    pub selection_range: Vec<ElementId>,
    pub cursor: Cursor,
    pub note_entry: bool,
    pub playing: bool,
    pub notation_point: score_core::DivisiPoint,
    pub playheads: Vec<ElementId>,
    pub selected_pitch: Option<score_core::Pitch>,
    /// A mark picked in the score is highlighted on its own, not its note.
    pub selected_mark: Option<MarkRef>,
    pub selected_bar_mark: Option<BarMarkRef>,
    pub hover: Option<crate::selection::Hover>,
    pub position: crate::playhead::Position,
    pub(super) time_target: Option<crate::note_drag::TimeTarget>,
    pub(super) ghost: Option<crate::ghost::Ghost>,
}

/// Returns the displayed box under the pointer, even when a divisi mark has
/// more than one box with the same canonical identity.
pub fn hover_bounds(layout: &Layout, hit: Hit, pointer: ScorePoint) -> Option<Bounds<Pixels>> {
    let p = ScorePoint {
        x: pointer.x / layout.scale,
        y: pointer.y / layout.scale,
    };
    match hit {
        Hit::Note(id, Some(pitch)) => layout
            .measures
            .iter()
            .flat_map(|measure| {
                measure
                    .content
                    .events
                    .iter()
                    .filter(move |event| event.id == id && event.pitch == Some(pitch))
                    .map(move |event| {
                        let x = measure.origin.x + event.center.x;
                        let y = measure.origin.y + event.center.y;
                        ((p.x - x).abs().max((p.y - y).abs()), x, y)
                    })
            })
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(_, x, y)| {
                Bounds::new(
                    point(px((x - 8.) * layout.scale), px((y - 6.) * layout.scale)),
                    size(px(16. * layout.scale), px(12. * layout.scale)),
                )
            }),
        Hit::Mark(mark) if !matches!(mark, MarkRef::Connection(_)) => layout
            .measures
            .iter()
            .flat_map(|measure| {
                measure
                    .content
                    .marks
                    .iter()
                    .filter(move |box_| box_.mark == mark)
                    .map(move |box_| {
                        let x = measure.origin.x + box_.center.x;
                        let y = measure.origin.y + box_.center.y;
                        ((p.x - x).abs().max((p.y - y).abs()), x, y, box_.half)
                    })
            })
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(_, x, y, half)| {
                Bounds::new(
                    point(
                        px((x - half.0 - 2.) * layout.scale),
                        px((y - half.1 - 2.) * layout.scale),
                    ),
                    size(
                        px((half.0 + 2.) * 2. * layout.scale),
                        px((half.1 + 2.) * 2. * layout.scale),
                    ),
                )
            }),
        _ => None,
    }
}

pub fn score_canvas(
    layout: Rc<Layout>,
    state: CanvasState,
    view: WeakEntity<crate::AppView>,
    music_font: Rc<crate::music_font::MusicFont>,
) -> impl IntoElement {
    let CanvasState {
        show_ranges,
        score,
        selection,
        selection_range,
        cursor,
        note_entry,
        playing,
        notation_point,
        playheads,
        selected_pitch,
        selected_mark,
        selected_bar_mark,
        hover,
        position,
        time_target,
        ghost,
    } = state;
    let active_staff = layout
        .voice_measure_at_tick(
            cursor.part,
            cursor.staff,
            notation_point.measure,
            cursor.voice,
            f64::from(notation_point.tick),
        )
        .map(|m| m.staff)
        .or_else(|| {
            selection
                .and_then(|id| layout.event_measure(id))
                .map(|m| m.staff)
        })
        .or_else(|| {
            layout
                .voice_cursor(
                    cursor.part,
                    cursor.staff,
                    cursor.measure,
                    cursor.voice,
                    cursor.index,
                )
                .and_then(|p| {
                    layout.measures.iter().find(|m| {
                        m.index == cursor.measure && (m.origin.y * layout.scale - p.y).abs() < 0.01
                    })
                })
                .map(|m| m.staff)
        })
        .unwrap_or(cursor.staff);
    let (selection, selection_range) = if selected_mark.is_some() || selected_bar_mark.is_some() {
        (None, Vec::new())
    } else {
        (selection, selection_range)
    };
    let connection_selected =
        move |connection| selected_mark == Some(MarkRef::Connection(connection));
    let passage = crate::selection::passage_bounds(&layout, &selection_range);
    let selected: std::collections::HashSet<_> = selection_range.into_iter().collect();
    let scale = layout.scale;
    let width = layout.width * scale;
    let height = layout.height * scale;
    canvas(
        move |bounds, _, cx| {
            // Keep the true, scrolled canvas origin for mouse hit testing.
            let _ = view.update(cx, |view, cx| {
                view.canvas_origin = bounds.origin;
                view.viewport.record_canvas(bounds.origin);
                if let Some(page) = view.pending_page.take() {
                    view.reveal_selection = false;
                    view.viewport.reveal_page(&view.layout, page);
                    cx.notify();
                } else if let Some((bar, mark)) = view.review.reveal_bar.take() {
                    view.reveal_selection = false;
                    view.viewport.reveal_review_bar(&view.layout, bar, mark);
                    cx.notify();
                } else if view.reveal_selection {
                    view.reveal_selection = false;
                    if view.playback.is_playing() {
                        let cursor = view.document.editor.cursor();
                        view.viewport.follow(
                            &view.layout,
                            view.playback.playhead,
                            crate::playhead::tracking_bounds(&view.layout, cursor, view.position),
                            true,
                        );
                    } else {
                        view.viewport.reveal_selection(
                            &view.layout,
                            view.document.editor.selection(),
                            view.document.editor.cursor(),
                        );
                    }
                    cx.notify();
                }
            });
        },
        move |bounds, _, window, cx| {
            let origin = bounds.origin;
            // A large orchestra has thousands of marks; shape only what can be seen.
            let clip = window.content_mask().bounds;
            let shown = |ink: Ink| visible(&clip, origin, scale, ink);
            for (index, page) in layout.pages.iter().enumerate() {
                let paper = Bounds::new(
                    origin + point(px(0.), px(page.top * scale)),
                    size(px(page.width * scale), px(page.height * scale)),
                );
                window.paint_quad(fill(
                    Bounds::new(paper.origin + point(px(3.), px(3.)), paper.size),
                    rgba(0x00000044),
                ));
                window.paint_quad(quad(
                    paper,
                    px(0.),
                    rgb(0xfbf9f3),
                    px(1.),
                    rgb(0xe1ded6),
                    Default::default(),
                ));
                if layout.mode == score_layout::ViewMode::Page {
                    text_at(
                        &format!("{} / {}", index + 1, layout.pages.len()),
                        font(".SystemUIFont"),
                        (12. * scale).max(10.),
                        origin
                            + point(
                                px((page.width / 2. - 12.) * scale),
                                px((page.top + page.height - 24.) * scale),
                            ),
                        // 5.7:1 on the paper; with the 10 px floor it stays legible zoomed out.
                        0x67635c,
                        window,
                        cx,
                    );
                }
            }
            for mark in &layout.tempos {
                let color = if selected_bar_mark
                    == Some(BarMarkRef {
                        measure: mark.measure,
                        mark: BarMark::Tempo,
                    }) {
                    crate::playhead::SELECTED
                } else {
                    0x202c27
                };
                let mut at =
                    origin + point(px(mark.position.x * scale), px(mark.position.y * scale));
                if !mark.mark.text.is_empty() {
                    for run in crate::music_font::text_runs(&mark.mark.text) {
                        match run {
                            crate::music_font::TextRun::Text(text) => {
                                let text: SharedString = text.into();
                                let line = window.text_system().shape_line(
                                    text.clone(),
                                    px(18. * scale),
                                    &[TextRun {
                                        len: text.len(),
                                        font: font("Times New Roman").bold(),
                                        color: rgb(color).into(),
                                        background_color: None,
                                        underline: None,
                                        strikethrough: None,
                                    }],
                                    None,
                                );
                                let _ = line.paint(
                                    at - point(px(0.), line.ascent),
                                    line.ascent + line.descent,
                                    window,
                                    cx,
                                );
                                at.x += line.width;
                            }
                            crate::music_font::TextRun::Music(text) => {
                                let unit = music_font.bounds(&text, 1.);
                                let font_size = 18. * scale / f32::from(unit.size.height);
                                music_font.paint_sized(&text, at, color, font_size, window);
                                at.x += music_font.bounds(&text, font_size).size.width + px(scale);
                            }
                        }
                    }
                    at.x += px(12. * scale);
                }
                let glyph = mark.mark.beat.glyph();
                music_font.paint_sized(glyph, at, color, 28. * scale, window);
                at.x += music_font.bounds(glyph, 28. * scale).size.width + px(8. * scale);
                text_at(
                    &format!("= {}", mark.mark.bpm),
                    font("Times New Roman"),
                    18. * scale,
                    at,
                    color,
                    window,
                    cx,
                );
            }
            for band in &passage {
                window.paint_quad(quad(
                    Bounds::new(origin + band.origin, band.size),
                    px(3.),
                    rgb(0xe3ebff),
                    px(1.),
                    rgb(0x79a4ff),
                    Default::default(),
                ));
            }
            if let Some(hover) = hover.filter(|hover| match hover.hit {
                Hit::Note(id, _) => !selected.contains(&id),
                Hit::Mark(mark) => selected_mark != Some(mark),
            }) && let Some(bounds) = hover.bounds
            {
                window.paint_quad(quad(
                    Bounds::new(origin + bounds.origin, bounds.size),
                    px(3.),
                    rgba(0x3c70cf22),
                    px(0.),
                    transparent_black(),
                    Default::default(),
                ));
            }
            let (show_caret, show_marker) = crate::playhead::indicators(note_entry, playing);
            if show_marker
                && let Some(marker) = crate::playhead::score_bounds(&layout, cursor, position)
            {
                crate::playhead::paint_marker(
                    origin.x + marker.left(),
                    origin.y + marker.top(),
                    origin.y + marker.bottom(),
                    window,
                );
            }
            if show_caret && let Some(caret) = crate::playhead::caret_bounds(&layout, cursor) {
                window.paint_quad(fill(
                    Bounds::new(origin + caret.origin, caret.size),
                    rgb(crate::playhead::voice_color(cursor.voice)),
                ));
            }
            for bracket in layout.brackets.iter().filter(|b| {
                shown((
                    b.x,
                    b.top - INK_MARGIN,
                    b.x + INK_MARGIN,
                    b.bottom + INK_MARGIN,
                ))
            }) {
                let x = origin.x + px(bracket.x * scale);
                let top = origin.y + px(bracket.top * scale);
                let bottom = origin.y + px(bracket.bottom * scale);
                // SMuFL extensible brackets: original font terminals plus a stem.
                music_font.paint_sized("\u{e003}", point(x, top), 0x202c27, 20. * scale, window);
                music_font.paint_sized("\u{e004}", point(x, bottom), 0x202c27, 20. * scale, window);
                window.paint_quad(fill(
                    Bounds::new(
                        point(x, top),
                        size(px(music_font.engraving.bracket * 0.5 * scale), bottom - top),
                    ),
                    rgb(0x202c27),
                ));
            }
            for brace in layout
                .braces
                .iter()
                .filter(|b| shown((b.x - INK_MARGIN, b.top, b.x + 10., b.bottom)))
            {
                let height = (brace.bottom - brace.top) * scale;
                // Bravura's narrower alternates keep a tall piano brace proportionate.
                let glyph = if brace.bottom - brace.top > 200. {
                    "\u{f402}"
                } else {
                    "\u{f401}"
                };
                let font_size = height / f32::from(music_font.bounds(glyph, 1.).size.height);
                let ink = music_font.bounds(glyph, font_size);
                music_font.paint_sized(
                    glyph,
                    point(
                        origin.x + px((brace.x + 10.) * scale) - ink.size.width - ink.origin.x,
                        origin.y + px(brace.top * scale) - ink.origin.y,
                    ),
                    0x303735,
                    font_size,
                    window,
                );
            }
            for line in layout.connectors.iter().filter(|l| shown(line_ink(l))) {
                let mut path = PathBuilder::stroke(px(line.thickness * scale));
                path.move_to(origin + point(px(line.from.x * scale), px(line.from.y * scale)));
                path.line_to(origin + point(px(line.to.x * scale), px(line.to.y * scale)));
                if let Ok(path) = path.build() {
                    window.paint_path(path, rgb(0x303735));
                }
            }
            for ending in &layout.rehearsals {
                let color = if selected_bar_mark
                    == Some(BarMarkRef {
                        measure: ending.measure,
                        mark: BarMark::Rehearsal,
                    }) {
                    crate::playhead::SELECTED
                } else {
                    0x303735
                };
                for line in ending.frame.iter().filter(|line| shown(line_ink(line))) {
                    let mut path = PathBuilder::stroke(px(line.thickness * scale));
                    path.move_to(origin + point(px(line.from.x * scale), px(line.from.y * scale)));
                    path.line_to(origin + point(px(line.to.x * scale), px(line.to.y * scale)));
                    if let Ok(path) = path.build() {
                        window.paint_path(path, rgb(color));
                    }
                }
                if !ending.text.is_empty()
                    && shown((
                        ending.position.x,
                        ending.position.y - 18.,
                        ending.frame[0].to.x,
                        ending.position.y + 5.,
                    ))
                {
                    mixed_text_at(
                        &music_font,
                        &ending.text,
                        font("Times New Roman").bold(),
                        18. * scale,
                        origin
                            + point(px(ending.position.x * scale), px(ending.position.y * scale)),
                        color,
                        window,
                        cx,
                    );
                }
            }
            for ending in &layout.endings {
                let color = if selected_bar_mark
                    == Some(BarMarkRef {
                        measure: ending.ending_start,
                        mark: BarMark::Ending,
                    }) {
                    crate::playhead::SELECTED
                } else {
                    0x303735
                };
                for line in ending.lines.iter().filter(|line| shown(line_ink(line))) {
                    let mut path = PathBuilder::stroke(px(line.thickness * scale));
                    path.move_to(origin + point(px(line.from.x * scale), px(line.from.y * scale)));
                    path.line_to(origin + point(px(line.to.x * scale), px(line.to.y * scale)));
                    if let Ok(path) = path.build() {
                        window.paint_path(path, rgb(color));
                    }
                }
                if !ending.label.is_empty()
                    && shown((
                        ending.label_position.x,
                        ending.label_position.y - 14.,
                        ending.label_position.x + 200.,
                        ending.label_position.y + 4.,
                    ))
                {
                    mixed_text_at(
                        &music_font,
                        &ending.label,
                        font("Times New Roman").bold(),
                        14. * scale,
                        origin
                            + point(
                                px(ending.label_position.x * scale),
                                px(ending.label_position.y * scale),
                            ),
                        color,
                        window,
                        cx,
                    );
                }
            }
            for label in &layout.navigation {
                let color = if selected_bar_mark
                    == Some(BarMarkRef {
                        measure: label.measure,
                        mark: label.mark,
                    }) {
                    crate::playhead::SELECTED
                } else {
                    0x303735
                };
                let font = font("Times New Roman").bold().italic();
                let text_width =
                    mixed_text_width(&music_font, &label.text, &font, label.size * scale, window);
                let x = label.position.x
                    - if label.right_aligned {
                        f32::from(text_width) / scale
                    } else {
                        0.
                    };
                if shown((
                    x,
                    label.position.y - label.size,
                    x + f32::from(text_width) / scale,
                    label.position.y + 4.,
                )) {
                    mixed_text_at(
                        &music_font,
                        &label.text,
                        font,
                        label.size * scale,
                        origin + point(px(x * scale), px(label.position.y * scale)),
                        color,
                        window,
                        cx,
                    );
                }
            }
            for label in layout.labels.iter().filter(|l| {
                shown((
                    l.position.x,
                    l.position.y - 2. * score_layout::STAFF_LABEL_SIZE,
                    l.position.x + LABEL_REACH,
                    l.position.y + score_layout::STAFF_LABEL_SIZE,
                ))
            }) {
                mixed_text_at(
                    &music_font,
                    &label.text,
                    font("Times New Roman"),
                    score_layout::STAFF_LABEL_SIZE * scale,
                    origin + point(px(label.position.x * scale), px(label.position.y * scale)),
                    if label.part == cursor.part
                        && label.staff.is_none_or(|staff| staff == active_staff)
                    {
                        0x3c70cf
                    } else {
                        0x46433f
                    },
                    window,
                    cx,
                );
            }
            for measure in &layout.measures {
                let offset =
                    origin + point(px(measure.origin.x * scale), px(measure.origin.y * scale));
                let ink = measure_ink_bounds(measure, offset, scale);
                if !ink.intersects(&window.content_mask().bounds) {
                    continue;
                }
                for line in &measure.content.lines {
                    let mut path = PathBuilder::stroke(px(line.thickness * scale));
                    path.move_to(offset + point(px(line.from.x * scale), px(line.from.y * scale)));
                    path.line_to(offset + point(px(line.to.x * scale), px(line.to.y * scale)));
                    if let Ok(path) = path.build() {
                        let color = line.element.map_or(0x303735, |id| {
                            crate::playhead::note_color(Some(id), &selected, &playheads)
                        });
                        window.paint_path(path, rgb(color));
                    }
                }
                for group in &measure.content.beams {
                    let color = if group.elements.iter().all(|id| playheads.contains(id)) {
                        0xb56720
                    } else if group.elements.iter().all(|id| selected.contains(id)) {
                        0x3c70cf
                    } else {
                        0x202c27
                    };
                    for segment in &group.segments {
                        let mut path = PathBuilder::fill();
                        for (index, p) in segment.corners.iter().enumerate() {
                            let p = offset + point(px(p.x * scale), px(p.y * scale));
                            if index == 0 {
                                path.move_to(p);
                            } else {
                                path.line_to(p);
                            }
                        }
                        path.close();
                        if let Ok(path) = path.build() {
                            window.paint_path(path, rgb(color));
                        }
                    }
                }
                for label in &measure.content.techniques {
                    text_at(
                        label.text,
                        font("Times New Roman").italic(),
                        16. * scale,
                        offset + point(px(label.position.x * scale), px(label.position.y * scale)),
                        if selected.contains(&label.element)
                            || selected_mark == Some(MarkRef::Technique(label.element))
                            || selected_mark == Some(MarkRef::Division(label.element))
                        {
                            0x3c70cf
                        } else {
                            0x202c27
                        },
                        window,
                        cx,
                    );
                }
                for (index, glyph) in measure.content.glyphs.iter().enumerate() {
                    let mut color =
                        crate::playhead::note_color(glyph.element, &selected, &playheads);
                    if measure
                        .content
                        .marks
                        .iter()
                        .any(|mark| mark.glyph == Some(index) && Some(mark.mark) == selected_mark)
                    {
                        color = crate::playhead::SELECTED;
                    }
                    // Distinguish the chosen chord tone through the notehead ink,
                    // while keeping staff lines visible through hollow heads.
                    if color == crate::playhead::SELECTED
                        && glyph.element == selection
                        && glyph
                            .text
                            .chars()
                            .next()
                            .is_some_and(|c| matches!(c, '\u{e0a2}'..='\u{e0a4}'))
                        && measure.content.events.iter().any(|head| {
                            Some(head.id) == selection
                                && head.pitch == selected_pitch
                                && (head.center.y - glyph.position.y).abs() < 0.01
                        })
                    {
                        color = 0x194ba2;
                    }
                    if show_ranges
                        && let Some(warning) = range_warning::color(
                            &score,
                            &measure.content,
                            glyph,
                            selection.zip(selected_pitch),
                        )
                    {
                        color = warning;
                    }
                    music_font.paint_oriented(
                        &glyph.text,
                        offset + point(px(glyph.position.x * scale), px(glyph.position.y * scale)),
                        rgb(color).into(),
                        score_layout::MUSIC_SIZE * scale,
                        glyph.rotated,
                        window,
                    );
                }
                text_at(
                    &(measure.index + 1).to_string(),
                    font(".SystemUIFont"),
                    11. * scale,
                    offset + point(px(0.), px(-18. * scale)),
                    0x89938d,
                    window,
                    cx,
                );
            }
            for mark in layout.piano_arpeggios.iter().filter(|m| {
                let (center, half) = (m.bounds.center, m.bounds.half);
                shown((
                    center.x - half.0,
                    center.y - half.1,
                    center.x + half.0,
                    center.y + half.1,
                ))
            }) {
                let anchor = mark.bounds.mark.anchor();
                let color = if selected_mark == Some(mark.bounds.mark)
                    || selected.contains(&anchor)
                    || selected.contains(&mark.other_hand)
                {
                    crate::playhead::SELECTED
                } else if playheads.contains(&anchor) || playheads.contains(&mark.other_hand) {
                    0xb56720
                } else {
                    0x202c27
                };
                music_font.paint_oriented(
                    &mark.glyph.text,
                    origin
                        + point(
                            px(mark.glyph.position.x * scale),
                            px(mark.glyph.position.y * scale),
                        ),
                    rgb(color).into(),
                    score_layout::MUSIC_SIZE * scale,
                    true,
                    window,
                );
            }
            for hairpin in layout.hairpins.iter().filter(|h| {
                let (a, b) = (line_ink(&h.upper), line_ink(&h.lower));
                shown((a.0.min(b.0), a.1.min(b.1), a.2.max(b.2), a.3.max(b.3)))
            }) {
                let color = if selected.contains(&hairpin.connection.start)
                    || selected.contains(&hairpin.connection.end)
                    || connection_selected(hairpin.connection)
                {
                    0x3c70cf
                } else {
                    0x202c27
                };
                for line in [&hairpin.upper, &hairpin.lower] {
                    let mut path = PathBuilder::stroke(px(line.thickness * scale));
                    path.move_to(origin + point(px(line.from.x * scale), px(line.from.y * scale)));
                    path.line_to(origin + point(px(line.to.x * scale), px(line.to.y * scale)));
                    if let Ok(path) = path.build() {
                        window.paint_path(path, rgb(color));
                    }
                }
            }
            for curve in layout.curves.iter().filter(|c| shown(curve_ink(c))) {
                let p = |x, y| origin + point(px(x * scale), px(y * scale));
                let (endpoint, middle) = if curve.connection.kind == score_core::ConnectionKind::Tie
                {
                    (
                        music_font.engraving.tie_endpoint,
                        music_font.engraving.tie_middle,
                    )
                } else {
                    (
                        music_font.engraving.slur_endpoint,
                        music_font.engraving.slur_middle,
                    )
                };
                // A variable-length curve uses Bravura's endpoint/midpoint thicknesses.
                // Cubic interpolation gives 1/4 of its weight to endpoints at the middle.
                let control = (middle - endpoint * 0.25) / 1.5;
                let mut path = PathBuilder::fill();
                let span = curve.to.x - curve.from.x;
                path.move_to(p(curve.from.x, curve.from.y - endpoint / 2.));
                path.cubic_bezier_to(
                    p(curve.to.x, curve.to.y - endpoint / 2.),
                    p(
                        curve.from.x + span / 3.,
                        curve.from.y + curve.bend - control,
                    ),
                    p(curve.to.x - span / 3., curve.to.y + curve.bend - control),
                );
                path.line_to(p(curve.to.x, curve.to.y + endpoint / 2.));
                path.cubic_bezier_to(
                    p(curve.from.x, curve.from.y + endpoint / 2.),
                    p(curve.to.x - span / 3., curve.to.y + curve.bend + control),
                    p(
                        curve.from.x + span / 3.,
                        curve.from.y + curve.bend + control,
                    ),
                );
                path.close();
                if let Ok(path) = path.build() {
                    window.paint_path(
                        path,
                        rgb(
                            if selected.contains(&curve.connection.start)
                                || selected.contains(&curve.connection.end)
                                || connection_selected(curve.connection)
                            {
                                0x3c70cf
                            } else {
                                0x202c27
                            },
                        ),
                    );
                }
            }
            if let Some(id) = selection
                && let Some(target) = time_target
            {
                paint_time_target(
                    &layout,
                    id,
                    target,
                    cursor.voice,
                    origin,
                    &music_font,
                    window,
                );
            }
            if let Some(ghost) = ghost {
                paint_ghost(ghost, origin, scale, &music_font, window);
            }
        },
    )
    .w(px(width))
    .h(px(height))
}

fn paint_ghost(
    ghost: crate::ghost::Ghost,
    origin: Point<Pixels>,
    scale: f32,
    music_font: &crate::music_font::MusicFont,
    window: &mut Window,
) {
    let color = rgba((crate::playhead::voice_color(ghost.voice) << 8) | 0x88);
    let glyph = match ghost.duration.value() {
        score_core::NoteValue::Whole => "\u{e0a2}",
        score_core::NoteValue::Half => "\u{e0a3}",
        _ => "\u{e0a4}",
    };
    music_font.paint_oriented(
        glyph,
        origin + point(px(ghost.x * scale), px(ghost.y * scale)),
        color.into(),
        score_layout::MUSIC_SIZE * scale,
        false,
        window,
    );
    let y_local = ghost.y - ghost.staff_top;
    let line_right = if ghost.duration.value() == score_core::NoteValue::Whole {
        21.
    } else {
        16.
    };
    let mut paint_ledger = |line_y: f32| {
        let mut path = PathBuilder::stroke(px(1.6 * scale));
        path.move_to(
            origin
                + point(
                    px((ghost.x - 4.) * scale),
                    px((ghost.staff_top + line_y) * scale),
                ),
        );
        path.line_to(
            origin
                + point(
                    px((ghost.x + line_right) * scale),
                    px((ghost.staff_top + line_y) * scale),
                ),
        );
        if let Ok(path) = path.build() {
            window.paint_path(path, color);
        }
    };
    let mut line_y = -10.;
    while line_y >= y_local {
        paint_ledger(line_y);
        line_y -= 10.;
    }
    line_y = 50.;
    while line_y <= y_local {
        paint_ledger(line_y);
        line_y += 10.;
    }
    if ghost.duration.is_dotted() {
        let dot_y = if (y_local / 10.).fract().abs() < 0.001 {
            ghost.y - 5.
        } else {
            ghost.y
        };
        music_font.paint_oriented(
            "\u{e1e7}",
            origin + point(px((ghost.x + 16.) * scale), px(dot_y * scale)),
            color.into(),
            score_layout::MUSIC_SIZE * scale,
            false,
            window,
        );
    }
}

fn paint_time_target(
    layout: &Layout,
    id: ElementId,
    target: crate::note_drag::TimeTarget,
    voice: score_core::Voice,
    origin: Point<Pixels>,
    music_font: &crate::music_font::MusicFont,
    window: &mut Window,
) {
    let Some(source) = layout.event_measure(id) else {
        return;
    };
    let Some(head) = source.content.events.iter().find(|e| e.id == id) else {
        return;
    };
    let Some(destination) = layout.voice_measure_at_tick(
        id.part,
        id.staff,
        target.measure,
        voice,
        f64::from(target.tick),
    ) else {
        return;
    };
    let scale = layout.scale;
    let x = destination.origin.x + destination.content.x_at_tick(f64::from(target.tick));
    window.paint_quad(fill(
        Bounds::new(
            origin + point(px(x * scale), px((destination.origin.y - 24.) * scale)),
            size(px(2.), px(88. * scale)),
        ),
        rgb(0x3c70cf),
    ));
    let dx = x - source.content.x_at_tick(f64::from(head.tick));
    let offset = origin + point(px(dx * scale), px(destination.origin.y * scale));
    for glyph in source
        .content
        .glyphs
        .iter()
        .filter(|g| g.element == Some(id))
    {
        music_font.paint_oriented(
            &glyph.text,
            offset + point(px(glyph.position.x * scale), px(glyph.position.y * scale)),
            rgba(0x3c70cfaa).into(),
            score_layout::MUSIC_SIZE * scale,
            glyph.rotated,
            window,
        );
    }
    for line in source
        .content
        .lines
        .iter()
        .filter(|l| l.element == Some(id))
    {
        let mut path = PathBuilder::stroke(px(line.thickness * scale));
        path.move_to(offset + point(px(line.from.x * scale), px(line.from.y * scale)));
        path.line_to(offset + point(px(line.to.x * scale), px(line.to.y * scale)));
        if let Ok(path) = path.build() {
            window.paint_path(path, rgba(0x3c70cfaa));
        }
    }
    if let Some(stem) = source.content.beam_stem(id) {
        let glyph = match target.duration.value() {
            score_core::NoteValue::Eighth => Some(if stem.down { "\u{e241}" } else { "\u{e240}" }),
            score_core::NoteValue::Sixteenth => {
                Some(if stem.down { "\u{e243}" } else { "\u{e242}" })
            }
            _ => None,
        };
        if let Some(glyph) = glyph {
            music_font.paint_oriented(
                glyph,
                offset + point(px(stem.to.x * scale), px(stem.to.y * scale)),
                rgba(0x3c70cfaa).into(),
                score_layout::MUSIC_SIZE * scale,
                false,
                window,
            );
        }
    }
}

/// Layout-space ink extents: `(left, top, right, bottom)`.
type Ink = (f32, f32, f32, f32);
/// Allowance for stroke width, glyph overhang and curve thickness.
const INK_MARGIN: f32 = 24.;
/// Wider than any staff label; its real width is known only after shaping.
const LABEL_REACH: f32 = 400.;

fn line_ink(line: &score_layout::Line) -> Ink {
    (
        line.from.x.min(line.to.x),
        line.from.y.min(line.to.y),
        line.from.x.max(line.to.x),
        line.from.y.max(line.to.y),
    )
}

fn curve_ink(curve: &score_layout::Curve) -> Ink {
    // The arch bulges by up to `bend` beyond the chord joining its ends.
    (
        curve.from.x.min(curve.to.x),
        curve.from.y.min(curve.to.y) + curve.bend.min(0.),
        curve.from.x.max(curve.to.x),
        curve.from.y.max(curve.to.y) + curve.bend.max(0.),
    )
}

/// Whether ink placed in layout units can reach the visible part of the canvas.
fn visible(clip: &Bounds<Pixels>, origin: Point<Pixels>, scale: f32, ink: Ink) -> bool {
    let (left, top, right, bottom) = ink;
    Bounds::from_corners(
        origin
            + point(
                px((left - INK_MARGIN) * scale),
                px((top - INK_MARGIN) * scale),
            ),
        origin
            + point(
                px((right + INK_MARGIN) * scale),
                px((bottom + INK_MARGIN) * scale),
            ),
    )
    .intersects(clip)
}

pub(super) fn text_at(
    text: &str,
    font: Font,
    font_size: f32,
    baseline: Point<Pixels>,
    color: u32,
    window: &mut Window,
    cx: &mut App,
) {
    let text: SharedString = text.to_owned().into();
    let run = TextRun {
        len: text.len(),
        font,
        color: rgb(color).into(),
        background_color: None,
        underline: None,
        strikethrough: None,
    };
    let line = window
        .text_system()
        .shape_line(text, px(font_size), &[run], None);
    let height = line.ascent + line.descent;
    let _ = line.paint(baseline - point(px(0.), line.ascent), height, window, cx);
}

fn mixed_text_width(
    music_font: &crate::music_font::MusicFont,
    text: &str,
    font: &Font,
    font_size: f32,
    window: &mut Window,
) -> Pixels {
    crate::music_font::text_runs(text)
        .into_iter()
        .map(|run| match run {
            crate::music_font::TextRun::Text(text) => {
                let text: SharedString = text.into();
                window
                    .text_system()
                    .shape_line(
                        text.clone(),
                        px(font_size),
                        &[TextRun {
                            len: text.len(),
                            font: font.clone(),
                            color: rgb(0x303735).into(),
                            background_color: None,
                            underline: None,
                            strikethrough: None,
                        }],
                        None,
                    )
                    .width
            }
            crate::music_font::TextRun::Music(text) => {
                let unit = music_font.bounds(&text, 1.);
                let size = font_size * 0.85 / f32::from(unit.size.height);
                music_font.bounds(&text, size).size.width + px(font_size * 0.08)
            }
        })
        .fold(px(0.), |width, run| width + run)
}

#[allow(clippy::too_many_arguments)]
fn mixed_text_at(
    music_font: &crate::music_font::MusicFont,
    text: &str,
    font: Font,
    font_size: f32,
    baseline: Point<Pixels>,
    color: u32,
    window: &mut Window,
    cx: &mut App,
) {
    let mut at = baseline;
    for run in crate::music_font::text_runs(text) {
        match run {
            crate::music_font::TextRun::Text(text) => {
                let text: SharedString = text.into();
                let line = window.text_system().shape_line(
                    text.clone(),
                    px(font_size),
                    &[TextRun {
                        len: text.len(),
                        font: font.clone(),
                        color: rgb(color).into(),
                        background_color: None,
                        underline: None,
                        strikethrough: None,
                    }],
                    None,
                );
                let _ = line.paint(
                    at - point(px(0.), line.ascent),
                    line.ascent + line.descent,
                    window,
                    cx,
                );
                at.x += line.width;
            }
            crate::music_font::TextRun::Music(text) => {
                let unit = music_font.bounds(&text, 1.);
                let size = font_size * 0.85 / f32::from(unit.size.height);
                let ink = music_font.bounds(&text, size);
                music_font.paint_sized(
                    &text,
                    at - point(
                        ink.origin.x,
                        ink.origin.y + ink.size.height - px(font_size * 0.08),
                    ),
                    color,
                    size,
                    window,
                );
                at.x += ink.size.width + px(font_size * 0.08);
            }
        }
    }
}

/// Include written marks that extend past the automatically spaced measure.
fn measure_ink_bounds(
    measure: &score_layout::PlacedMeasure,
    offset: Point<Pixels>,
    scale: f32,
) -> Bounds<Pixels> {
    let left = measure.content.marks.iter().fold(-20.0_f32, |left, box_| {
        left.min(box_.center.x - box_.half.0 - 20.)
    });
    let right = measure
        .content
        .marks
        .iter()
        .fold(measure.width + 20., |right, box_| {
            right.max(box_.center.x + box_.half.0 + 20.)
        });
    let top = measure.content.ink_top.min(-26.);
    Bounds::new(
        offset + point(px(left * scale), px(top * scale)),
        size(
            px((right - left) * scale),
            px((measure.content.ink_bottom - top + 10.) * scale),
        ),
    )
}

#[cfg(test)]
mod tests {
    use super::{INK_MARGIN, curve_ink, measure_ink_bounds, visible};
    use gpui::{Bounds, point, px};
    use score_core::{
        Connection, ConnectionKind, Duration, Dynamic, ElementId, Event, EventId, MarkRef,
        Notation, Offset, PartId, Pitch, Pitches, Score, StaffId, Step,
    };

    #[test]
    fn a_mark_outside_its_measure_is_visible_at_multiple_scales() {
        let mut score = Score::default();
        let id = score
            .insert(
                0,
                0,
                Event {
                    pitches: Pitches::single(Pitch::new(Step::C, 4)),
                    duration: Duration::Whole,
                    notation: Notation::default(),
                },
            )
            .unwrap();
        score.set_dynamic(id, Some(Dynamic::F)).unwrap();
        score
            .set_mark_offset(MarkRef::Dynamic(id), Offset { dx: 400, dy: 0 })
            .unwrap();
        let layout = score_layout::LayoutEngine::default().layout_compact(&score, 600.);
        let measure = layout
            .measures
            .iter()
            .find(|placed| {
                placed
                    .content
                    .marks
                    .iter()
                    .any(|box_| box_.mark == MarkRef::Dynamic(id))
            })
            .unwrap();
        let mark = measure
            .content
            .marks
            .iter()
            .find(|box_| box_.mark == MarkRef::Dynamic(id))
            .unwrap();
        assert!(mark.center.x - mark.half.0 > measure.width + 20.);
        for scale in [0.5, 1., 2.] {
            let shift_x = 10. - (measure.origin.x + mark.center.x) * scale;
            let offset = point(px(shift_x + measure.origin.x * scale), px(100.));
            let clip = Bounds::from_corners(point(px(0.), px(0.)), point(px(20.), px(1000.)));
            assert!((shift_x + (measure.origin.x + measure.width + 20.) * scale) < 0.);
            assert!(measure_ink_bounds(measure, offset, scale).intersects(&clip));
        }
    }

    fn curve(y: f32, bend: f32) -> score_layout::Curve {
        let id = ElementId {
            part: PartId(0),
            staff: StaffId::Upper,
            measure: 0,
            event: EventId::default(),
        };
        score_layout::Curve {
            staff_origin: y,
            from: score_layout::Point { x: 100., y },
            to: score_layout::Point { x: 300., y },
            bend,
            connection: Connection {
                start: id,
                end: id,
                kind: ConnectionKind::Slur,
            },
        }
    }

    #[test]
    fn a_slur_arch_entering_the_view_is_painted_even_when_its_ends_are_not() {
        let clip = Bounds::from_corners(point(px(0.), px(0.)), point(px(800.), px(130.)));
        let origin = point(px(0.), px(0.));
        // Ends at y=200 lie below the view; only the arch rises into it.
        assert!(visible(&clip, origin, 1., curve_ink(&curve(200., -60.))));
        assert!(!visible(&clip, origin, 1., curve_ink(&curve(200., 60.))));
        // Scrolling the canvas carries its ink along.
        let scrolled = point(px(0.), px(-1000.));
        assert!(visible(&clip, scrolled, 1., curve_ink(&curve(1100., 0.))));
        assert!(!visible(&clip, scrolled, 1., curve_ink(&curve(200., -60.))));
    }

    #[test]
    fn engraved_slurs_and_ties_stay_inside_their_culling_box() {
        // The painted arch lies within the hull of its ends and Bézier controls,
        // which sit `bend` (plus a sub-unit thickness) away from the chord.
        const THICKNESS: f32 = INK_MARGIN / 2.;
        let mut checked = 0;
        for bytes in [
            include_bytes!("../../../examples/menuetto-y-trio-orquesta.partitura").as_slice(),
            include_bytes!("../../../examples/reguladores.partitura").as_slice(),
            include_bytes!("../../../examples/voces-y-divisi.partitura").as_slice(),
            include_bytes!("../../../examples/piano-expresion.partitura").as_slice(),
        ] {
            let score = score_io::decode(bytes).unwrap();
            for mode in [
                score_layout::ViewMode::Page,
                score_layout::ViewMode::Continuous,
                score_layout::ViewMode::SingleSystem,
            ] {
                let layout = score_layout::LayoutEngine::default().layout_view(&score, 1000., mode);
                checked += layout.curves.len();
                for c in &layout.curves {
                    let (left, top, right, bottom) = curve_ink(c);
                    for (x, y) in [
                        (c.from.x, c.from.y),
                        (c.to.x, c.to.y),
                        (c.from.x, c.from.y + c.bend),
                        (c.to.x, c.to.y + c.bend),
                    ] {
                        assert!(
                            x >= left - THICKNESS
                                && x <= right + THICKNESS
                                && y >= top - THICKNESS
                                && y <= bottom + THICKNESS,
                            "{c:?} escapes its culling box"
                        );
                    }
                }
            }
        }
        assert!(checked > 1000);
    }
}
