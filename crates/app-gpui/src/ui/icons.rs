use gpui::{prelude::*, *};

#[derive(Clone, Copy)]
pub enum Icon {
    Save,
    Undo,
    Redo,
    Search,
    ZoomIn,
    ZoomOut,
    ZoomActual,
    FullScreen,
    Page,
    FitPage,
    TempoUp,
    TempoDown,
    Repeats,
    HalfSpeed,
    LeftPanel,
    RightPanel,
    Export,
    Clipboard,
    First,
    Play,
    Pause,
    Stop,
    Loop,
    Speaker,
    Mixer,
    Copy,
    Cut,
    Trash,
}

impl Icon {
    /// The toolbar model names icons; unknown names draw nothing.
    pub fn named(name: &str) -> Option<Self> {
        Some(match name {
            "save" => Self::Save,
            "undo" => Self::Undo,
            "redo" => Self::Redo,
            "search" => Self::Search,
            "zoom-in" => Self::ZoomIn,
            "zoom-out" => Self::ZoomOut,
            "zoom-actual" => Self::ZoomActual,
            "full-screen" => Self::FullScreen,
            "page" => Self::Page,
            "fit-page" => Self::FitPage,
            "tempo-up" => Self::TempoUp,
            "tempo-down" => Self::TempoDown,
            "repeats" => Self::Repeats,
            "half-speed" => Self::HalfSpeed,
            "left-panel" => Self::LeftPanel,
            "right-panel" => Self::RightPanel,
            "export" => Self::Export,
            "clipboard" => Self::Clipboard,
            "first" => Self::First,
            "play" => Self::Play,
            "stop" => Self::Stop,
            "loop" | "repeat" => Self::Loop,
            "speaker" => Self::Speaker,
            "mixer" => Self::Mixer,
            "copy" => Self::Copy,
            "cut" => Self::Cut,
            _ => return None,
        })
    }
}

pub fn icon(kind: Icon, color: u32) -> impl IntoElement {
    icon_sized(kind, color, 18.)
}

pub fn icon_sized(kind: Icon, color: u32, size: f32) -> impl IntoElement {
    canvas(
        |_, _, _| (),
        move |bounds, _, window, _| {
            let scale = size / 18.;
            let p = |x: f32, y: f32| bounds.origin + point(px(x * scale), px(y * scale));
            let mut path = PathBuilder::stroke(px(1.3 * scale));
            let mut line = |points: &[(f32, f32)]| {
                path.move_to(p(points[0].0, points[0].1));
                for &(x, y) in &points[1..] {
                    path.line_to(p(x, y));
                }
            };
            match kind {
                Icon::Trash => {
                    line(&[(3., 5.), (15., 5.)]);
                    line(&[(6., 5.), (6., 2.), (12., 2.), (12., 5.)]);
                    line(&[(4., 5.), (5., 16.), (13., 16.), (14., 5.)]);
                    line(&[(7., 8.), (7., 13.)]);
                    line(&[(11., 8.), (11., 13.)]);
                }
                Icon::Save => {
                    line(&[
                        (3., 2.),
                        (13., 2.),
                        (16., 5.),
                        (16., 16.),
                        (2., 16.),
                        (2., 2.),
                        (3., 2.),
                    ]);
                    line(&[(6., 2.), (6., 7.), (12., 7.), (12., 2.)]);
                    line(&[(6., 16.), (6., 11.), (12., 11.), (12., 16.)]);
                }
                Icon::Undo => {
                    line(&[(7., 4.), (3., 8.), (7., 12.)]);
                    line(&[(3., 8.), (11., 8.), (14., 10.), (14., 14.)]);
                }
                Icon::Redo => {
                    line(&[(11., 4.), (15., 8.), (11., 12.)]);
                    line(&[(15., 8.), (7., 8.), (4., 10.), (4., 14.)]);
                }
                Icon::Search | Icon::ZoomIn | Icon::ZoomOut => {
                    line(&[(12., 12.), (16., 16.)]);
                    if matches!(kind, Icon::ZoomIn | Icon::ZoomOut) {
                        line(&[(4.5, 7.5), (10.5, 7.5)]);
                    }
                    if matches!(kind, Icon::ZoomIn) {
                        line(&[(7.5, 4.5), (7.5, 10.5)]);
                    }
                    path.move_to(p(13., 7.5));
                    path.cubic_bezier_to(p(7.5, 13.), p(13., 10.54), p(10.54, 13.));
                    path.cubic_bezier_to(p(2., 7.5), p(4.46, 13.), p(2., 10.54));
                    path.cubic_bezier_to(p(7.5, 2.), p(2., 4.46), p(4.46, 2.));
                    path.cubic_bezier_to(p(13., 7.5), p(10.54, 2.), p(13., 4.46));
                }
                Icon::ZoomActual => {
                    // A familiar 1:1 sign, fitted to the same icon grid.
                    line(&[(2., 6.), (4., 4.), (4., 14.)]);
                    line(&[(2., 14.), (6., 14.)]);
                    line(&[(12., 6.), (14., 4.), (14., 14.)]);
                    line(&[(12., 14.), (16., 14.)]);
                    line(&[(9., 6.), (9., 7.5)]);
                    line(&[(9., 10.5), (9., 12.)]);
                }
                Icon::FullScreen => {
                    line(&[(2., 7.), (2., 2.), (7., 2.)]);
                    line(&[(11., 2.), (16., 2.), (16., 7.)]);
                    line(&[(16., 11.), (16., 16.), (11., 16.)]);
                    line(&[(7., 16.), (2., 16.), (2., 11.)]);
                }
                Icon::Page => {
                    line(&[
                        (4., 1.),
                        (10., 1.),
                        (14., 5.),
                        (14., 17.),
                        (4., 17.),
                        (4., 1.),
                    ]);
                    line(&[(10., 1.), (10., 5.), (14., 5.)]);
                    line(&[(6.5, 9.), (11.5, 9.)]);
                    line(&[(6.5, 12.), (11.5, 12.)]);
                }
                Icon::FitPage => {
                    line(&[(1., 6.), (1., 1.), (6., 1.)]);
                    line(&[(12., 1.), (17., 1.), (17., 6.)]);
                    line(&[(17., 12.), (17., 17.), (12., 17.)]);
                    line(&[(6., 17.), (1., 17.), (1., 12.)]);
                    line(&[(6., 4.), (12., 4.), (12., 14.), (6., 14.), (6., 4.)]);
                }
                Icon::TempoUp | Icon::TempoDown => {
                    line(&[(2., 16.), (6., 2.), (9., 2.), (13., 16.), (2., 16.)]);
                    line(&[(7.5, 13.), (4., 6.)]);
                    line(&[(5., 13.), (10., 13.)]);
                    line(&[(11., 5.), (17., 5.)]);
                    if matches!(kind, Icon::TempoUp) {
                        line(&[(14., 2.), (14., 8.)]);
                    }
                }
                Icon::Repeats => {
                    // Return arrow distinguishes following repeats from a playback loop.
                    line(&[(5., 6.), (2., 9.), (5., 12.)]);
                    line(&[(2., 9.), (13., 9.), (13., 3.), (8., 3.)]);
                    line(&[(7., 15.), (16., 15.)]);
                    line(&[(16., 4.), (16., 15.)]);
                }
                Icon::HalfSpeed => {
                    line(&[(3., 15.), (15., 15.)]);
                    line(&[(9., 11.), (5., 8.)]);
                    line(&[(9., 3.), (9., 5.)]);
                    path.move_to(p(2., 13.));
                    path.cubic_bezier_to(p(9., 3.), p(0., 8.), p(4., 3.));
                    path.cubic_bezier_to(p(16., 13.), p(14., 3.), p(18., 8.));
                }
                Icon::LeftPanel | Icon::RightPanel => {
                    line(&[(2., 3.), (16., 3.), (16., 15.), (2., 15.), (2., 3.)]);
                    let x = if matches!(kind, Icon::LeftPanel) {
                        7.
                    } else {
                        11.
                    };
                    line(&[(x, 3.), (x, 15.)]);
                }
                Icon::Export => {
                    line(&[(9., 13.), (9., 1.), (5., 5.)]);
                    line(&[(9., 1.), (13., 5.)]);
                    line(&[(3., 10.), (3., 16.), (15., 16.), (15., 10.)]);
                }
                Icon::Clipboard => {
                    line(&[
                        (6., 3.),
                        (3., 3.),
                        (3., 17.),
                        (15., 17.),
                        (15., 3.),
                        (12., 3.),
                    ]);
                    line(&[(6., 1.), (12., 1.), (12., 5.), (6., 5.), (6., 1.)]);
                    line(&[(6., 9.), (12., 9.)]);
                    line(&[(6., 12.), (11., 12.)]);
                }
                Icon::First => {
                    line(&[(3., 3.), (3., 15.)]);
                    line(&[(14., 3.), (5., 9.), (14., 15.), (14., 3.)]);
                }
                Icon::Play => {
                    line(&[(5., 2.), (15., 9.), (5., 16.), (5., 2.)]);
                }
                Icon::Pause => {
                    line(&[(6., 3.), (6., 15.)]);
                    line(&[(12., 3.), (12., 15.)]);
                }
                Icon::Stop => {
                    line(&[(4., 4.), (14., 4.), (14., 14.), (4., 14.), (4., 4.)]);
                }
                Icon::Loop => {
                    line(&[(3., 9.), (3., 5.), (15., 5.)]);
                    line(&[(12., 2.), (15., 5.), (12., 8.)]);
                    line(&[(15., 9.), (15., 13.), (3., 13.)]);
                    line(&[(6., 10.), (3., 13.), (6., 16.)]);
                }
                Icon::Mixer => {
                    for (x, knob) in [(4., 11.), (9., 5.), (14., 9.)] {
                        line(&[(x, 2.), (x, 16.)]);
                        line(&[(x - 2., knob), (x + 2., knob)]);
                    }
                }
                Icon::Copy => {
                    line(&[(6., 6.), (16., 6.), (16., 16.), (6., 16.), (6., 6.)]);
                    line(&[(12., 6.), (12., 2.), (2., 2.), (2., 12.), (6., 12.)]);
                }
                Icon::Cut => {
                    line(&[(4., 2.), (13., 12.)]);
                    line(&[(14., 2.), (5., 12.)]);
                    line(&[(2., 13.), (5., 12.), (6., 15.), (3., 16.), (2., 13.)]);
                    line(&[(16., 13.), (13., 12.), (12., 15.), (15., 16.), (16., 13.)]);
                }
                Icon::Speaker => {
                    line(&[
                        (2., 7.),
                        (6., 7.),
                        (10., 3.),
                        (10., 15.),
                        (6., 11.),
                        (2., 11.),
                        (2., 7.),
                    ]);
                    line(&[(13., 6.), (15., 9.), (13., 12.)]);
                }
            }
            if let Ok(path) = path.build() {
                window.paint_path(path, rgb(color));
            }
        },
    )
    .w(px(size))
    .h(px(size))
    .flex_shrink_0()
}
