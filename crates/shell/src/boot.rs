//! Fills the page from the provider: disk copies first, the network only for what the open game lacks or holds stale.

use std::sync::Arc;

use blocking::unblock;
use futures::StreamExt;
use gpui_kit::{AsyncWindowContext, Context, WeakEntity, Window};
use model::{Provider, SignIn, Snapshot};

use super::{
    Load, Page, art,
    library::{self, Library, Support},
    loaded::Loaded,
};

impl Page {
    pub(super) fn boot(&self, window: &Window, cx: &mut Context<Self>) {
        let mut games = self.provider.games();
        cx.spawn_in(window, async move |page, cx| {
            let mut have_copy = false;
            while let Some(snapshot) = games.next().await {
                match snapshot {
                    Snapshot::Cached(Some(all)) | Snapshot::Fresh(Ok(all)) => {
                        have_copy = true;
                        rescan(&page, all, cx).await;
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

    /// Fills the page anew after a provider's store was cleared. The session stays: it is not a cache.
    pub(super) fn reload(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        log::info!("caches cleared");
        self.library = Load::Pending;
        // `boot` asks for the session again.
        self.signed_out = false;
        // The run keeps what it leaves on first, from the trainers about to go.
        self.stop_run(cx);
        self.trainers.clear();
        self.deferred = None;
        self.boot(window, cx);
        cx.notify();
    }

    pub(super) fn set_library(&mut self, library: Library, window: &mut Window, cx: &mut Context<Self>) {
        let listed = |id: &Option<Arc<str>>| id.as_ref().is_some_and(|id| library.games.iter().any(|game| game.id == *id));
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
                            let loaded = Loaded::new(trainer, hotkeys, stamp.clone(), page.profiles.of(&id), window, cx);
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
            page.update_in(cx, |page, window, cx| {
                page.loading.remove(&id);
                // The source changed while it loaded: this trainer is for the old build.
                if page.game().is_some_and(|game| game.id == id && game.stamp != stamp) {
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
            // The user may have rebound a slot, pinned a mod or folded a section while it waited.
            for (index, slots) in self.bindings.slots(&id, &loaded.cheats).into_iter().enumerate() {
                loaded.rebind(index, slots);
            }
            loaded.restore(self.profiles.of(&id));
            self.install(id, loaded);
        }
    }
}

/// Lists the installed games of `games` anew, with the executables the user picked.
pub(super) async fn rescan(page: &WeakEntity<Page>, mut games: Vec<model::Game>, cx: &mut AsyncWindowContext) {
    loop {
        let Ok((art, picked, provider)) =
            page.read_with(cx, |page, _| (art::Media::Pictures.dir(&page.data_dir), page.picked.clone(), page.provider.clone()))
        else {
            return;
        };
        let version = picked.version();
        let library = unblock(move || library::scan(games, &art, &picked)).await;
        let current = page.update_in(cx, |page, window, cx| {
            let current = page.picked.version() == version;
            if current {
                page.set_library(library, window, cx);
            }
            current
        });
        if !matches!(current, Ok(false)) {
            return;
        }
        // A source changed while it scanned: again, with it.
        let Some(all) = catalog(provider.as_ref()).await else { return };
        games = all;
    }
}

/// The provider's games, the disk copy when there is one.
pub(super) async fn catalog(provider: &dyn Provider) -> Option<Vec<model::Game>> {
    let mut snapshots = provider.games();
    while let Some(snapshot) = snapshots.next().await {
        match snapshot {
            Snapshot::Cached(Some(all)) | Snapshot::Fresh(Ok(all)) => return Some(all),
            Snapshot::Fresh(Err(_)) => return None,
            Snapshot::Cached(None) => {}
        }
    }
    None
}
