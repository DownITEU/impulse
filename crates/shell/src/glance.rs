//! The HUD over the game while Impulse is behind it: one small topmost window that never takes the input,
//! at the minimap dock's place on the game's monitor. Hidden while the game is not in front, gone with the run.

use anyhow::Result;
use futures::{StreamExt, channel::mpsc};
use gpui_kit::{
    App, AsyncApp, Context, Entity, IntoElement, Render, Subscription, Task, WeakEntity, Window, WindowHandle, WindowKind, WindowOptions,
    prelude::*, px, rgb, size,
};

use super::{
    docks::Docks,
    hud::{MINIMAP_DOCK, minimap},
    layer::{Layer, Playing},
    theme::*,
};

/// Left, top, width, height in physical pixels.
type Rect = (i32, i32, i32, i32);

/// A window call to make between updates: on our own window it runs GPUI's callbacks at once.
enum Call {
    Open,
    Place(isize, Rect),
    Hide(isize),
}

struct Hud {
    window: WindowHandle<Card>,
    hwnd: isize,
    placed: Option<Rect>,
}

pub(super) struct Glance {
    docks: Entity<Docks>,
    accent: u32,
    playing: Playing,
    hud: Option<Hud>,
    _playing: Subscription,
    _calls: Task<()>,
}

impl Glance {
    pub fn new(layer: &Entity<Layer>, docks: Entity<Docks>, accent: u32, cx: &mut Context<Self>) -> Self {
        let (wake, woken) = mpsc::unbounded();
        let playing = cx.subscribe(layer, move |glance, _, playing: &Playing, _| {
            glance.playing = *playing;
            let _ = wake.unbounded_send(());
        });
        // The app quits once its last window closes.
        cx.on_release(|glance, cx| glance.close(cx)).detach();
        let calls = cx.spawn(async move |glance, cx| {
            let _ = calls(glance, woken, cx).await;
        });
        Self { docks, accent, playing: Playing::Idle, hud: None, _playing: playing, _calls: calls }
    }

    /// Settles what an update can; the rest is the next window call.
    fn next(&mut self, cx: &mut App) -> Option<Call> {
        let want = match self.playing {
            Playing::Idle => {
                self.close(cx);
                return None;
            }
            Playing::Front(game) if self.docks.read(cx).is_shown(MINIMAP_DOCK) => overlay::monitor_of(game).map(|monitor| {
                let screen = size(px(monitor.size.0 as f32 / monitor.scale), px(monitor.size.1 as f32 / monitor.scale));
                let bounds = self.docks.read(cx).bounds(MINIMAP_DOCK, screen).to_device_pixels(monitor.scale);
                (monitor.origin.0 + bounds.origin.x.0, monitor.origin.1 + bounds.origin.y.0, bounds.size.width.0, bounds.size.height.0)
            }),
            Playing::Front(_) | Playing::Away => None,
        };
        let Some(hud) = &mut self.hud else { return want.map(|_| Call::Open) };
        if hud.placed == want {
            return None;
        }
        hud.placed = want;
        Some(want.map_or(Call::Hide(hud.hwnd), |rect| Call::Place(hud.hwnd, rect)))
    }

    fn close(&mut self, cx: &mut App) {
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
                    let accent = glance.read_with(cx, |glance, _| glance.accent)?;
                    let hud = open(accent, cx)?;
                    // Before the first show, so it never takes the input.
                    overlay::pass_through(hud.hwnd);
                    glance.update(cx, |glance, _| glance.hud = Some(hud))?;
                }
                Call::Place(hwnd, rect) => overlay::place(hwnd, rect),
                Call::Hide(hwnd) => overlay::hide(hwnd),
            }
        }
    }
    Ok(())
}

fn open(accent: u32, cx: &mut AsyncApp) -> Result<Hud> {
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
    let window = cx.update(|cx| cx.open_window(options, |_, cx| cx.new(|_| Card { accent })))?;
    let hwnd = window.update(cx, |_, window, _| super::hwnd(window))?.ok_or_else(|| anyhow::anyhow!("the HUD has no window handle"))?;
    Ok(Hud { window, hwnd, placed: None })
}

struct Card {
    accent: u32,
}

impl Render for Card {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        minimap(self.accent).bg(rgb(BG)).border_1().border_color(line(0.14))
    }
}
