//! Fills the page from the provider: disk copies first, the network only for what the open game lacks or holds stale.

use std::sync::Arc;

use client::{library::Support, trainer::Sheet};
use futures::StreamExt;
use gpui_kit::{App, Context, Window};
use model::{SignIn, Snapshot};

use super::{Load, Page, loaded::Loaded};

impl Page {
    pub(super) fn sign_in(&self, window: &Window, cx: &mut Context<Self>) {
        let mut sign_in = self.provider.sign_in();
        cx.spawn_in(window, async move |page, cx| {
            while let Some(state) = sign_in.next().await {
                page.update(cx, |page, cx| {
                    match state {
                        SignIn::Account(account) => page.account = Some(account),
                        SignIn::Failed => page.signed_out = true,
                    }
                    cx.notify();
                })
                .ok();
            }
        })
        .detach();
    }

    /// Fills the page anew after a provider's store was cleared. The session stays: it is not a cache.
    pub(super) fn reload(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        log::info!("caches cleared");
        self.library.update(cx, |library, cx| library.reload(cx));
        // The session is asked for again.
        self.signed_out = false;
        // The run keeps what it leaves on first, from the trainers about to go.
        self.stop_run(cx);
        self.trainers.clear();
        self.deferred = None;
        self.sign_in(window, cx);
        cx.notify();
    }

    /// Brings the open game's trainer. Nothing is downloaded for a game the user did not open.
    pub(super) fn open_trainer(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(game) = self.game(cx) else {
            return;
        };
        let current =
            matches!(self.trainers.get(&game.id), Some(Load::Ready(loaded)) if !loaded.sheet.stale && loaded.sheet.stamp == game.stamp);
        if game.support == Support::NoTrainer || current {
            return;
        }
        let (id, stamp) = (game.id.clone(), game.stamp.clone());
        let mut snapshots = self.provider.trainer(&game.source, &game.install);
        // One task per game: a second open while it runs changes nothing.
        if !self.loading.insert(id.clone()) {
            return;
        }
        cx.spawn_in(window, async move |page, cx| {
            while let Some(snapshot) = snapshots.next().await {
                page.update_in(cx, |page, window, cx| {
                    match snapshot {
                        Snapshot::Cached(Some(trainer)) | Snapshot::Fresh(Ok(trainer)) => {
                            let hotkeys = page.bindings.slots(&id, &trainer.cheats);
                            let sheet = Sheet::new(trainer, hotkeys, stamp.clone(), page.profiles.of(&id));
                            let loaded = Loaded::new(sheet, window, cx);
                            page.install(id.clone(), loaded, cx);
                        }
                        // The network is next.
                        Snapshot::Cached(None) => {
                            page.trainers.entry(id.clone()).or_insert(Load::Pending);
                        }
                        Snapshot::Fresh(Err(e)) => page.refresh_failed(&id, format!("{e:#}")),
                    }
                    cx.notify();
                })
                .ok();
            }
            page.update_in(cx, |page, window, cx| {
                page.loading.remove(&id);
                // The source changed while it loaded: this trainer is for the old build.
                if page.game(cx).is_some_and(|game| game.id == id && game.stamp != stamp) {
                    page.open_trainer(window, cx);
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// A stale copy still beats an error, and the next open asks again.
    fn refresh_failed(&mut self, id: &Arc<str>, error: String) {
        if let Some((_, loaded)) = self.deferred.as_mut().filter(|(held, _)| held == id) {
            loaded.sheet.stale = true;
        }
        match self.trainers.get_mut(id) {
            Some(Load::Ready(loaded)) => loaded.sheet.stale = true,
            _ => {
                self.trainers.insert(id.clone(), Load::Failed(error));
            }
        }
    }

    // A newer copy may order the cheats differently.
    fn install(&mut self, id: Arc<str>, loaded: Loaded, cx: &App) {
        // A run keeps the cheat list it started with: its variables and values belong to it.
        if self.play.holds(&id) {
            self.deferred = Some((id, loaded));
            return;
        }
        if self.selected(cx).as_ref() == Some(&id) {
            self.drop_cheat_state();
            // Its cap may be gone, and with it the click that ends the capture.
            self.bindings.cancel();
        }
        self.trainers.insert(id, Load::Ready(loaded));
    }

    /// For when a run lets go of its cheat list.
    pub(super) fn install_deferred(&mut self, cx: &App) {
        if let Some((id, mut loaded)) = self.deferred.take() {
            // The user may have rebound a slot, pinned a mod or folded a section while it waited.
            for (index, slots) in self.bindings.slots(&id, &loaded.sheet.cheats).into_iter().enumerate() {
                loaded.rebind(index, slots);
            }
            loaded.sheet.restore(self.profiles.of(&id));
            self.install(id, loaded, cx);
        }
    }
}
