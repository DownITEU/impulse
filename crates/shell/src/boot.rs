//! Fills the page from the provider: disk copies first, the network only for what the open game lacks or holds stale.

use std::{path::PathBuf, sync::Arc};

use blocking::unblock;
use futures::StreamExt;
use gpui_kit::{AsyncWindowContext, Context, WeakEntity, Window};
use model::{SignIn, Snapshot};

use super::{
    Load, Page, art,
    library::{self, Library, Support},
    loaded::Loaded,
};

impl Page {
    pub(super) fn boot(&self, window: &Window, cx: &mut Context<Self>) {
        let (mut games, art) = (self.provider.games(), art::cache_dir(&self.data_dir));
        cx.spawn_in(window, async move |page, cx| {
            let mut have_copy = false;
            while let Some(snapshot) = games.next().await {
                match snapshot {
                    Snapshot::Cached(Some(all)) | Snapshot::Fresh(Ok(all)) => {
                        have_copy = true;
                        rescan(&page, all, art.clone(), cx).await;
                    }
                    Snapshot::Fresh(Err(e)) if !have_copy => {
                        page.update(cx, |page, cx| {
                            page.library = Load::Failed(format!("{e:#}"));
                            cx.notify();
                        })
                        .ok();
                    }
                    _ => {}
                }
            }
        })
        .detach();

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

    /// Drops every disk copy and fills the page anew. The session stays: it is not a cache.
    pub(super) fn invalidate_caches(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let (provider, art) = (self.provider.clone(), art::cache_dir(&self.data_dir));
        cx.spawn_in(window, async move |page, cx| {
            unblock(move || {
                provider.forget_caches();
                art::forget(&art);
            })
            .await;
            log::info!("caches invalidated");
            page.update_in(cx, |page, window, cx| {
                page.library = Load::Pending;
                // `boot` asks for the session again.
                page.signed_out = false;
                page.trainers.clear();
                page.deferred = None;
                page.stop_run(cx);
                page.boot(window, cx);
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn set_library(&mut self, library: Library, window: &mut Window, cx: &mut Context<Self>) {
        let listed = |id: &Option<Arc<str>>| {
            id.as_ref()
                .is_some_and(|id| library.games.iter().any(|game| game.id == *id))
        };
        if !listed(&self.selected) {
            self.selected = library.games.first().map(|game| game.id.clone());
            self.drop_cheat_state();
        }
        if !listed(&self.play.game().cloned()) {
            self.stop_run(cx);
        }
        self.library = Load::Ready(library);
        self.open_trainer(window, cx);
        cx.notify();
    }

    /// Brings the open game's trainer. Nothing is downloaded for a game the user did not open.
    pub(super) fn open_trainer(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(game) = self.game() else {
            return;
        };
        let current = matches!(self.trainers.get(&game.id), Some(Load::Ready(loaded)) if !loaded.stale && loaded.stamp == game.stamp);
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
                            let loaded = Loaded::new(trainer, hotkeys, stamp.clone(), window, cx);
                            page.install(id.clone(), loaded);
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
            page.update(cx, |page, cx| {
                page.loading.remove(&id);
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// A stale copy still beats an error, and the next open asks again.
    fn refresh_failed(&mut self, id: &Arc<str>, error: String) {
        if let Some((_, loaded)) = self.deferred.as_mut().filter(|(held, _)| held == id) {
            loaded.stale = true;
        }
        match self.trainers.get_mut(id) {
            Some(Load::Ready(loaded)) => loaded.stale = true,
            _ => {
                self.trainers.insert(id.clone(), Load::Failed(error));
            }
        }
    }

    // A newer copy may order the cheats differently.
    fn install(&mut self, id: Arc<str>, loaded: Loaded) {
        // A run keeps the cheat list it started with: its variables and values belong to it.
        if self.play.holds(&id) {
            self.deferred = Some((id, loaded));
            return;
        }
        if self.selected.as_ref() == Some(&id) {
            self.drop_cheat_state();
            // Its cap may be gone, and with it the click that ends the capture.
            self.bindings.cancel();
        }
        self.trainers.insert(id, Load::Ready(loaded));
    }

    /// For when a run lets go of its cheat list.
    pub(super) fn install_deferred(&mut self) {
        if let Some((id, mut loaded)) = self.deferred.take() {
            // The user may have rebound a slot while it waited.
            for (index, slots) in self.bindings.slots(&id, &loaded.cheats).into_iter().enumerate() {
                loaded.rebind(index, slots);
            }
            self.install(id, loaded);
        }
    }
}

async fn rescan(page: &WeakEntity<Page>, games: Vec<model::Game>, art: PathBuf, cx: &mut AsyncWindowContext) {
    let library = unblock(move || library::scan(games, &art)).await;
    page.update_in(cx, |page, window, cx| page.set_library(library, window, cx))
        .ok();
}
