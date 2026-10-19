use gpui::{prelude::*, *};
use score_core::{ElementId, MarkRef};
use score_edit::Cursor;
use score_layout::Layout;
use std::rc::Rc;

pub struct CanvasState {
    pub selection: Option<ElementId>,
    pub selection_range: Vec<ElementId>,
    pub cursor: Cursor,
    pub playheads: Vec<ElementId>,
    pub selected_pitch: Option<score_core::Pitch>,
    /// A mark picked in the score is highlighted on its own, not its note.
    pub selected_mark: Option<MarkRef>,
    pub position: crate::playhead::Position,
    pub capacity: u16,
}
pub fn score_canvas(
    layout: Layout,
    state: CanvasState,
    view: WeakEntity<crate::AppView>,
    music_font: Rc<crate::music_font::MusicFont>,
) -> impl IntoElement {
    let CanvasState {
        selection,
        selection_range,
        cursor,
        playheads,
        selected_pitch,
        selected_mark,
        position,
        capacity,
    } = state;
    let (selection, selection_range) = if selected_mark.is_some() {
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
                if view.reveal_selection {
                    view.reveal_selection = false;
                    view.viewport.reveal_selection(
                        &view.layout,
                        view.document.editor.selection(),
                        view.document.editor.cursor(),
                    );
                    cx.notify();
                }
            });
        },
        move |bounds, _, window, cx| {
            let origin = bounds.origin;
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
            if let Some(marker) = crate::playhead::score_bounds(&layout, cursor, position, capacity)
            {
                crate::playhead::paint_marker(
                    origin.x + marker.left(),
                    origin.y + marker.top(),
                    origin.y + marker.bottom(),
                    window,
                );
            }
            for bracket in &layout.brackets {
                let p = |x, y| origin + point(px(x * scale), px(y * scale));
                let x = bracket.x;
                let top = bracket.top;
                let bottom = bracket.bottom;
                // Filled, tapered hooks and a broad stem, like a printed score.
                let mut path = PathBuilder::fill();
                path.move_to(p(x + 9., top - 7.));
                path.cubic_bezier_to(p(x, top + 2.), p(x + 7., top - 1.), p(x + 1., top - 1.));
                path.line_to(p(x, bottom - 2.));
                path.cubic_bezier_to(
                    p(x + 9., bottom + 7.),
                    p(x + 1., bottom + 1.),
                    p(x + 7., bottom + 1.),
                );
                path.line_to(p(x + 9., bottom + 3.));
                path.cubic_bezier_to(p(x + 4., bottom - 2.), p(x + 7., bottom), p(x + 4., bottom));
                path.line_to(p(x + 4., top + 2.));
                path.cubic_bezier_to(p(x + 9., top - 3.), p(x + 4., top), p(x + 7., top));
                path.close();
                if let Ok(path) = path.build() {
                    window.paint_path(path, rgb(0x202c27));
                }
            }
            for brace in &layout.braces {
                let x = brace.x;
                let top = brace.top;
                let bottom = brace.bottom;
                let mid = (top + bottom) / 2.;
                let d = (bottom - top) / 4.;
                let p = |x, y| origin + point(px(x * scale), px(y * scale));
                let mut path = PathBuilder::stroke(px(2. * scale));
                path.move_to(p(x + 10., top));
                path.cubic_bezier_to(
                    p(x + 5., mid - d),
                    p(x - 2., top + d * 0.2),
                    p(x + 5., top + d * 0.5),
                );
                path.cubic_bezier_to(
                    p(x, mid),
                    p(x + 5., mid - d * 0.4),
                    p(x + 5., mid - d * 0.15),
                );
                path.cubic_bezier_to(
                    p(x + 5., mid + d),
                    p(x + 5., mid + d * 0.15),
                    p(x + 5., mid + d * 0.4),
                );
                path.cubic_bezier_to(
                    p(x + 10., bottom),
                    p(x + 5., bottom - d * 0.5),
                    p(x - 2., bottom - d * 0.2),
                );
                if let Ok(path) = path.build() {
                    window.paint_path(path, rgb(0x303735));
                }
            }
            for line in &layout.connectors {
                let mut path = PathBuilder::stroke(px(line.thickness * scale));
                path.move_to(origin + point(px(line.from.x * scale), px(line.from.y * scale)));
                path.line_to(origin + point(px(line.to.x * scale), px(line.to.y * scale)));
                if let Ok(path) = path.build() {
                    window.paint_path(path, rgb(0x303735));
                }
            }
            for label in &layout.labels {
                text_at(
                    &label.text,
                    font("Times New Roman"),
                    12. * scale,
                    origin + point(px(label.position.x * scale), px(label.position.y * scale)),
                    if label.part == cursor.part
                        && label.staff.is_none_or(|staff| staff == cursor.staff)
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
                    music_font.paint_sized(
                        &glyph.text,
                        offset + point(px(glyph.position.x * scale), px(glyph.position.y * scale)),
                        color,
                        score_layout::MUSIC_SIZE * scale,
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
            for hairpin in &layout.hairpins {
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
            for curve in &layout.curves {
                let p = |x, y| origin + point(px(x * scale), px(y * scale));
                let mut path = PathBuilder::stroke(px(1.5 * scale));
                path.move_to(p(curve.from.x, curve.from.y));
                let span = curve.to.x - curve.from.x;
                path.cubic_bezier_to(
                    p(curve.to.x, curve.to.y),
                    p(curve.from.x + span / 3., curve.from.y + curve.bend),
                    p(curve.to.x - span / 3., curve.to.y + curve.bend),
                );
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
        },
    )
    .w(px(width))
    .h(px(height))
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
