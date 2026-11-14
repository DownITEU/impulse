//! Impulse in the notification area. With close to tray on (the default) the close button hides the window
//! there; the icon opens it, and its menu also stops the running trainer, toggles the overlay and quits,
//! stopping a live trainer first.

use std::time::Duration;

use futures::{StreamExt, channel::mpsc};
use gpui_kit::{App, Context, Entity, Window};
use tray::{Event, Menu, Tray};

use super::{
    hwnd,
    layer::Layer,
    library::Library,
    run::{RunEvent, Runs},
    settings::Settings,
};

// Quitting waits this long at most for a live trainer to stop.
const STOP_WAIT: Duration = Duration::from_secs(5);

pub struct TrayIcon {
    /// `None` when the notification area refused it.
    tray: Option<Tray>,
    library: Entity<Library>,
    runs: Entity<Runs>,
    layer: Entity<Layer>,
    settings: Entity<Settings>,
    // Impulse ends once the trainer is stopped.
    quitting: bool,
}

impl TrayIcon {
    /// The icon, and the window's close button wired to it.
    pub fn new(
        library: Entity<Library>,
        runs: Entity<Runs>,
        layer: Entity<Layer>,
        settings: Entity<Settings>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut icon = Self { tray: None, library, runs, layer, settings, quitting: false };
        let Some(hwnd) = hwnd(window) else { return icon };
        // Without the icon too: a close that quits stops a live trainer first.
        let this = cx.weak_entity();
        window.on_window_should_close(cx, move |_, cx| this.update(cx, |icon, cx| icon.may_close(hwnd, cx)).unwrap_or(true));
        // Quitting skips drops: the icon would stay until a hover.
        cx.on_app_quit(|icon, _| {
            icon.tray = None;
            async {}
        })
        .detach();
        cx.subscribe(&icon.runs, |icon, _, event: &RunEvent, cx| {
            // The quit waited for the trainer to stop.
            if matches!(event, RunEvent::Ended) && icon.quitting {
                cx.quit();
            }
        })
        .detach();
        let (events, mut received) = mpsc::unbounded();
        icon.tray = Tray::new("Impulse", move |event| drop(events.unbounded_send(event))).inspect_err(|e| log::warn!("tray: {e}")).ok();
        if icon.tray.is_none() {
            return icon;
        }
        cx.spawn_in(window, async move |icon, cx| {
            while let Some(event) = received.next().await {
                icon.update(cx, |icon, cx| icon.clicked(event, hwnd, cx)).ok();
            }
        })
        .detach();
        // The menu names the live game and the overlay's state.
        cx.observe(&icon.runs, |icon, _, cx| icon.set_menu(cx)).detach();
        cx.observe(&icon.library, |icon, _, cx| icon.set_menu(cx)).detach();
        cx.observe(&icon.layer, |icon, _, cx| icon.set_menu(cx)).detach();
        icon
    }

    fn clicked(&mut self, event: Event, hwnd: isize, cx: &mut Context<Self>) {
        match event {
            Event::Open => tray::show(hwnd),
            Event::Stop => self.runs.update(cx, |runs, cx| runs.stop(cx)),
            Event::Overlay => self.layer.read(cx).toggle(),
            Event::Quit => self.quit(hwnd, cx),
        }
    }

    /// A live trainer is stopped as STOP stops it first, so its mods do not stay on in the game; meanwhile
    /// Impulse is gone from the screen and the tray.
    fn quit(&mut self, hwnd: isize, cx: &mut Context<Self>) {
        // Quitting skips drops: the icon would stay until a hover.
        self.tray = None;
        self.runs.update(cx, |runs, cx| runs.stop(cx));
        if !self.runs.read(cx).is_stopping() {
            return cx.quit();
        }
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
            if !self.runs.read(cx).is_active() {
                return true;
            }
            self.quit(hwnd, cx);
            return false;
        }
        match self.layer.read(cx).is_over() {
            true => self.layer.read(cx).leave(),
            false => tray::hide(hwnd),
        }
        false
    }

    fn set_menu(&self, cx: &App) {
        let Some(tray) = &self.tray else { return };
        let playing = self.runs.read(cx).live().and_then(|id| self.library.read(cx).game(id)).map(|game| game.name.to_string());
        tray.set_menu(Menu { playing, over: self.layer.read(cx).is_over() });
    }
}
