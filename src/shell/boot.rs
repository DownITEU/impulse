//! Fills the page: disk copies first, the network only for what the open game lacks or holds stale.

use std::{path::PathBuf, sync::Arc};

use blocking::unblock;
use gpui_kit::{AsyncWindowContext, Context, WeakEntity, Window};

use super::{
    Load, Page, art,
    library::{self, Library, Support},
    loaded::Loaded,
};
use crate::{
    catalog::{self, Catalog},
    play::binary,
    session::{self, Source, account},
    trainer,
};

impl Page {
    pub(super) fn boot(&self, window: &Window, cx: &mut Context<Self>) {
        let dir = self.data_dir.clone();
        cx.spawn_in(window, async move |page, cx| {
            let cached = {
                let dir = dir.clone();
                unblock(move || catalog::cached(&dir)).await
            };
            let have_copy = cached.is_some();
            let art = art::cache_dir(&dir);
            if let Some(catalog) = cached {
                rescan(&page, catalog, art.clone(), cx).await;
            }
            match unblock(move || catalog::refresh(&dir, have_copy)).await {
                Ok(Some(catalog)) => {
                    log::info!("catalog updated");
                    rescan(&page, catalog, art, cx).await;
                }
                Ok(None) => log::debug!("catalog is up to date"),
                Err(e) => {
                    log::warn!("catalog not refreshed: {e:#}");
                    if !have_copy {
                        page.update(cx, |page, cx| {
                            page.library = Load::Failed(format!("{e:#}"));
                            cx.notify();
                        })
                        .ok();
                    }
                }
            }
        })
        .detach();

        let Some(transport) = self.transport.clone() else {
            return;
        };
        let dir = self.data_dir.clone();
        cx.spawn_in(window, async move |page, cx| {
            let cached = {
                let dir = dir.clone();
                unblock(move || account::cached(&dir)).await
            };
            let known = cached.as_ref().is_some_and(|(_, fresh)| *fresh);
            if let Some((account, _)) = cached {
                page.update(cx, |page, cx| {
                    page.account = Some(account);
                    cx.notify();
                })
                .ok();
            }
            let session = match session::connect(&transport, dir.join("session.bin")).await {
                Ok((session, source)) => {
                    let how = match source {
                        Source::Restored => "restored",
                        Source::Refreshed => "refreshed",
                        Source::Wand => "new from Wand",
                    };
                    let days = (session.expires_in().as_secs() + 43_200) / 86_400;
                    log::info!("Session: {how}, expires in {days} d");
                    Arc::new(session)
                }
                Err(e) => {
                    log::error!("Session: {e:#}");
                    page.update(cx, |page, cx| {
                        page.session = Load::Failed(format!("{e:#}"));
                        cx.notify();
                    })
                    .ok();
                    return;
                }
            };
            page.update_in(cx, |page, window, cx| {
                page.session = Load::Ready(session.clone());
                page.open_trainer(window, cx);
            })
            .ok();
            if known {
                return;
            }
            match account::fetch(&session, &transport, &dir).await {
                Ok(account) => {
                    page.update(cx, |page, cx| {
                        page.account = Some(account);
                        cx.notify();
                    })
                    .ok();
                }
                Err(e) => log::warn!("account: {e:#}"),
            }
        })
        .detach();
    }

    /// Drops every disk copy and fills the page anew. The session stays: it is not a cache.
    pub(super) fn invalidate_caches(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let dir = self.data_dir.clone();
        cx.spawn_in(window, async move |page, cx| {
            unblock(move || {
                catalog::forget(&dir);
                trainer::forget(&dir);
                binary::forget(&dir);
                account::forget(&dir);
                art::forget(&art::cache_dir(&dir));
            })
            .await;
            log::info!("caches invalidated");
            page.update_in(cx, |page, window, cx| {
                page.library = Load::Pending;
                page.trainers.clear();
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

    /// Brings the open game's trainer: the disk copy at once, then the API only when that
    /// copy is missing or stale. Nothing is downloaded for a game the user did not open.
    pub(super) fn open_trainer(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(game) = self.game() else {
            return;
        };
        let held = match self.trainers.get(&game.id) {
            Some(Load::Ready(loaded)) => Some(loaded.stamp == game.stamp),
            _ => None,
        };
        if game.support == Support::NoTrainer || held == Some(true) {
            return;
        }
        let (id, stamp) = (game.id.clone(), game.stamp.clone());
        // One task per game: a second open while it runs changes nothing.
        if !self.loading.insert(id.clone()) {
            return;
        }
        let dir = self.data_dir.clone();
        cx.spawn_in(window, async move |page, cx| {
            let mut fresh = false;
            if held.is_none() {
                let copy = {
                    let (dir, id) = (dir.clone(), id.clone());
                    unblock(move || trainer::cached(&dir, &id)).await
                };
                if let Some((trainer, found)) = copy {
                    fresh = found == stamp;
                    page.update_in(cx, |page, window, cx| {
                        let loaded = Loaded::new(trainer, found, window, cx);
                        page.install(id.clone(), loaded);
                        cx.notify();
                    })
                    .ok();
                }
            }
            // Asked now, not at the start: the session may have connected meanwhile.
            let link = page.update(cx, |page, cx| {
                let (Load::Ready(session), Some(transport)) = (&page.session, &page.transport)
                else {
                    return None;
                };
                let link = (!fresh).then(|| (session.clone(), transport.clone()))?;
                page.trainers.entry(id.clone()).or_insert(Load::Pending);
                cx.notify();
                Some(link)
            });
            let result = match link {
                Ok(Some((session, transport))) => {
                    Some(trainer::fetch(&session, &transport, &dir, &id, stamp.clone()).await)
                }
                _ => None,
            };
            page.update_in(cx, |page, window, cx| {
                match result {
                    Some(Ok(trainer)) => {
                        let loaded = Loaded::new(trainer, stamp, window, cx);
                        page.install(id.clone(), loaded);
                    }
                    Some(Err(e)) => {
                        log::warn!("trainer {id}: {e:#}");
                        // A stale copy still beats an error.
                        if !matches!(page.trainers.get(&id), Some(Load::Ready(_))) {
                            page.trainers
                                .insert(id.clone(), Load::Failed(format!("{e:#}")));
                        }
                    }
                    None => {}
                }
                page.loading.remove(&id);
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    // A newer copy may order the cheats differently.
    fn install(&mut self, id: Arc<str>, loaded: Loaded) {
        if self.selected.as_ref() == Some(&id) {
            self.drop_cheat_state();
        }
        self.trainers.insert(id, Load::Ready(loaded));
    }
}

async fn rescan(
    page: &WeakEntity<Page>,
    catalog: Catalog,
    art: PathBuf,
    cx: &mut AsyncWindowContext,
) {
    let library = unblock(move || library::scan(&catalog, &art)).await;
    page.update_in(cx, |page, window, cx| page.set_library(library, window, cx))
        .ok();
}
