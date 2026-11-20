//! The overlay's window: the hotkey turns Impulse's window into a transparent layer over the game in
//! front, topmost on its monitor and with the input, and back into the window behind the game. Leaving the
//! game for another window turns it back too, and the game in front again brings the overlay back. It also
//! tells `Glance` whether the live game is in front, and holds the minimap's hotkey meanwhile.

mod docks;
mod glance;
mod hud;
mod settings;
mod store;
mod warning;

use std::{io, pin::pin, time::Duration};

use anyhow::Result;
use futures::{
    StreamExt,
    channel::{mpsc, oneshot},
    future,
};
use gpui_kit::{
    App, AsyncWindowContext, Context, Entity, EventEmitter, Styled, Task, WeakEntity, Window, WindowBackgroundAppearance,
    base::Root as Surface, transparent_black,
};
pub use hud::{Entered, Hud, bind_keys};
use model::Chord;
use overlay::{EXTRAS, Event, Key, Watch};
pub use settings::{OverlaySettings, overlay_hotkey};
use store::{Slot, Store};

use client::storage::Paths;

use crate::{
    run::{RunEvent, Runs},
    settings::Settings,
};

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

/// What the window is asked to do; one switch runs at a time, in order.
enum Ask {
    Toggle,
    Leave,
    Foreground,
    /// A run started or ended.
    Recheck,
    Extra(Slot),
}

struct Pressed(Slot);

/// The live game as the HUD sees it; emitted on change.
#[derive(Clone, Copy, PartialEq)]
pub(super) enum Playing {
    Idle,
    /// The game runs behind another window, or under the overlay.
    Away,
    /// The game's window is in front.
    Front(isize),
}

pub(super) struct Layer {
    mode: Mode,
    playing: Playing,
    store: Store,
    asks: mpsc::UnboundedSender<Ask>,
    hwnd: Option<isize>,
    watch: Option<Watch>,
    // The extras' keys as registered now, by `Slot::number`.
    held: [Option<Key>; EXTRAS],
    // The game in front was in exclusive fullscreen when the overlay went over it.
    exclusive: bool,
    runs: WeakEntity<Runs>,
    _switches: Task<()>,
}

impl Layer {
    pub fn new(paths: &Paths, runs: &Entity<Runs>, settings: &Entity<Settings>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let (asks, asked) = mpsc::unbounded();
        let hwnd = crate::hwnd(window);
        let mut excluded = settings.read(cx).prefs().hide_from_capture;
        if excluded && let Some(hwnd) = hwnd {
            overlay::exclude_from_capture(hwnd, true);
        }
        cx.observe(settings, move |layer, settings, cx| {
            let hidden = settings.read(cx).prefs().hide_from_capture;
            if hidden != excluded
                && let Some(hwnd) = layer.hwnd
            {
                overlay::exclude_from_capture(hwnd, hidden);
            }
            excluded = hidden;
        })
        .detach();
        let store = Store::load(paths);
        let watch = hwnd.and_then(|_| watch(store.hotkey(), &asks).inspect_err(|e| log::warn!("overlay hotkey: {e}")).ok());
        // A foreground event can be missed; the overlay losing the input is one too.
        cx.observe_window_activation(window, |layer, window, _| {
            if layer.is_over() && !window.is_window_active() {
                let _ = layer.asks.unbounded_send(Ask::Foreground);
            }
        })
        .detach();
        cx.subscribe(runs, |layer, _, event: &RunEvent, _| {
            if !matches!(event, RunEvent::Pressed(_)) {
                let _ = layer.asks.unbounded_send(Ask::Recheck);
            }
        })
        .detach();
        let switches = cx.spawn_in(window, async move |layer, cx| {
            if let Some(hwnd) = hwnd {
                let _ = switch(layer, hwnd, asked, cx).await;
            }
        });
        Self {
            mode: Mode::Window { armed: None },
            playing: Playing::Idle,
            store,
            asks,
            hwnd,
            watch,
            held: [None; EXTRAS],
            exclusive: false,
            runs: runs.downgrade(),
            _switches: switches,
        }
    }

    /// The process of the game a trainer is live in.
    fn live(&self, cx: &App) -> Option<u32> {
        self.runs.upgrade()?.read(cx).pid()
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

    /// Whether the overlay's or an extra hotkey fires on `chord`.
    pub fn takes(&self, chord: Chord) -> bool {
        self.hotkey().fired_by(chord.vks()) || Slot::ALL.into_iter().any(|slot| self.key(slot).is_some_and(|key| key.fired_by(chord.vks())))
    }

    /// The game in front was in exclusive fullscreen as the overlay went over it: the layer may not show over it.
    pub fn exclusive(&self) -> bool {
        self.exclusive
    }

    /// Fails, keeping the old one, when another app holds `key`.
    pub fn set_hotkey(&mut self, key: Key, cx: &mut Context<Self>) -> io::Result<()> {
        // An unchanged key still registers when its watch never started (another app held it then).
        if self.hwnd.is_none() || (key == self.hotkey() && self.watch.is_some()) {
            return Ok(());
        }
        // Registered before the old one goes, so a refused key leaves the old in place.
        self.watch = Some(watch(key, &self.asks)?);
        self.store.set_hotkey(key);
        // The old watch took the extras' with it.
        self.held = [None; EXTRAS];
        self.hold_keys(cx);
        cx.notify();
        Ok(())
    }

    pub fn key(&self, slot: Slot) -> Option<Key> {
        self.store.key(slot)
    }

    pub fn set_key(&mut self, slot: Slot, key: Option<Key>, cx: &mut Context<Self>) {
        self.store.set_key(slot, key);
        self.hold_keys(cx);
        cx.notify();
    }

    /// The extras are taken from every app, so only while there is a minimap to act on: the live game in front or under
    /// the overlay.
    fn hold_keys(&mut self, cx: &App) {
        let under = match self.mode {
            Mode::Over { game } => self.live(cx).is_some_and(|pid| overlay::owner(game) == pid),
            Mode::Window { .. } => false,
        };
        let held = matches!(self.playing, Playing::Front(_)) || under;
        let Some(watch) = &self.watch else { return };
        for slot in Slot::ALL {
            let wanted = held.then(|| self.store.key(slot)).flatten();
            if wanted != self.held[slot.number()] {
                // A refused key leaves none registered, so the next recheck tries it again.
                self.held[slot.number()] = match watch.set_extra(slot.number(), wanted) {
                    Ok(()) => wanted,
                    Err(e) => {
                        log::warn!("{slot:?} hotkey: {e}");
                        None
                    }
                };
            }
        }
    }

    fn set(&mut self, mode: Mode, cx: &mut Context<Self>) {
        self.mode = mode;
        cx.notify();
    }

    fn seen(&mut self, front: Option<isize>, cx: &mut Context<Self>) {
        let playing = match self.live(cx) {
            None => Playing::Idle,
            // Without the foreground hook the HUD would stay over whatever comes in front next.
            Some(pid) => match front {
                Some(front) if self.watch.is_some() && !self.is_over() && overlay::owner(front) == pid => Playing::Front(front),
                _ => Playing::Away,
            },
        };
        if playing != self.playing {
            self.playing = playing;
            cx.emit(playing);
        }
        self.hold_keys(cx);
    }
}

impl EventEmitter<Playing> for Layer {}
impl EventEmitter<Pressed> for Layer {}

fn watch(key: Key, asks: &mpsc::UnboundedSender<Ask>) -> io::Result<Watch> {
    let asks = asks.clone();
    Watch::start(key, move |event| {
        let ask = match event {
            Event::Hotkey => Ask::Toggle,
            Event::Extra(number) => match Slot::ALL.get(number) {
                Some(&slot) => Ask::Extra(slot),
                None => return,
            },
            Event::Foreground => Ask::Foreground,
        };
        let _ = asks.unbounded_send(ask);
    })
}

// Window calls on our own window run GPUI's callbacks at once, so they are made here, between updates.
// The window in front is read here too: a foreground event may be late.
async fn switch(layer: WeakEntity<Layer>, hwnd: isize, mut asked: mpsc::UnboundedReceiver<Ask>, cx: &mut AsyncWindowContext) -> Result<()> {
    while let Some(ask) = asked.next().await {
        let (mode, live) = layer.read_with(cx, |layer, cx| (layer.mode, layer.live(cx)))?;
        let front = overlay::foreground();
        match (mode, ask) {
            (_, Ask::Extra(slot)) => layer.update(cx, |_, cx| cx.emit(Pressed(slot)))?,
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
        let front = overlay::foreground();
        layer.update(cx, |layer, cx| layer.seen(front, cx))?;
    }
    Ok(())
}

async fn enter(layer: &WeakEntity<Layer>, hwnd: isize, game: isize, cx: &mut AsyncWindowContext) -> Result<()> {
    // Asked before our window covers the game: it would change the answer.
    let exclusive = overlay::exclusive(game);
    overlay::cloak(hwnd, true);
    // Fullscreen saves the window's rect for the way back, and a minimized one has none.
    overlay::unminimize(hwnd);
    layer.update_in(cx, |layer, window, cx| {
        layer.exclusive = exclusive;
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
        layer.exclusive = false;
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
