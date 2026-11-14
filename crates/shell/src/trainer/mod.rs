//! The trainers of the games opened in this run: the open game's loading, which copy shows while a run holds it,
//! and what the user keeps about a game (`games.json`).

mod title;

use std::{collections::HashSet, sync::Arc};

use client::{
    Load,
    library::Support,
    profile::{Collection, Profile, Profiles, Values},
    run,
    storage::Paths,
    trainer::{Sheet, Shelf},
};
use futures::{StreamExt, channel::oneshot};
use gpui_kit::{Context, Entity, EventEmitter};
use model::{Chord, Provider, Snapshot};

use crate::library::{Library, LibraryEvent};
pub use title::title;

pub enum TrainerEvent {
    /// A copy of the game's trainer shows: the first, a newer one, or the one that waited.
    Installed(Arc<str>),
    /// The game's hotkeys changed.
    Rebound(Arc<str>),
    /// A cheat's value changed.
    Set { game: Arc<str>, index: usize, value: f64 },
}

/// While it lives the game's shown copy stays; dropping it shows the newer one that waited.
pub struct Hold {
    _release: oneshot::Sender<()>,
}

pub struct Trainers {
    provider: Arc<dyn Provider>,
    library: Entity<Library>,
    profiles: Profiles,
    shelf: Shelf,
    // Games whose trainer is being read or downloaded right now.
    loading: HashSet<Arc<str>>,
}

impl EventEmitter<TrainerEvent> for Trainers {}

impl Trainers {
    pub fn new(provider: Arc<dyn Provider>, paths: &Paths, library: Entity<Library>, cx: &mut Context<Self>) -> Self {
        cx.subscribe(&library, |trainers, _, _: &LibraryEvent, cx| trainers.open(cx)).detach();
        Self { provider, library, profiles: Profiles::load(paths), shelf: Shelf::default(), loading: HashSet::new() }
    }

    pub fn get(&self, game: &str) -> Option<&Load<Sheet>> {
        self.shelf.get(game)
    }

    pub fn sheet(&self, game: &str) -> Option<&Sheet> {
        self.shelf.sheet(game)
    }

    pub fn sheets(&self) -> impl Iterator<Item = &Sheet> {
        self.shelf.sheets()
    }

    pub fn is_loading(&self, game: &str) -> bool {
        self.loading.contains(game)
    }

    pub fn profile(&self, game: &str) -> &Profile {
        self.profiles.of(game)
    }

    /// Keeps the game's shown copy until the `Hold` drops.
    pub fn hold(&mut self, game: Arc<str>, cx: &mut Context<Self>) -> Hold {
        let (release, released) = oneshot::channel::<()>();
        self.shelf.hold(game.clone());
        cx.spawn(async move |trainers, cx| {
            // Ends as the `Hold` drops.
            let _ = released.await;
            trainers
                .update(cx, |trainers, cx| {
                    if trainers.shelf.release(&game, trainers.profiles.of(&game)) {
                        trainers.installed(game, cx);
                    }
                })
                .ok();
        })
        .detach();
        Hold { _release: release }
    }

    /// The caches were cleared: the copies go, and the open game's loads with the next list.
    pub fn clear(&mut self, cx: &mut Context<Self>) {
        self.shelf.clear();
        cx.notify();
    }

    /// The hotkeys in effect, by cheat: the user's choices over the provider's.
    pub fn bind(&mut self, game: &Arc<str>, hotkeys: Vec<Vec<Option<Chord>>>, cx: &mut Context<Self>) {
        let Some(sheet) = self.shelf.sheet_mut(game) else { return };
        sheet.hotkeys = hotkeys;
        cx.emit(TrainerEvent::Rebound(game.clone()));
        cx.notify();
    }

    pub fn set(&mut self, game: &Arc<str>, index: usize, value: f64, cx: &mut Context<Self>) {
        let Some(sheet) = self.shelf.sheet_mut(game) else { return };
        sheet.values[index] = value;
        cx.emit(TrainerEvent::Set { game: game.clone(), index, value });
        cx.notify();
    }

    /// Every cheat at rest, as a trainer starts.
    pub fn reset(&mut self, game: &str, cx: &mut Context<Self>) {
        if let Some(sheet) = self.shelf.sheet_mut(game) {
            sheet.reset();
            cx.notify();
        }
    }

    pub fn toggle_pin(&mut self, game: &str, index: usize, cx: &mut Context<Self>) {
        let Some(sheet) = self.shelf.sheet_mut(game) else { return };
        self.profiles.edit(game, |profile| sheet.toggle_pin(index, profile));
        cx.notify();
    }

    pub fn toggle_fold(&mut self, game: &str, title: &str, cx: &mut Context<Self>) {
        let Some(sheet) = self.shelf.sheet_mut(game) else { return };
        self.profiles.edit(game, |profile| sheet.toggle_fold(title, profile));
        cx.notify();
    }

    pub fn toggle_favorite(&mut self, game: &str, cx: &mut Context<Self>) {
        self.profiles.edit(game, |profile| profile.favorite = !profile.favorite);
        cx.notify();
    }

    /// Whether a run's mods come back at the next one; what was kept goes either way.
    pub fn set_save_mods(&mut self, game: &str, on: bool, cx: &mut Context<Self>) {
        self.profiles.edit(game, |profile| {
            profile.save_mods = on;
            profile.values.clear();
        });
        cx.notify();
    }

    pub fn forget_kept(&mut self, game: &str, cx: &mut Context<Self>) {
        self.profiles.edit(game, |profile| profile.values.clear());
        cx.notify();
    }

    /// Renames the collection at `at` and changes its icon, or adds `collection`.
    pub fn keep_collection(&mut self, game: &str, at: Option<usize>, collection: Collection, cx: &mut Context<Self>) {
        self.profiles.edit(game, |profile| match at.and_then(|at| profile.collections.get_mut(at)) {
            Some(kept) => (kept.name, kept.icon) = (collection.name, collection.icon),
            None => profile.collections.push(collection),
        });
        cx.notify();
    }

    pub fn delete_collection(&mut self, game: &str, at: usize, cx: &mut Context<Self>) {
        self.profiles.edit(game, |profile| {
            if at < profile.collections.len() {
                profile.collections.remove(at);
            }
        });
        cx.notify();
    }

    /// Counts a run that went live; the values it starts with.
    pub fn run_opened(&mut self, game: &str, cx: &mut Context<Self>) -> Option<Values> {
        cx.notify();
        self.profiles.edit(game, run::opened)
    }

    /// Keeps a run's time and what it leaves on.
    pub fn run_closed(&mut self, game: &str, secs: u64, cx: &mut Context<Self>) {
        let left = self.shelf.sheet(game).map(Sheet::live_values);
        self.profiles.edit(game, |profile| run::closed(profile, secs, left));
        cx.notify();
    }

    /// The open game's trainer. Nothing is downloaded for a game the user did not open.
    fn open(&mut self, cx: &mut Context<Self>) {
        let Some(game) = self.library.read(cx).opened().cloned() else { return };
        if game.support == Support::NoTrainer || self.shelf.is_current(&game.id, &game.stamp) {
            return;
        }
        // One task per game: a second open while it runs changes nothing.
        if !self.loading.insert(game.id.clone()) {
            return;
        }
        let (id, stamp) = (game.id.clone(), game.stamp.clone());
        let mut snapshots = self.provider.trainer(&game.source, &game.install);
        cx.spawn(async move |trainers, cx| {
            while let Some(snapshot) = snapshots.next().await {
                trainers
                    .update(cx, |trainers, cx| {
                        match snapshot {
                            Snapshot::Cached(Some(trainer)) | Snapshot::Fresh(Ok(trainer)) => {
                                let sheet = Sheet::new(trainer, stamp.clone(), trainers.profiles.of(&id));
                                if trainers.shelf.put(id.clone(), sheet) {
                                    trainers.installed(id.clone(), cx);
                                }
                            }
                            // The network is next.
                            Snapshot::Cached(None) => trainers.shelf.pending(id.clone()),
                            Snapshot::Fresh(Err(e)) => trainers.shelf.failed(id.clone(), format!("{e:#}")),
                        }
                        cx.notify();
                    })
                    .ok();
            }
            trainers
                .update(cx, |trainers, cx| {
                    trainers.loading.remove(&id);
                    // The source changed while it loaded: this trainer is for the old build.
                    if trainers.library.read(cx).opened().is_some_and(|game| game.id == id && game.stamp != stamp) {
                        trainers.open(cx);
                    }
                    cx.notify();
                })
                .ok();
        })
        .detach();
    }

    fn installed(&mut self, game: Arc<str>, cx: &mut Context<Self>) {
        cx.emit(TrainerEvent::Installed(game));
        cx.notify();
    }
}
