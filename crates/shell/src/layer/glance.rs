//! The HUD over the game while Impulse is behind it: one small topmost window that never takes the input,
//! with the minimap at its dock's place on the game's monitor. Shown while the game is in front and its live
//! map is up, gone with the run.

use anyhow::Result;
use futures::{StreamExt, channel::mpsc};
use gpui_kit::{
    App, AsyncApp, Context, Entity, IntoElement, Render, Task, WeakEntity, Window, WindowHandle, WindowKind, WindowOptions, div,
    prelude::*, px, size,
};

use super::{
    Layer, Playing,
    docks::{self, Docks},
    hud::MINIMAP_DOCK,
};
use crate::{
    map::{LiveMap, Minimap},
    settings::Settings,
    ui::{theme::*, widgets::brackets},
};

/// Left, top, width, height in physical pixels.
type Rect = (i32, i32, i32, i32);

/// A window call to make between updates: on our own window it runs GPUI's callbacks at once.
enum Call {
    Open,
    Place(isize, Rect),
    Hide(isize),
}

struct Popup {
    window: WindowHandle<Card>,
    hwnd: isize,
    placed: Option<Rect>,
}

pub(super) struct Glance {
    docks: Entity<Docks>,
    live: Entity<LiveMap>,
    playing: Playing,
    // The live map follows the player while the HUD could show, so it shows once they are placed again.
    following: bool,
    // From screen capture.
    hidden: bool,
    hud: Option<Popup>,
    _calls: Task<()>,
}

impl Glance {
    pub fn new(
        layer: &Entity<Layer>,
        docks: Entity<Docks>,
        live: Entity<LiveMap>,
        settings: &Entity<Settings>,
        cx: &mut Context<Self>,
    ) -> Self {
        cx.observe(settings, |glance, settings, cx| {
            let hidden = settings.read(cx).prefs().hide_from_capture;
            if hidden != glance.hidden {
                glance.set_hidden(hidden);
            }
        })
        .detach();
        let hidden = settings.read(cx).prefs().hide_from_capture;
        let (wake, woken) = mpsc::unbounded();
        cx.subscribe(layer, {
            let wake = wake.clone();
            move |glance, _, playing: &Playing, _| {
                glance.playing = *playing;
                let _ = wake.unbounded_send(());
            }
        })
        .detach();
        // The live map comes up or goes after the game is in front.
        cx.observe(&live, move |_, _, _| {
            let _ = wake.unbounded_send(());
        })
        .detach();
        // The app quits once its last window closes.
        cx.on_release(|glance, cx| glance.close(cx)).detach();
        let calls = cx.spawn(async move |glance, cx| {
            let _ = calls(glance, woken, cx).await;
        });
        Self { docks, live, playing: Playing::Idle, following: false, hidden, hud: None, _calls: calls }
    }

    fn set_hidden(&mut self, hidden: bool) {
        self.hidden = hidden;
        if let Some(hud) = &self.hud {
            overlay::exclude_from_capture(hud.hwnd, hidden);
        }
    }

    /// Settles what an update can; the rest is the next window call.
    fn next(&mut self, cx: &mut App) -> Option<Call> {
        let game = match self.playing {
            Playing::Idle => {
                self.close(cx);
                return None;
            }
            Playing::Front(game) if self.docks.read(cx).is_shown(MINIMAP_DOCK) && self.live.read(cx).ready(cx) => Some(game),
            Playing::Front(_) | Playing::Away => None,
        };
        self.follow(game.is_some(), cx);
        // Where the dock's minimap is.
        let want = game.filter(|_| self.live.read(cx).placed()).and_then(|game| {
            let monitor = overlay::monitor_of(game)?;
            let screen = size(px(monitor.size.0 as f32 / monitor.scale), px(monitor.size.1 as f32 / monitor.scale));
            let bounds = docks::below_header(self.docks.read(cx).bounds(MINIMAP_DOCK, screen)).to_device_pixels(monitor.scale);
            Some((monitor.origin.0 + bounds.origin.x.0, monitor.origin.1 + bounds.origin.y.0, bounds.size.width.0, bounds.size.height.0))
        });
        let Some(hud) = &mut self.hud else { return want.map(|_| Call::Open) };
        if hud.placed == want {
            return None;
        }
        let (window, hwnd) = (hud.window, hud.hwnd);
        hud.placed = want;
        // Hidden, it lets go of its tiles.
        if want.is_none() {
            window.update(cx, |card, window, cx| card.minimap.update(cx, |minimap, cx| minimap.hide(window, cx))).ok();
        }
        Some(want.map_or(Call::Hide(hwnd), |rect| Call::Place(hwnd, rect)))
    }

    fn follow(&mut self, follow: bool, cx: &mut App) {
        if follow != self.following {
            self.following = follow;
            self.live.update(cx, |live, cx| live.set_shown(follow, cx));
        }
    }

    fn close(&mut self, cx: &mut App) {
        self.follow(false, cx);
        if let Some(hud) = self.hud.take() {
            hud.window.update(cx, |_, window, _| window.remove_window()).ok();
        }
    }
}

async fn calls(glance: WeakEntity<Glance>, mut woken: mpsc::UnboundedReceiver<()>, cx: &mut AsyncApp) -> Result<()> {
    while woken.next().await.is_some() {
        while woken.try_recv().is_ok() {}
        while let Some(call) = glance.update(cx, |glance, cx| glance.next(cx))? {
            match call {
                Call::Open => {
                    let (live, hidden) = glance.read_with(cx, |glance, _| (glance.live.clone(), glance.hidden))?;
                    let hud = open(live, cx)?;
                    // Before the first show, so it never takes the input.
                    overlay::pass_through(hud.hwnd);
                    overlay::exclude_from_capture(hud.hwnd, hidden);
                    glance.update(cx, |glance, _| glance.hud = Some(hud))?;
                }
                Call::Place(hwnd, rect) => overlay::place(hwnd, rect),
                Call::Hide(hwnd) => overlay::hide(hwnd),
            }
        }
    }
    Ok(())
}

fn open(live: Entity<LiveMap>, cx: &mut AsyncApp) -> Result<Popup> {
    let options = WindowOptions {
        titlebar: None,
        focus: false,
        show: false,
        kind: WindowKind::PopUp,
        is_movable: false,
        is_resizable: false,
        is_minimizable: false,
        ..Default::default()
    };
    let window = cx.update(|cx| {
        cx.open_window(options, |_, cx| {
            let minimap = cx.new(|cx| Minimap::new(live.clone(), cx));
            cx.new(|_| Card { minimap })
        })
    })?;
    let hwnd = window.update(cx, |_, window, _| crate::hwnd(window))?.ok_or_else(|| anyhow::anyhow!("the HUD has no window handle"))?;
    Ok(Popup { window, hwnd, placed: None })
}

struct Card {
    minimap: Entity<Minimap>,
}

impl Render for Card {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div().relative().size_full().border_1().border_color(line(0.14)).child(self.minimap.clone()).children(brackets(cx))
    }
}
