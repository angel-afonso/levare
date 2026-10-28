//! The window itself: its first size, its title, and the parts of its frame
//! Levare draws itself, per platform:
//!
//! - macOS keeps its traffic lights inside Levare's title bar; AppKit moves the
//!   window from the bar's empty space.
//! - Windows has no caption once the title bar is Levare's, so Levare draws the
//!   minimize, maximize and close buttons and marks them, and the bar's empty
//!   space, as window control areas. Windows' own hit testing then gives the
//!   drag, snap layouts and double-click to maximize.

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
    maximized: bool,
}

impl FrameState {
    /// Reads the frame state for this render and sends the title only when it
    /// changed: on X11 every title change is a round trip to the server.
    pub fn sync(&mut self, window: &mut Window, title: String) {
        if title != self.title {
            window.set_window_title(&title);
            self.title = title;
        }
        self.maximized = window.is_maximized();
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
pub fn drag_area() -> Stateful<Div> {
    let area = div().id("titlebar-drag");
    if cfg!(target_os = "windows") {
        return area.window_control_area(WindowControlArea::Drag);
    }
    // macOS: AppKit moves the window. Linux has the compositor's title bar
    // as well.
    area.on_mouse_down(MouseButton::Left, |_, window, _| window.start_window_move())
}

/// Minimize, maximize or restore, and close, at the right of the title bar,
/// when the platform leaves them to Levare.
pub fn window_controls(view: &AppView) -> Option<AnyElement> {
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
    None
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
