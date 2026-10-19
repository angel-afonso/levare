use gpui::{prelude::*, *};

#[derive(Clone, Copy)]
pub enum Icon {
    Save,
    Undo,
    Redo,
    Search,
    LeftPanel,
    RightPanel,
    Export,
    Clipboard,
    AddStaff,
    First,
    Play,
    Pause,
    Stop,
    Loop,
    Speaker,
    AddBar,
    Chord,
    Mixer,
    Copy,
    Cut,
    Tie,
    Slur,
    Trash,
}

impl Icon {
    /// The toolbar model names icons; unknown names draw nothing.
    pub fn named(name: &str) -> Option<Self> {
        Some(match name {
            "save" => Self::Save,
            "undo" => Self::Undo,
            "redo" => Self::Redo,
            "tie" => Self::Tie,
            "slur" => Self::Slur,
            "search" => Self::Search,
            "left-panel" => Self::LeftPanel,
            "right-panel" => Self::RightPanel,
            "export" => Self::Export,
            "clipboard" => Self::Clipboard,
            "add-staff" => Self::AddStaff,
            "first" => Self::First,
            "play" => Self::Play,
            "stop" => Self::Stop,
            "loop" | "repeat" => Self::Loop,
            "speaker" => Self::Speaker,
            "add-bar" => Self::AddBar,
            "chord" => Self::Chord,
            "mixer" => Self::Mixer,
            "copy" => Self::Copy,
            "cut" => Self::Cut,
            _ => return None,
        })
    }
}

pub fn icon(kind: Icon, color: u32) -> impl IntoElement {
    canvas(
        |_, _, _| (),
        move |bounds, _, window, _| {
            let p = |x: f32, y: f32| bounds.origin + point(px(x), px(y));
            let mut path = PathBuilder::stroke(px(1.3));
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
                Icon::Search => {
                    line(&[(13., 13.), (17., 17.)]);
                    line(&[
                        (5., 2.),
                        (10., 2.),
                        (13., 5.),
                        (13., 10.),
                        (10., 13.),
                        (5., 13.),
                        (2., 10.),
                        (2., 5.),
                        (5., 2.),
                    ]);
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
                Icon::AddStaff => {
                    for y in [3., 6., 9., 12., 15.] {
                        line(&[(2., y), (12., y)]);
                    }
                    if matches!(kind, Icon::AddStaff) {
                        line(&[(15., 9.), (15., 17.)]);
                        line(&[(11., 13.), (18., 13.)]);
                    }
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
                    line(&[(3., 7.), (3., 4.), (14., 4.), (14., 1.)]);
                    line(&[(11., 2.), (14., 4.), (17., 2.)]);
                    line(&[(15., 11.), (15., 14.), (4., 14.), (4., 17.)]);
                    line(&[(1., 16.), (4., 14.), (7., 16.)]);
                }
                Icon::AddBar => {
                    for y in [4., 7., 10., 13.] {
                        line(&[(1., y), (11., y)]);
                    }
                    line(&[(6., 4.), (6., 13.)]);
                    line(&[(15., 6.), (15., 14.)]);
                    line(&[(11., 10.), (18., 10.)]);
                }
                Icon::Chord => {
                    for (x, y) in [(5., 14.), (7., 10.), (5., 6.)] {
                        line(&[
                            (x - 3., y),
                            (x, y - 2.),
                            (x + 3., y),
                            (x, y + 2.),
                            (x - 3., y),
                        ]);
                    }
                    line(&[(8.5, 13.), (8.5, 2.), (14., 4.)]);
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
                Icon::Tie => {
                    line(&[(2., 7.), (2.5, 7.5)]);
                    line(&[(15.5, 7.), (16., 7.5)]);
                    line(&[(2., 10.), (5., 13.), (9., 14.), (13., 13.), (16., 10.)]);
                }
                Icon::Slur => {
                    line(&[(1., 14.), (4., 8.), (9., 6.), (14., 8.), (17., 14.)]);
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
    .w(px(18.))
    .h(px(18.))
    .flex_shrink_0()
}
