//! Impulse in the notification area. With close to tray on (the default) the close button hides the window
//! there; the icon opens it, and its menu also stops the running trainer, toggles the overlay and quits.

use futures::{StreamExt, channel::mpsc};
use gpui_kit::{Context, Window};
use tray::{Event, Menu, Tray};

use super::{Page, hwnd};

impl Page {
    /// The icon, and the close button wired to it; `None` when the notification area refused it.
    pub(super) fn start_tray(window: &mut Window, cx: &mut Context<Self>) -> Option<Tray> {
        let hwnd = hwnd(window)?;
        let (events, mut received) = mpsc::unbounded();
        let tray = Tray::new("Impulse", move |event| drop(events.unbounded_send(event)))
            .inspect_err(|e| log::warn!("tray: {e}"))
            .ok()?;
        cx.spawn_in(window, async move |page, cx| {
            while let Some(event) = received.next().await {
                page.update(cx, |page, cx| page.tray_event(event, hwnd, cx)).ok();
            }
        })
        .detach();
        let page = cx.weak_entity();
        window.on_window_should_close(cx, move |_, cx| page.update(cx, |page, cx| page.may_close(hwnd, cx)).unwrap_or(true));
        // Any change may be the run's or the overlay's.
        cx.observe_self(Self::sync_tray).detach();
        Some(tray)
    }

    fn tray_event(&mut self, event: Event, hwnd: isize, cx: &mut Context<Self>) {
        match event {
            Event::Open => tray::show(hwnd),
            Event::Stop => self.stop_run(cx),
            Event::Overlay => self.layer.read(cx).toggle(),
            Event::Quit => {
                // Quitting skips drops: the icon would stay until a hover.
                self.tray = None;
                cx.quit();
            }
        }
    }

    /// `false` keeps the window: hidden in the tray, or back from over a game, where the overlay has the input.
    fn may_close(&mut self, hwnd: isize, cx: &mut Context<Self>) -> bool {
        if self.quit_on_close || self.tray.is_none() {
            return true;
        }
        match self.over {
            true => self.layer.read(cx).leave(),
            false => tray::hide(hwnd),
        }
        false
    }

    fn sync_tray(&mut self, _: &mut Context<Self>) {
        let Some(tray) = &self.tray else { return };
        let playing = self.play.live().and_then(|id| self.games().iter().find(|game| game.id == *id)).map(|game| game.name.to_string());
        tray.set_menu(Menu { playing, over: self.over });
    }
}
