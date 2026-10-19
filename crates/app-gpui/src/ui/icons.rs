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
