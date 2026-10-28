//! The parts of the window frame Levare draws itself, per platform:
//!
//! - macOS keeps its traffic lights inside Levare's title bar; AppKit moves the
//!   window from the bar's empty space.
//! - Windows has no caption once the title bar is Levare's, so Levare draws the
//!   minimize, maximize and close buttons and marks them, and the bar's empty
//!   space, as window control areas. Windows' own hit testing then gives the
//!   drag, snap layouts and double-click to maximize.
//! - Linux asks for client-side decorations. When the compositor agrees,
//!   Levare draws the border, shadow, resize edges and buttons; when it keeps
//!   server-side ones, the compositor draws everything and Levare adds nothing.

use super::icons::{Icon, icon_sized};
use super::*;

/// Height of Levare's title bar; the Windows caption buttons fill it.
pub const TITLEBAR_HEIGHT: f32 = 44.;
/// Windows caption buttons follow the system's: 46 px wide.
const CAPTION_WIDTH: f32 = 46.;
/// Hover colour of the Windows close button, the system's red.
const CLOSE_HOVER: u32 = 0xc42b1c;
/// Thickness of the top resize edge on Windows, above the drag area.
const TOP_RESIZE: f32 = 4.;
/// The transparent margin around a client-decorated Linux window that holds
/// its shadow and resize edges.
const SHADOW: Pixels = px(10.);
const ROUNDING: Pixels = px(10.);

/// The size the first window asks for, and its smallest size.
const PREFERRED: (f32, f32) = (1440., 1000.);
const MINIMUM: (f32, f32) = (960., 680.);
/// Room left around the first window for the taskbar, dock or menu bar, which
/// GPUI's display bounds include.
const SCREEN_MARGIN: (f32, f32) = (32., 96.);

/// What the platform said about the frame at the last render.
#[derive(Default)]
pub struct FrameState {
    /// The window title last sent to the platform.
    title: String,
    decorations: Decorations,
    controls: WindowControls,
    maximized: bool,
    /// Linux: the left button went down on the title bar's empty space; the
    /// next mouse move hands the window to the compositor. Waiting for the
    /// move keeps double-click free for maximize.
    move_on_drag: bool,
}

impl FrameState {
    /// Reads the frame state for this render and sends the title only when it
    /// changed: on X11 every title change is a round trip to the server.
    pub fn sync(&mut self, window: &mut Window, title: String) {
        if title != self.title {
            window.set_window_title(&title);
            self.title = title;
        }
        self.decorations = window.window_decorations();
        self.controls = window.window_controls();
        self.maximized = window.is_maximized();
        if cfg!(any(target_os = "linux", target_os = "freebsd")) {
            window.set_client_inset(if self.client_side() { SHADOW } else { px(0.) });
        }
    }

    /// Levare draws the frame: Linux with client-side decorations.
    fn client_side(&self) -> bool {
        matches!(self.decorations, Decorations::Client { .. })
    }
}

/// The first window: the preferred size, shrunk to fit the primary display,
/// and centered on it.
pub fn first_window(cx: &App) -> (Bounds<Pixels>, Size<Pixels>) {
    let display = cx.primary_display().map(|display| display.bounds());
    let (window, minimum) = fit_first_window(display.map(|bounds| bounds.size));
    let bounds = match display {
        Some(display) => Bounds::centered_at(display.center(), window),
        None => Bounds::centered(None, window, cx),
    };
    (bounds, minimum)
}

/// The window and minimum sizes for a display of `screen` size: never larger
/// than the screen minus room for the taskbar, dock or menu bar.
fn fit_first_window(screen: Option<Size<Pixels>>) -> (Size<Pixels>, Size<Pixels>) {
    let preferred = size(px(PREFERRED.0), px(PREFERRED.1));
    let minimum = size(px(MINIMUM.0), px(MINIMUM.1));
    let Some(screen) = screen else {
        return (preferred, minimum);
    };
    let room = size(
        (screen.width - px(SCREEN_MARGIN.0)).max(px(MINIMUM.0 / 2.)),
        (screen.height - px(SCREEN_MARGIN.1)).max(px(MINIMUM.1 / 2.)),
    );
    (preferred.min(&room), minimum.min(&room))
}

/// Wraps the whole window. With Linux client-side decorations it adds the
/// transparent margin with the resize edges and gives `content` the window's
/// border, rounded top corners and shadow; elsewhere it returns `content`.
pub fn frame(state: &FrameState, content: Div) -> Div {
    let Decorations::Client { tiling } = state.decorations else {
        return content;
    };
    let content = content
        .border_color(rgb(OUTLINE))
        .when(!tiling.top, |d| d.border_t_1())
        .when(!tiling.bottom, |d| d.border_b_1())
        .when(!tiling.left, |d| d.border_l_1())
        .when(!tiling.right, |d| d.border_r_1())
        .when(!(tiling.top || tiling.left), |d| d.rounded_tl(ROUNDING))
        .when(!(tiling.top || tiling.right), |d| d.rounded_tr(ROUNDING))
        .when(!tiling.is_tiled(), |d| {
            d.shadow(vec![BoxShadow {
                color: hsla(0., 0., 0., 0.45),
                offset: point(px(0.), px(1.)),
                blur_radius: SHADOW / 2.,
                spread_radius: px(0.),
            }])
        });
    let edges = [
        (ResizeEdge::Top, !tiling.top),
        (ResizeEdge::Bottom, !tiling.bottom),
        (ResizeEdge::Left, !tiling.left),
        (ResizeEdge::Right, !tiling.right),
        (ResizeEdge::TopLeft, !(tiling.top || tiling.left)),
        (ResizeEdge::TopRight, !(tiling.top || tiling.right)),
        (ResizeEdge::BottomLeft, !(tiling.bottom || tiling.left)),
        (ResizeEdge::BottomRight, !(tiling.bottom || tiling.right)),
    ];
    div()
        .size_full()
        .relative()
        .bg(transparent_black())
        .when(!tiling.top, |d| d.pt(SHADOW))
        .when(!tiling.bottom, |d| d.pb(SHADOW))
        .when(!tiling.left, |d| d.pl(SHADOW))
        .when(!tiling.right, |d| d.pr(SHADOW))
        .child(content)
        .children(
            edges
                .into_iter()
                .filter(|(_, shown)| *shown)
                .map(|(edge, _)| resize_handle(edge)),
        )
}

/// One edge or corner of the margin: the resize cursor, and a press hands the
/// resize to the compositor. Corners come after edges so they sit on top.
fn resize_handle(edge: ResizeEdge) -> Div {
    let cursor = match edge {
        ResizeEdge::Top | ResizeEdge::Bottom => CursorStyle::ResizeUpDown,
        ResizeEdge::Left | ResizeEdge::Right => CursorStyle::ResizeLeftRight,
        ResizeEdge::TopLeft | ResizeEdge::BottomRight => CursorStyle::ResizeUpLeftDownRight,
        ResizeEdge::TopRight | ResizeEdge::BottomLeft => CursorStyle::ResizeUpRightDownLeft,
    };
    let handle =
        div()
            .absolute()
            .cursor(cursor)
            .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                cx.stop_propagation();
                window.start_window_resize(edge);
            });
    match edge {
        ResizeEdge::Top => handle.top_0().left(SHADOW).right(SHADOW).h(SHADOW),
        ResizeEdge::Bottom => handle.bottom_0().left(SHADOW).right(SHADOW).h(SHADOW),
        ResizeEdge::Left => handle.left_0().top(SHADOW).bottom(SHADOW).w(SHADOW),
        ResizeEdge::Right => handle.right_0().top(SHADOW).bottom(SHADOW).w(SHADOW),
        ResizeEdge::TopLeft => handle.top_0().left_0().size(SHADOW),
        ResizeEdge::TopRight => handle.top_0().right_0().size(SHADOW),
        ResizeEdge::BottomLeft => handle.bottom_0().left_0().size(SHADOW),
        ResizeEdge::BottomRight => handle.bottom_0().right_0().size(SHADOW),
    }
}

/// Settings the title bar itself needs on each platform.
pub fn titlebar_frame(view: &AppView, bar: Div) -> Div {
    if cfg!(target_os = "windows") {
        // Occluding keeps GPUI from handling presses on the bar's empty space
        // and the caption buttons (the editor would take focus and swallow
        // them), so Windows gets them and drags, snaps or closes the window.
        bar.relative()
            .occlude()
            .pr_0()
            .when(!view.ui.frame.maximized, |bar| {
                // A strip with no control area, so Windows offers its top
                // resize edge there instead of the drag area's caption.
                bar.child(
                    div()
                        .absolute()
                        .top_0()
                        .left_0()
                        .right_0()
                        .h(px(TOP_RESIZE))
                        .occlude(),
                )
            })
    } else {
        bar
    }
}

/// The title bar's empty space, which moves the window.
pub fn drag_area(view: &AppView, cx: &Context<AppView>) -> Stateful<Div> {
    let area = div().id("titlebar-drag");
    if cfg!(target_os = "windows") {
        return area.window_control_area(WindowControlArea::Drag);
    }
    if !view.ui.frame.client_side() {
        // macOS: AppKit moves the window. Linux with server-side decorations
        // has the compositor's own title bar as well.
        return area.on_mouse_down(MouseButton::Left, |_, window, _| window.start_window_move());
    }
    area.on_mouse_down(
        MouseButton::Left,
        cx.listener(|view, _, _, _| view.ui.frame.move_on_drag = true),
    )
    .on_mouse_up(
        MouseButton::Left,
        cx.listener(|view, _, _, _| view.ui.frame.move_on_drag = false),
    )
    .on_mouse_down_out(cx.listener(|view, _, _, _| view.ui.frame.move_on_drag = false))
    .on_mouse_move(cx.listener(|view, _, window, _| {
        if std::mem::take(&mut view.ui.frame.move_on_drag) {
            window.start_window_move();
        }
    }))
    .on_click(|event, window, _| {
        if event.click_count() == 2 {
            window.zoom_window();
        }
    })
    .when(view.ui.frame.controls.window_menu, |area| {
        area.on_mouse_down(MouseButton::Right, |event, window, _| {
            window.show_window_menu(event.position)
        })
    })
}

/// Minimize, maximize or restore, and close, at the right of the title bar,
/// when the platform leaves them to Levare.
pub fn window_controls(view: &AppView, cx: &Context<AppView>) -> Option<AnyElement> {
    let frame = &view.ui.frame;
    let maximize = if frame.maximized {
        Icon::WindowRestore
    } else {
        Icon::WindowMaximize
    };
    if cfg!(target_os = "windows") {
        return Some(
            div()
                .flex()
                .h_full()
                .flex_shrink_0()
                .child(caption_button(
                    "window-minimize",
                    Icon::WindowMinimize,
                    WindowControlArea::Min,
                    HOVER,
                ))
                .child(caption_button(
                    "window-maximize",
                    maximize,
                    WindowControlArea::Max,
                    HOVER,
                ))
                .child(caption_button(
                    "window-close",
                    Icon::WindowClose,
                    WindowControlArea::Close,
                    CLOSE_HOVER,
                ))
                .into_any_element(),
        );
    }
    if !frame.client_side() {
        return None;
    }
    Some(
        div()
            .flex()
            .items_center()
            .gap_1()
            .flex_shrink_0()
            .when(frame.controls.minimize, |row| {
                row.child(frame_button(
                    "window-minimize",
                    Icon::WindowMinimize,
                    cx,
                    |_, window, _| window.minimize_window(),
                ))
            })
            .when(frame.controls.maximize, |row| {
                row.child(frame_button(
                    "window-maximize",
                    maximize,
                    cx,
                    |_, window, _| window.zoom_window(),
                ))
            })
            .child(frame_button(
                "window-close",
                Icon::WindowClose,
                cx,
                |view, window, cx| view.run(id::CLOSE, window, cx),
            ))
            .into_any_element(),
    )
}

/// A Windows caption button. It has no click handler: Windows hit-tests it
/// through its control area and minimizes, maximizes or closes the window
/// itself, which also gives Windows 11 its snap layouts on maximize.
fn caption_button(
    id: &'static str,
    glyph: Icon,
    area: WindowControlArea,
    hover: u32,
) -> Stateful<Div> {
    div()
        .id(id)
        .w(px(CAPTION_WIDTH))
        .h_full()
        .flex()
        .flex_shrink_0()
        .items_center()
        .justify_center()
        .hover(move |style| style.bg(rgb(hover)))
        .window_control_area(area)
        .child(icon_sized(glyph, TEXT, 14.))
}

/// A Linux window button, styled like the title bar's other icon buttons.
fn frame_button(
    id: &'static str,
    glyph: Icon,
    cx: &Context<AppView>,
    click: impl Fn(&mut AppView, &mut Window, &mut Context<AppView>) + 'static,
) -> Stateful<Div> {
    action(id, icon_sized(glyph, MUTED, 16.), false, cx, click)
        .w(px(28.))
        .h(px(28.))
}

#[cfg(test)]
mod tests {
    // Not `super::*`: GPUI's glob would bring its own `test` attribute.
    use super::{MINIMUM, PREFERRED, fit_first_window};
    use gpui::{px, size};

    fn fit(width: f32, height: f32) -> ((f32, f32), (f32, f32)) {
        let (window, minimum) = fit_first_window(Some(size(px(width), px(height))));
        (
            (f32::from(window.width), f32::from(window.height)),
            (f32::from(minimum.width), f32::from(minimum.height)),
        )
    }

    #[test]
    fn a_large_screen_gets_the_preferred_size() {
        assert_eq!(fit(2560., 1440.), (PREFERRED, MINIMUM));
    }

    #[test]
    fn a_small_screen_holds_the_whole_window() {
        let (window, minimum) = fit(1024., 768.);
        assert_eq!(window, (992., 672.));
        assert!(window.0 <= 1024. && window.1 <= 768. - 48., "{window:?}");
        assert_eq!(minimum, (960., 672.), "the minimum shrinks with it");
    }

    #[test]
    fn without_a_display_it_asks_for_the_preferred_size() {
        let (window, minimum) = fit_first_window(None);
        assert_eq!(window, size(px(PREFERRED.0), px(PREFERRED.1)));
        assert_eq!(minimum, size(px(MINIMUM.0), px(MINIMUM.1)));
    }
}
