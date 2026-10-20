//! Mixed labels keep their normal text font while notation always uses Bravura.
use crate::music_font::{MusicFont, TextRun, text_runs};
use gpui::{prelude::*, *};
use std::rc::Rc;

pub(super) fn inline_text(font: Rc<MusicFont>, text: impl AsRef<str>) -> Div {
    div()
        .flex()
        .min_w_0()
        .items_center()
        .children(text_runs(text.as_ref()).into_iter().map(|run| {
            match run {
                TextRun::Text(text) => div()
                    .flex_shrink_0()
                    .whitespace_nowrap()
                    .child(text)
                    .into_any_element(),
                TextRun::Music(text) => InlineSymbol {
                    font: font.clone(),
                    text,
                }
                .into_any_element(),
            }
        }))
}

struct InlineSymbol {
    font: Rc<MusicFont>,
    text: String,
}

struct SymbolLayout {
    font_size: f32,
    ink: Bounds<Pixels>,
    color: Hsla,
}

impl IntoElement for InlineSymbol {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}

impl Element for InlineSymbol {
    type RequestLayoutState = SymbolLayout;
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        None
    }
    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, SymbolLayout) {
        let style = window.text_style();
        let text_size = style.font_size.to_pixels(window.rem_size());
        let line_height = style
            .line_height
            .to_pixels(text_size.into(), window.rem_size());
        let unit = self.font.bounds(&self.text, 1.);
        let font_size = f32::from(text_size) * 1.05 / f32::from(unit.size.height);
        let ink = self.font.bounds(&self.text, font_size);
        let layout = window.request_layout(
            Style {
                size: size((ink.size.width + px(1.)).into(), line_height.into()),
                flex_shrink: 0.,
                ..Default::default()
            },
            [],
            cx,
        );
        (
            layout,
            SymbolLayout {
                font_size,
                ink,
                color: style.color,
            },
        )
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut SymbolLayout,
        _: &mut Window,
        _: &mut App,
    ) {
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        layout: &mut SymbolLayout,
        _: &mut (),
        window: &mut Window,
        _: &mut App,
    ) {
        self.font.paint_colored(
            &self.text,
            bounds.origin
                + point(
                    (bounds.size.width - layout.ink.size.width) / 2. - layout.ink.origin.x,
                    (bounds.size.height - layout.ink.size.height) / 2. - layout.ink.origin.y,
                ),
            layout.color,
            layout.font_size,
            window,
        );
    }
}
