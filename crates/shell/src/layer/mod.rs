//! The overlay's window: the hotkey turns Impulse's window into a transparent layer over the game in
//! front, topmost on its monitor and with the input, and back into the window behind the game. Leaving the
//! game for another window turns it back too, and the game in front again brings the overlay back.

mod settings;
mod store;

use std::{io, path::Path, pin::pin, time::Duration};

use anyhow::Result;
use futures::{
    StreamExt,
    channel::{mpsc, oneshot},
    future,
};
use gpui_kit::{
    App, AsyncWindowContext, Context, Styled, Task, WeakEntity, Window, WindowBackgroundAppearance, base::Root as Surface, transparent_black,
};
use overlay::{Event, Key, Watch};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use store::Store;

// GPUI switches fullscreen in a task of its own: how often and how long to look for it.
const TICK: Duration = Duration::from_millis(8);
const TRIES: usize = 60;
// A window that stopped drawing must not stay cloaked.
const REVEAL: Duration = Duration::from_millis(500);

#[derive(Clone, Copy, PartialEq)]
enum Mode {
    /// `armed`: the game the overlay was left for another window; it comes back with the game.
    Window { armed: Option<isize> },
    /// Over `game`, with the input.
    Over { game: isize },
}

/// The process of the game a trainer runs in.
type Live = Box<dyn Fn(&App) -> Option<u32>>;

/// What the window is asked to do; one switch runs at a time, in order.
enum Ask {
    Toggle,
    Leave,
    Foreground,
}

pub(super) struct Layer {
    mode: Mode,
    store: Store,
    asks: mpsc::UnboundedSender<Ask>,
    hwnd: Option<isize>,
    watch: Option<Watch>,
    live: Live,
    _switches: Task<()>,
}

impl Layer {
    pub fn new(data_dir: &Path, live: impl Fn(&App) -> Option<u32> + 'static, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let (asks, asked) = mpsc::unbounded();
        let hwnd = match window.window_handle().map(|handle| handle.as_raw()) {
            Ok(RawWindowHandle::Win32(handle)) => Some(handle.hwnd.get()),
            _ => None,
        };
        let store = Store::load(data_dir);
        let watch = hwnd.and_then(|_| watch(store.hotkey(), &asks).inspect_err(|e| log::warn!("overlay hotkey: {e}")).ok());
        // A foreground event can be missed; the overlay losing the input is one too.
        cx.observe_window_activation(window, |layer, window, _| {
            if layer.is_over() && !window.is_window_active() {
                let _ = layer.asks.unbounded_send(Ask::Foreground);
            }
        })
        .detach();
        let switches = cx.spawn_in(window, async move |layer, cx| {
            if let Some(hwnd) = hwnd {
                let _ = switch(layer, hwnd, asked, cx).await;
            }
        });
        Self { mode: Mode::Window { armed: None }, store, asks, hwnd, watch, live: Box::new(live), _switches: switches }
    }

    pub fn is_over(&self) -> bool {
        matches!(self.mode, Mode::Over { .. })
    }

    /// Over the game in front, or from Impulse over the running game; back if over one.
    pub fn toggle(&self) {
        let _ = self.asks.unbounded_send(Ask::Toggle);
    }

    pub fn leave(&self) {
        let _ = self.asks.unbounded_send(Ask::Leave);
    }

    pub fn hotkey(&self) -> Key {
        self.store.hotkey()
    }

    /// Fails, keeping the old one, when another app holds `key`.
    pub fn set_hotkey(&mut self, key: Key, cx: &mut Context<Self>) -> io::Result<()> {
        if key == self.hotkey() || self.hwnd.is_none() {
            return Ok(());
        }
        // Registered before the old one goes, so a refused key leaves the old in place.
        self.watch = Some(watch(key, &self.asks)?);
        self.store.set_hotkey(key);
        cx.notify();
        Ok(())
    }

    fn set(&mut self, mode: Mode, cx: &mut Context<Self>) {
        self.mode = mode;
        cx.notify();
    }
}

fn watch(key: Key, asks: &mpsc::UnboundedSender<Ask>) -> io::Result<Watch> {
    let asks = asks.clone();
    Watch::start(key, move |event| {
        let _ = asks.unbounded_send(match event {
            Event::Hotkey => Ask::Toggle,
            Event::Foreground => Ask::Foreground,
        });
    })
}

// Window calls on our own window run GPUI's callbacks at once, so they are made here, between updates.
// The window in front is read here too: a foreground event may be late.
async fn switch(layer: WeakEntity<Layer>, hwnd: isize, mut asked: mpsc::UnboundedReceiver<Ask>, cx: &mut AsyncWindowContext) -> Result<()> {
    while let Some(ask) = asked.next().await {
        let (mode, live) = layer.read_with(cx, |layer, cx| (layer.mode, (layer.live)(cx)))?;
        let front = overlay::foreground();
        match (mode, ask) {
            (Mode::Window { armed }, Ask::Toggle) => {
                let other = front.filter(|&front| front != hwnd);
                if let Some(game) = other.or(armed.filter(|&game| overlay::is_window(game))).or_else(|| live.and_then(overlay::window_of)) {
                    // From Impulse in front: the game comes forward under the layer.
                    if other.is_none() {
                        overlay::give_back(game);
                    }
                    enter(&layer, hwnd, game, cx).await?;
                }
            }
            (Mode::Over { game }, Ask::Toggle | Ask::Leave) => leave(&layer, hwnd, game, None, cx).await?,
            (Mode::Over { game }, Ask::Foreground) => match front.filter(|&front| front != hwnd) {
                // Alt-Tab to the game itself: back to it, as with the hotkey.
                Some(front) if front == game => leave(&layer, hwnd, game, None, cx).await?,
                Some(front) => leave(&layer, hwnd, front, Some(game), cx).await?,
                None => {}
            },
            (Mode::Window { armed: Some(game) }, Ask::Foreground) => {
                if !overlay::is_window(game) {
                    layer.update(cx, |layer, cx| layer.set(Mode::Window { armed: None }, cx))?;
                } else if front == Some(game) {
                    enter(&layer, hwnd, game, cx).await?;
                }
            }
            _ => {}
        }
    }
    Ok(())
}

async fn enter(layer: &WeakEntity<Layer>, hwnd: isize, game: isize, cx: &mut AsyncWindowContext) -> Result<()> {
    overlay::cloak(hwnd, true);
    // Fullscreen saves the window's rect for the way back, and a minimized one has none.
    overlay::unminimize(hwnd);
    layer.update_in(cx, |layer, window, cx| {
        layer.set(Mode::Over { game }, cx);
        see_through(true, window, cx);
        window.toggle_fullscreen();
    })?;
    // Raised before GPUI's fullscreen, the layer's rect would be saved for the way back.
    fullscreen(true, cx).await?;
    overlay::raise(hwnd, game);
    reveal(hwnd, cx).await
}

/// Back to the window, right behind `below`: the window now in front, with the overlay `armed` for the game,
/// or the game itself, which gets the input back.
async fn leave(layer: &WeakEntity<Layer>, hwnd: isize, below: isize, armed: Option<isize>, cx: &mut AsyncWindowContext) -> Result<()> {
    overlay::cloak(hwnd, true);
    overlay::lower(hwnd, Some(below));
    layer.update_in(cx, |layer, window, cx| {
        layer.set(Mode::Window { armed }, cx);
        see_through(false, window, cx);
        window.toggle_fullscreen();
    })?;
    fullscreen(false, cx).await?;
    if armed.is_none() {
        overlay::give_back(below);
    }
    reveal(hwnd, cx).await
}

/// Over the game the window draws only the overlay. gpui-component paints the theme's background under the content.
fn see_through(on: bool, window: &mut Window, cx: &mut App) {
    window.set_background_appearance(match on {
        true => WindowBackgroundAppearance::Transparent,
        false => WindowBackgroundAppearance::Opaque,
    });
    Surface::update(window, cx, |surface, _, cx| {
        surface.style().background = on.then(|| transparent_black().into());
        cx.notify();
    });
}

async fn fullscreen(on: bool, cx: &mut AsyncWindowContext) -> Result<()> {
    for _ in 0..TRIES {
        if cx.update(|window, _| window.is_fullscreen())? == on {
            break;
        }
        cx.background_executor().timer(TICK).await;
    }
    Ok(())
}

/// Uncloaks once a frame of the new layout is on the screen: a next-frame callback runs before its frame
/// is drawn, so the second one runs after it.
async fn reveal(hwnd: isize, cx: &mut AsyncWindowContext) -> Result<()> {
    let (drawn, done) = oneshot::channel();
    cx.update(|window, _| {
        window.refresh();
        window.on_next_frame(|window, _| {
            window.on_next_frame(move |_, _| {
                let _ = drawn.send(());
            })
        });
    })?;
    future::select(done, pin!(cx.background_executor().timer(REVEAL))).await;
    overlay::cloak(hwnd, false);
    Ok(())
}
