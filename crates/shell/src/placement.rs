//! Where the window was: its monitor, its bounds and whether it was maximized, kept in `window.json`. `restore` gives
//! them to the next start's window; `Keeper` saves them as the window moves.

use std::{path::Path, time::Duration};

use client::storage::{Paths, Stored};
use gpui_kit::{App, Bounds, Context, DisplayId, Pixels, Size, Subscription, Task, Window, WindowBounds, point, px, size};
use serde::{Deserialize, Serialize};

/// A move or a resize is a burst of changes: the file is written once the window settles.
const SETTLE: Duration = Duration::from_millis(500);

#[derive(Default, Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(default)]
struct Placement {
    /// The monitor's UUID, which stays across starts while its id does not.
    display: Option<String>,
    /// The restored bounds in logical pixels: left, top, width, height.
    bounds: Option<[f32; 4]>,
    maximized: bool,
}

impl Placement {
    /// `None` for a fullscreen window: the overlay's, not where the window lives.
    fn of(bounds: WindowBounds, display: Option<String>) -> Option<Self> {
        let (bounds, maximized) = match bounds {
            WindowBounds::Windowed(bounds) => (bounds, false),
            WindowBounds::Maximized(bounds) => (bounds, true),
            WindowBounds::Fullscreen(_) => return None,
        };
        let bounds = [bounds.origin.x, bounds.origin.y, bounds.size.width, bounds.size.height].map(Pixels::as_f32);
        Some(Self { display, bounds: Some(bounds), maximized })
    }

    /// The bounds to open with; `None` before a first save or for a damaged file.
    fn window_bounds(&self) -> Option<WindowBounds> {
        let [left, top, width, height] = self.bounds.filter(|bounds| bounds.iter().all(|value| value.is_finite()))?;
        let bounds = Bounds { origin: point(px(left), px(top)), size: size(px(width), px(height)) };
        let bounds = if self.maximized { WindowBounds::Maximized(bounds) } else { WindowBounds::Windowed(bounds) };
        (width > 0. && height > 0.).then_some(bounds)
    }
}

/// The window's bounds and monitor from its last run, else `default` centered on the primary monitor (blocking: a
/// tiny file). A monitor gone opens `default` on the primary one, and bounds off every monitor take the platform's
/// default.
pub fn restore(data_dir: &Path, default: Size<Pixels>, cx: &App) -> (WindowBounds, Option<DisplayId>) {
    let saved: Stored<Placement> = Stored::load(Paths::new(data_dir.to_owned()).window());
    let display = saved.display.as_deref().and_then(|wanted| {
        cx.displays().into_iter().find(|display| display.uuid().is_ok_and(|uuid| uuid.to_string() == wanted)).map(|display| display.id())
    });
    // A gone monitor's bounds would open off screen or at its scale.
    let Some(bounds) = saved.window_bounds().filter(|_| display.is_some()) else {
        let centered = Bounds::centered(None, default, cx);
        return (if saved.maximized { WindowBounds::Maximized(centered) } else { WindowBounds::Windowed(centered) }, None);
    };
    (bounds, display)
}

/// Saves the window's placement once it settles after a move or a resize, or at quit.
pub struct Keeper {
    saved: Stored<Placement>,
    pending: Option<Placement>,
    save: Task<()>,
    _subscriptions: [Subscription; 2],
}

impl Keeper {
    pub fn new(paths: &Paths, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let moved = cx.observe_window_bounds(window, |keeper, window, cx| keeper.moved(window, cx));
        let quit = cx.on_app_quit(|keeper, _| {
            keeper.flush();
            async {}
        });
        Self { saved: Stored::load(paths.window()), pending: None, save: Task::ready(()), _subscriptions: [moved, quit] }
    }

    fn moved(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // A minimized window reports its restored bounds as windowed, a maximized one's too.
        if crate::hwnd(window).is_some_and(topmost::is_minimized) {
            return;
        }
        let display = window.display(cx).and_then(|display| display.uuid().ok()).map(|uuid| uuid.to_string());
        self.pending = Placement::of(window.window_bounds(), display).filter(|placement| *placement != *self.saved);
        // Fullscreen drops the wait too: the window shown on the overlay's way there is not where it lives.
        self.save = match self.pending {
            Some(_) => cx.spawn(async move |keeper, cx| {
                cx.background_executor().timer(SETTLE).await;
                let _ = keeper.update(cx, |keeper, _| keeper.flush());
            }),
            None => Task::ready(()),
        };
    }

    fn flush(&mut self) {
        if let Some(placement) = self.pending.take() {
            self.saved.edit(|saved| *saved = placement);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bounds(left: f32, top: f32, width: f32, height: f32) -> Bounds<Pixels> {
        Bounds { origin: point(px(left), px(top)), size: size(px(width), px(height)) }
    }

    #[test]
    fn a_placement_round_trips_windowed_and_maximized_and_skips_fullscreen() {
        let windowed = Placement::of(WindowBounds::Windowed(bounds(-1600., 120., 1400., 900.)), Some("d".into())).unwrap();
        assert_eq!((windowed.bounds, windowed.maximized), (Some([-1600., 120., 1400., 900.]), false));
        assert_eq!(windowed.window_bounds(), Some(WindowBounds::Windowed(bounds(-1600., 120., 1400., 900.))));
        let maximized = Placement::of(WindowBounds::Maximized(bounds(10., 20., 1280., 800.)), None).unwrap();
        assert_eq!(maximized.window_bounds(), Some(WindowBounds::Maximized(bounds(10., 20., 1280., 800.))));
        assert!(Placement::of(WindowBounds::Fullscreen(bounds(0., 0., 1920., 1080.)), None).is_none());
    }

    #[test]
    fn no_bounds_or_damaged_ones_open_with_the_default() {
        assert_eq!(Placement::default().window_bounds(), None);
        for damaged in [[0., 0., 0., 800.], [0., 0., 1280., -1.], [f32::NAN, 0., 1280., 800.]] {
            assert_eq!(Placement { bounds: Some(damaged), ..Placement::default() }.window_bounds(), None, "{damaged:?}");
        }
    }
}
