//! Impulse in the notification area. With close to tray on (the default) the close button hides the window
//! there; the icon opens it, and its menu also stops the running trainer, toggles the overlay and quits,
//! stopping a live trainer first.

use std::time::Duration;

use futures::{StreamExt, channel::mpsc};
use gpui_kit::{Context, Window};
use tray::{Event, Menu, Tray};

use super::{Page, hwnd, run::Play};

// Quitting waits this long at most for a live trainer to stop.
const STOP_WAIT: Duration = Duration::from_secs(5);

impl Page {
    /// The icon, and the close button wired to it; `None` when the notification area refused it.
    pub(super) fn start_tray(window: &mut Window, cx: &mut Context<Self>) -> Option<Tray> {
        let hwnd = hwnd(window)?;
        // Without the icon too: a close that quits stops a live trainer first.
        let page = cx.weak_entity();
        window.on_window_should_close(cx, move |_, cx| page.update(cx, |page, cx| page.may_close(hwnd, cx)).unwrap_or(true));
        let (events, mut received) = mpsc::unbounded();
        let tray = Tray::new("Impulse", move |event| drop(events.unbounded_send(event))).inspect_err(|e| log::warn!("tray: {e}")).ok()?;
        cx.spawn_in(window, async move |page, cx| {
            while let Some(event) = received.next().await {
                page.update(cx, |page, cx| page.tray_event(event, hwnd, cx)).ok();
            }
        })
        .detach();
        // Any change may be the run's or the overlay's.
        cx.observe_self(Self::sync_tray).detach();
        Some(tray)
    }

    fn tray_event(&mut self, event: Event, hwnd: isize, cx: &mut Context<Self>) {
        match event {
            Event::Open => tray::show(hwnd),
            Event::Stop => self.stop_run(cx),
            Event::Overlay => self.layer.read(cx).toggle(),
            Event::Quit => self.quit(hwnd, cx),
        }
    }

    /// A live trainer is stopped as STOP stops it first, so its mods do not stay on in the game; meanwhile
    /// Impulse is gone from the screen and the tray.
    fn quit(&mut self, hwnd: isize, cx: &mut Context<Self>) {
        // Quitting skips drops: the icon would stay until a hover.
        self.tray = None;
        if self.play.is_active() {
            self.stop_run(cx);
        }
        if !matches!(self.play, Play::Stopping { .. }) {
            return cx.quit();
        }
        // `run_stopped` quits.
        self.quitting = true;
        tray::hide(hwnd);
        cx.spawn(async move |_, cx| {
            cx.background_executor().timer(STOP_WAIT).await;
            cx.update(|cx| cx.quit());
        })
        .detach();
    }

    /// `false` keeps the window: hidden in the tray, or back from over a game, where the overlay has the input.
    fn may_close(&mut self, hwnd: isize, cx: &mut Context<Self>) -> bool {
        if self.settings.read(cx).prefs().quit_on_close || self.tray.is_none() {
            if !self.play.is_active() {
                return true;
            }
            self.quit(hwnd, cx);
            return false;
        }
        match self.over {
            true => self.layer.read(cx).leave(),
            false => tray::hide(hwnd),
        }
        false
    }

    fn sync_tray(&mut self, cx: &mut Context<Self>) {
        let Some(tray) = &self.tray else { return };
        let playing = self
            .play
            .live()
            .and_then(|id| self.library.read(cx).games().iter().find(|game| game.id == *id))
            .map(|game| game.name.to_string());
        tray.set_menu(Menu { playing, over: self.over });
    }
}
