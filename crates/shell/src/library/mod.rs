//! The game list: the source's catalog joined with the installed games and the executables the user picked,
//! and the game open in the page.

mod picker;
mod sidebar;
mod source_menu;

use std::{collections::HashSet, path::PathBuf, sync::Arc};

use blocking::unblock;
use client::{
    Registry,
    library::{self as listing, Game, GameId, Links, Picked, sources},
    storage::Paths,
};
use futures::StreamExt;
use gpui_kit::{AsyncApp, Context, Entity, EventEmitter, Task, WeakEntity};
use model::{Ref, Snapshot};

use crate::{Load, settings::Settings};
pub use picker::GamePicker;
pub use sidebar::{Marks, Sidebar, WIDTH as SIDEBAR_WIDTH, vacant};
pub use source_menu::{pick_source, source_options};

pub enum LibraryEvent {
    Listed,
    /// Another game is open: the user's pick, or the first when the open one left the list.
    Opened,
    /// A game's install changed, and with it maybe its trainer.
    Switched,
    /// The user linked a game to a title for its maps, or took the link away.
    Linked,
    /// The user chose another record's trainer for the game, or its own again.
    Retrained(GameId),
}

/// What `+ ADD` found for an executable.
pub struct Found {
    pub exe: PathBuf,
    /// The catalog: the list is scanned again with it.
    pub games: Arc<Vec<model::Game>>,
    /// Indices into `games` of those that keep their executable where `exe` is.
    pub owners: Vec<usize>,
}

pub struct Library {
    registry: Arc<Registry>,
    settings: Entity<Settings>,
    paths: Paths,
    picked: Picked,
    links: Links,
    trainer_links: Links,
    list: Load<listing::Library>,
    /// The listed game open, kept in the prefs.
    open: Option<GameId>,
    /// A catalog game open in its place, not installed.
    browsed: Option<Arc<Game>>,
    /// The record of the browse under way, until it shows, and whether it opens (else it refreshes the one shown): a
    /// list made meanwhile asks again with its catalog.
    wanted: Option<(Ref, bool)>,
    // Replaced by the next browse, dropped by an open: a late one never lands.
    _browsing: Option<Task<()>>,
    // Dropped by a reload.
    _listing: Task<()>,
}

impl EventEmitter<LibraryEvent> for Library {}

impl Library {
    pub fn new(registry: Arc<Registry>, paths: &Paths, settings: Entity<Settings>, cx: &mut Context<Self>) -> Self {
        // Opens once the list holds it.
        let open = settings.read(cx).prefs().game.clone();
        Self {
            _listing: listen(&registry, cx),
            registry,
            settings,
            paths: paths.clone(),
            picked: Picked::load(paths),
            links: Links::maps(paths),
            trainer_links: Links::trainers(paths),
            list: Load::Pending,
            open,
            browsed: None,
            wanted: None,
            _browsing: None,
        }
    }

    /// Lists anew after the source's caches were cleared; a browsed game goes with its catalog.
    pub fn reload(&mut self, cx: &mut Context<Self>) {
        self.list = Load::Pending;
        self._listing = listen(&self.registry, cx);
        if self.stop_browsing() {
            cx.emit(LibraryEvent::Opened);
        }
        cx.notify();
    }

    /// Drops the browsed game and the browse under way; `true` when one was shown.
    fn stop_browsing(&mut self) -> bool {
        (self.wanted, self._browsing) = (None, None);
        self.browsed.take().is_some()
    }

    pub fn list(&self) -> &Load<listing::Library> {
        &self.list
    }

    pub fn games(&self) -> &[Arc<Game>] {
        match &self.list {
            Load::Ready(list) => &list.games,
            _ => &[],
        }
    }

    pub fn game(&self, id: &GameId) -> Option<&Arc<Game>> {
        self.games().iter().find(|game| game.id == *id)
    }

    /// The catalog records of `ids`, listed or not, in one pass and catalog order.
    pub fn records(&self, ids: &[GameId]) -> Vec<&model::Game> {
        let Load::Ready(list) = &self.list else { return Vec::new() };
        let ids: HashSet<Ref> = ids.iter().filter_map(GameId::game).collect();
        list.catalog.iter().filter(|record| ids.contains(&record.id)).collect()
    }

    /// The open game's id; before the list comes, the one open at the last start.
    pub fn open_id(&self) -> Option<&GameId> {
        self.browsed.as_ref().map(|game| &game.id).or(self.open.as_ref())
    }

    /// The open game: a listed one, or a catalog game the user browses.
    pub fn opened(&self) -> Option<&Arc<Game>> {
        self.browsed.as_ref().or_else(|| self.game(self.open.as_ref()?))
    }

    /// The user opens a listed game: it is open at the next start too.
    pub fn open(&mut self, id: GameId, cx: &mut Context<Self>) {
        self.stop_browsing();
        self.keep_open(id, cx);
        cx.emit(LibraryEvent::Opened);
        cx.notify();
    }

    fn keep_open(&mut self, id: GameId, cx: &mut Context<Self>) {
        self.settings.update(cx, |settings, cx| settings.edit(|prefs| prefs.game = Some(id.clone()), cx));
        self.open = Some(id);
    }

    /// Opens the catalog record `record`, a listed game as it is. Not kept: the next start opens the last listed game.
    pub fn browse(&mut self, record: Ref, cx: &mut Context<Self>) {
        let id = GameId::of(&record);
        if self.game(&id).is_some() {
            return self.open(id, cx);
        }
        self.load_browsed(record, true, cx);
    }

    /// Makes the catalog game `record` off the UI thread, then shows it: `Opened` when it `opens`, else `Switched` (a
    /// new copy of the one shown); one gone from the catalog gives the listed game back. Only the latest lands: an
    /// open, another browse or a list of a new catalog drops it.
    fn load_browsed(&mut self, record: Ref, opens: bool, cx: &mut Context<Self>) {
        let Load::Ready(list) = &self.list else { return };
        let (catalog, paths, asked) = (list.catalog.clone(), self.paths.clone(), record.clone());
        self.wanted = Some((record, opens));
        self._browsing = Some(cx.spawn(async move |library, cx| {
            let game = unblock(move || listing::browse(&catalog, &asked, &paths)).await;
            library
                .update(cx, |library, cx| {
                    let event = if opens || game.is_none() { LibraryEvent::Opened } else { LibraryEvent::Switched };
                    (library.browsed, library.wanted) = (game.map(Arc::new), None);
                    cx.emit(event);
                    cx.notify();
                })
                .ok();
        }));
    }

    /// Uses the game's other install.
    pub fn switch(&mut self, id: &GameId, cx: &mut Context<Self>) {
        let Some(game) = self.game_mut(id) else { return };
        game.switch();
        let used = game.install.as_ref().and_then(sources::exe).is_some();
        self.picked.set_used(id, used);
        switched(cx);
    }

    /// The title the user linked `id` to, whether its source is there or not.
    pub fn link(&self, id: &GameId) -> Option<Ref> {
        self.links.get(id)
    }

    pub fn set_link(&mut self, id: &GameId, title: &Ref, cx: &mut Context<Self>) {
        self.links.set(id, title);
        cx.emit(LibraryEvent::Linked);
        cx.notify();
    }

    /// The user takes the link away: the game's maps are matched by store id again.
    pub fn unlink(&mut self, id: &GameId, cx: &mut Context<Self>) {
        if self.links.clear(id) {
            cx.emit(LibraryEvent::Linked);
            cx.notify();
        }
    }

    /// The user's choice of the catalog record whose trainer `id` uses; `None` or its own record is its own.
    pub fn set_trainer(&mut self, id: &GameId, record: Option<Ref>, cx: &mut Context<Self>) {
        // A list replaced meanwhile may have let the game go.
        let Some((own, was)) = self.game(id).map(|game| (game.source.id.clone(), game.stamp.trainer.clone())) else { return };
        match record.filter(|record| *record != own) {
            Some(record) => self.trainer_links.set(id, &record),
            None => drop(self.trainer_links.clear(id)),
        }
        if let Load::Ready(list) = &mut self.list {
            list.relink(|game| self.trainer_links.get(game));
        }
        // The trainer in use picked again reloads nothing.
        if self.game(id).is_some_and(|game| game.stamp.trainer != was) {
            cx.emit(LibraryEvent::Retrained(id.clone()));
        }
        cx.notify();
    }

    /// Makes `exe` the source of a listed game.
    pub fn pick(&mut self, id: GameId, exe: PathBuf, cx: &mut Context<Self>) {
        let Some(game) = self.game(&id) else { return };
        let source = game.source.clone();
        cx.spawn(async move |library, cx| {
            let Some((install, exe)) = unblock(move || sources::install(&source, &exe).map(|install| (install, exe))).await else { return };
            library
                .update(cx, |library, cx| {
                    library.picked.set(&id, Some(exe));
                    if let Some(game) = library.game_mut(&id) {
                        game.pick(install);
                    }
                    switched(cx);
                })
                .ok();
        })
        .detach();
    }

    /// Forgets the game's picked executable; a game the user added leaves the list with it.
    pub fn forget(&mut self, id: &GameId, cx: &mut Context<Self>) {
        self.picked.set(id, None);
        let Some(game) = self.game_mut(id) else { return };
        if game.unpick() {
            return switched(cx);
        }
        let Load::Ready(list) = &self.list else { return };
        // The same catalog: a browse under way goes on.
        let games = list.games.iter().filter(|game| game.id != *id).cloned().collect();
        let list = listing::Library { games, trainers: list.trainers, catalog: list.catalog.clone() };
        self.set(list, cx);
    }

    /// The catalog and the games `exe` may belong to.
    pub fn find(&self, exe: PathBuf, cx: &mut Context<Self>) -> Task<Option<Found>> {
        let registry = self.registry.clone();
        cx.spawn(async move |_, _| {
            let Some(games) = catalog(registry.as_ref()).await else {
                log::warn!("no catalog to add {} from", exe.display());
                return None;
            };
            let found = unblock(move || {
                let owners = sources::owners(&games, &exe);
                Found { exe, games: Arc::new(games), owners }
            });
            Some(found.await)
        })
    }

    /// Adds `games[at]` with its executable and opens it; a listed game takes it as its source.
    pub fn add(&mut self, exe: PathBuf, games: Arc<Vec<model::Game>>, at: usize, cx: &mut Context<Self>) {
        let id = GameId::of(&games[at].id);
        if self.game(&id).is_some() {
            self.pick(id.clone(), exe, cx);
            return self.open(id, cx);
        }
        self.picked.set(&id, Some(exe));
        cx.spawn(async move |library, cx| {
            rescan(&library, games, cx).await;
            library
                .update(cx, |library, cx| {
                    // Gone if the file went meanwhile.
                    if library.game(&id).is_some() {
                        library.open(id, cx);
                    }
                })
                .ok();
        })
        .detach();
    }

    /// Adds `exe` as the catalog record `record` of the list shown now: a list made meanwhile may hold it elsewhere.
    pub fn add_as(&mut self, exe: PathBuf, record: &Ref, cx: &mut Context<Self>) {
        let Load::Ready(list) = &self.list else { return };
        let Some(at) = list.catalog.iter().position(|game| game.id == *record) else { return };
        let games = list.catalog.clone();
        self.add(exe, games, at, cx);
    }

    fn game_mut(&mut self, id: &GameId) -> Option<&mut Game> {
        let Load::Ready(list) = &mut self.list else { return None };
        list.games.iter_mut().find(|game| game.id == *id).map(Arc::make_mut)
    }

    /// Shows `list` with the trainers chosen now: a scan that ran meanwhile has the old choice.
    /// The catalog game asked for last, else the one shown, is open as listed once installed, else made again from a
    /// new catalog.
    fn set(&mut self, mut list: listing::Library, cx: &mut Context<Self>) {
        list.relink(|game| self.trainer_links.get(game));
        let was = self.open_id().cloned();
        let has = |id: &GameId| list.games.iter().any(|game| game.id == *id);
        let target = self.wanted.clone().or_else(|| self.browsed.as_ref().map(|game| (game.source.id.clone(), false)));
        let installed = target.as_ref().map(|(record, _)| GameId::of(record)).filter(|id| has(id));
        let opened = installed.is_some();
        if let Some(id) = installed {
            self.stop_browsing();
            self.keep_open(id, cx);
        }
        if !self.open.as_ref().is_some_and(has) {
            self.open = list.games.first().map(|game| game.id.clone());
        }
        let new_catalog = match &self.list {
            Load::Ready(old) => !Arc::ptr_eq(&old.catalog, &list.catalog),
            _ => true,
        };
        self.list = Load::Ready(list);
        if let Some((record, opens)) = target.filter(|_| !opened && new_catalog) {
            self.load_browsed(record, opens, cx);
        }
        if opened || self.open_id() != was.as_ref() {
            cx.emit(LibraryEvent::Opened);
        }
        cx.emit(LibraryEvent::Listed);
        cx.notify();
    }
}

fn switched(cx: &mut Context<Library>) {
    cx.emit(LibraryEvent::Switched);
    cx.notify();
}

fn listen(registry: &Arc<Registry>, cx: &mut Context<Library>) -> Task<()> {
    let mut games = registry.games();
    cx.spawn(async move |library, cx| {
        let mut have_copy = false;
        while let Some(snapshot) = games.next().await {
            match snapshot {
                Snapshot::Cached(Some(all)) | Snapshot::Fresh(Ok(all)) => {
                    have_copy = true;
                    rescan(&library, Arc::new(all), cx).await;
                }
                Snapshot::Fresh(Err(e)) if !have_copy => {
                    library
                        .update(cx, |library, cx| {
                            library.list = Load::Failed(e);
                            cx.notify();
                        })
                        .ok();
                }
                _ => {}
            }
        }
    })
}

/// Lists the installed games of `games` anew, with the executables the user picked. A first list shows the last
/// scan's installs at once: reading the launchers takes minutes on a cold disk with hundreds of games.
async fn rescan(library: &WeakEntity<Library>, mut games: Arc<Vec<model::Game>>, cx: &mut AsyncApp) {
    loop {
        let Ok((picked, paths, registry, first)) = library.read_with(cx, |library, _| {
            let first = matches!(library.list, Load::Pending);
            (library.picked.clone(), library.paths.clone(), library.registry.clone(), first)
        }) else {
            return;
        };
        let version = picked.version();
        if first {
            let (all, picked, paths) = (games.clone(), picked.clone(), paths.clone());
            if let Some(list) = unblock(move || listing::kept(&all, &picked, &paths)).await {
                library
                    .update(cx, |library, cx| {
                        // Another scan's list may have come meanwhile: it is newer.
                        if matches!(library.list, Load::Pending) && library.picked.version() == version {
                            library.set(list, cx);
                        }
                    })
                    .ok();
            }
        }
        let all = games.clone();
        let list = unblock(move || listing::scan(&all, &picked, &paths)).await;
        if show(library, list, version, cx) != Some(false) {
            return;
        }
        // A source changed while it scanned: again, with it.
        let Some(all) = catalog(registry.as_ref()).await else { return };
        games = Arc::new(all);
    }
}

/// Shows `list` unless the picked executables changed while it was made: `Some(false)`.
fn show(library: &WeakEntity<Library>, list: listing::Library, version: u64, cx: &mut AsyncApp) -> Option<bool> {
    let shown = library.update(cx, |library, cx| {
        let current = library.picked.version() == version;
        if current {
            library.set(list, cx);
        }
        current
    });
    shown.ok()
}

/// The source's games, the disk copy when there is one.
async fn catalog(registry: &Registry) -> Option<Vec<model::Game>> {
    let mut snapshots = registry.games();
    while let Some(snapshot) = snapshots.next().await {
        match snapshot {
            Snapshot::Cached(Some(all)) | Snapshot::Fresh(Ok(all)) => return Some(all),
            Snapshot::Fresh(Err(_)) => return None,
            Snapshot::Cached(None) => {}
        }
    }
    None
}
