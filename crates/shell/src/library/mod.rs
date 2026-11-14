//! The game list: the provider's catalog joined with the installed games and the executables the user picked,
//! and the game open in the page.

mod picker;
mod sidebar;
mod source_menu;

use std::{mem, path::PathBuf, sync::Arc};

use blocking::unblock;
use client::{
    library::{self as listing, Game, GameId, Picked, sources},
    media::Media,
    storage::Paths,
};
use futures::StreamExt;
use gpui_kit::{AsyncApp, Context, Entity, EventEmitter, Task, WeakEntity};
use model::{Provider, Snapshot};

use crate::{Load, settings::Settings};
pub use picker::GamePicker;
pub use sidebar::{Marks, Sidebar, WIDTH as SIDEBAR_WIDTH, vacant};
pub use source_menu::{pick_source, source_options};

pub enum LibraryEvent {
    /// A new list.
    Listed,
    /// Another game is open: the user's pick, or the first when the open one left the list.
    Opened,
    /// A game's install changed, and with it maybe its trainer.
    Switched,
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
    provider: Arc<dyn Provider>,
    settings: Entity<Settings>,
    art: PathBuf,
    picked: Picked,
    list: Load<listing::Library>,
    open: Option<GameId>,
    // Dropped by a reload.
    _listing: Task<()>,
}

impl EventEmitter<LibraryEvent> for Library {}

impl Library {
    pub fn new(provider: Arc<dyn Provider>, paths: &Paths, settings: Entity<Settings>, cx: &mut Context<Self>) -> Self {
        // Opens once the list holds it.
        let open = settings.read(cx).prefs().game.clone();
        Self {
            _listing: listen(&provider, cx),
            provider,
            settings,
            art: Media::Pictures.dir(paths),
            picked: Picked::load(paths),
            list: Load::Pending,
            open,
        }
    }

    /// Lists anew after the provider's caches were cleared.
    pub fn reload(&mut self, cx: &mut Context<Self>) {
        self.list = Load::Pending;
        self._listing = listen(&self.provider, cx);
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
        let provider = self.provider.clone();
        cx.spawn(async move |_, _| {
            let Some(games) = catalog(provider.as_ref()).await else {
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

fn listen(provider: &Arc<dyn Provider>, cx: &mut Context<Library>) -> Task<()> {
    let mut games = provider.games();
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
                            library.list = Load::Failed(format!("{e:#}"));
                            cx.notify();
                        })
                        .ok();
                }
                _ => {}
            }
        }
    })
}

/// Lists the installed games of `games` anew, with the executables the user picked.
async fn rescan(library: &WeakEntity<Library>, mut games: Vec<model::Game>, cx: &mut AsyncApp) {
    loop {
        let Ok((art, picked, provider)) =
            library.read_with(cx, |library, _| (library.art.clone(), library.picked.clone(), library.provider.clone()))
        else {
            return;
        };
        let version = picked.version();
        let list = unblock(move || listing::scan(games, &art, &picked)).await;
        let current = library.update(cx, |library, cx| {
            let current = library.picked.version() == version;
            if current {
                library.set(list, cx);
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
async fn catalog(provider: &dyn Provider) -> Option<Vec<model::Game>> {
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
