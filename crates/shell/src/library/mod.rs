//! The game list: the source's catalog joined with the installed games and the executables the user picked,
//! and the game open in the page.

mod picker;
mod sidebar;
mod source_menu;

use std::{mem, path::PathBuf, sync::Arc};

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
}

/// What `+ ADD` found for an executable.
pub struct Found {
    pub exe: PathBuf,
    /// The catalog: the list is scanned again with it.
    pub games: Vec<model::Game>,
    /// Lower case, for the filter.
    pub names: Vec<String>,
    /// Indices into `games` of those that keep their executable where `exe` is.
    pub owners: Vec<usize>,
}

pub struct Library {
    registry: Arc<Registry>,
    settings: Entity<Settings>,
    paths: Paths,
    picked: Picked,
    links: Links,
    list: Load<listing::Library>,
    open: Option<GameId>,
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
            links: Links::load(paths),
            list: Load::Pending,
            open,
        }
    }

    /// Lists anew after the source's caches were cleared.
    pub fn reload(&mut self, cx: &mut Context<Self>) {
        self.list = Load::Pending;
        self._listing = listen(&self.registry, cx);
        cx.notify();
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

    /// The open game's id; before the list comes, the one open at the last start.
    pub fn open_id(&self) -> Option<&GameId> {
        self.open.as_ref()
    }

    pub fn opened(&self) -> Option<&Arc<Game>> {
        self.game(self.open.as_ref()?)
    }

    /// The user opens a game: it is open at the next start too.
    pub fn open(&mut self, id: GameId, cx: &mut Context<Self>) {
        self.settings.update(cx, |settings, cx| settings.edit(|prefs| prefs.game = Some(id.clone()), cx));
        self.open = Some(id);
        cx.emit(LibraryEvent::Opened);
        cx.notify();
    }

    /// Uses the game's other install.
    pub fn switch(&mut self, id: &GameId, cx: &mut Context<Self>) {
        let Some(game) = self.game_mut(id) else { return };
        game.switch();
        let used = sources::exe(&game.install).is_some();
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
        if let Load::Ready(mut list) = mem::replace(&mut self.list, Load::Pending) {
            list.games.retain(|game| game.id != *id);
            self.set(list, cx);
        }
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
                let names = games.iter().map(|game| game.name.to_lowercase()).collect();
                Found { exe, games, names, owners }
            });
            Some(found.await)
        })
    }

    /// Adds `games[at]` with its executable and opens it; a listed game takes it as its source.
    pub fn add(&mut self, exe: PathBuf, games: Vec<model::Game>, at: usize, cx: &mut Context<Self>) {
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

    fn game_mut(&mut self, id: &GameId) -> Option<&mut Game> {
        let Load::Ready(list) = &mut self.list else { return None };
        list.games.iter_mut().find(|game| game.id == *id).map(Arc::make_mut)
    }

    fn set(&mut self, list: listing::Library, cx: &mut Context<Self>) {
        let listed = self.open.as_ref().is_some_and(|id| list.games.iter().any(|game| game.id == *id));
        if !listed {
            self.open = list.games.first().map(|game| game.id.clone());
        }
        self.list = Load::Ready(list);
        if !listed {
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
                    rescan(&library, all, cx).await;
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
async fn rescan(library: &WeakEntity<Library>, games: Vec<model::Game>, cx: &mut AsyncApp) {
    let mut games = Arc::new(games);
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
