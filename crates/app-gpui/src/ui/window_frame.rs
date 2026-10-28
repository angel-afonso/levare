//! The window itself: its first size, its title, and later the parts of its
//! frame Levare draws per platform.

use super::*;

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
}

impl FrameState {
    /// Sends the title only when it changed: on X11 every title change is a
    /// round trip to the server, and Levare renders every frame of playback.
    pub fn sync(&mut self, window: &mut Window, title: String) {
        if title != self.title {
            window.set_window_title(&title);
            self.title = title;
        }
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
