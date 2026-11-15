//! The trainers of the games opened in this run: the open game's loading, which copy shows while a run holds it,
//! and what the user keeps about a game (`games.json`).

mod title;

use std::{collections::HashSet, sync::Arc};

use client::{
    Load, Registry,
    library::{GameId, Support},
    profile::{Collection, GameProfile, Profiles, TrainerProfile, Values},
    run,
    storage::Paths,
    trainer::{Sheet, Shelf},
};
use futures::{StreamExt, channel::oneshot};
use gpui_kit::{Context, Entity, EventEmitter};
use model::{Chord, Snapshot};

use crate::library::{Library, LibraryEvent};
pub use title::title;

pub enum TrainerEvent {
    /// A copy of the game's trainer shows: the first, a newer one, or the one that waited.
    Installed(GameId),
    /// The game's hotkeys changed.
    Rebound(GameId),
    /// A cheat's value changed.
    Set { game: GameId, index: usize, value: f64 },
}

/// While it lives the game's shown copy stays; dropping it shows the newer one that waited.
pub struct Hold {
    _release: oneshot::Sender<()>,
}

pub struct Trainers {
    registry: Arc<Registry>,
    library: Entity<Library>,
    profiles: Profiles,
    shelf: Shelf,
    // Games whose trainer is being read or downloaded right now.
    loading: HashSet<GameId>,
}

impl EventEmitter<TrainerEvent> for Trainers {}

impl Trainers {
    pub fn new(registry: Arc<Registry>, paths: &Paths, library: Entity<Library>, cx: &mut Context<Self>) -> Self {
        cx.subscribe(&library, |trainers, _, _: &LibraryEvent, cx| trainers.open(cx)).detach();
        Self { registry, library, profiles: Profiles::load(paths), shelf: Shelf::default(), loading: HashSet::new() }
    }

    pub fn get(&self, game: &GameId) -> Option<&Load<Sheet>> {
        self.shelf.get(game)
    }

    pub fn sheet(&self, game: &GameId) -> Option<&Sheet> {
        self.shelf.sheet(game)
    }

    pub fn sheets(&self) -> impl Iterator<Item = &Sheet> {
        self.shelf.sheets()
    }

    pub fn is_loading(&self, game: &GameId) -> bool {
        self.loading.contains(game)
    }

    pub fn game_profile(&self, game: &GameId) -> &GameProfile {
        self.profiles.game(game)
    }

    /// The shown copy's trainer profile; a blank one without a copy.
    pub fn trainer_profile(&self, game: &GameId) -> &TrainerProfile {
        self.shelf.sheet(game).map_or(TrainerProfile::blank(), |sheet| self.profiles.trainer(&sheet.trainer))
    }

    /// Changes the shown copy's trainer profile; nothing without a copy.
    fn edit_trainer(&mut self, game: &GameId, change: impl FnOnce(&mut TrainerProfile), cx: &mut Context<Self>) {
        let Some(trainer) = self.shelf.sheet(game).map(|sheet| sheet.trainer.clone()) else { return };
        self.profiles.edit_trainer(&trainer, change);
        cx.notify();
    }

    /// Keeps the game's shown copy until the `Hold` drops.
    pub fn hold(&mut self, game: GameId, cx: &mut Context<Self>) -> Hold {
        let (release, released) = oneshot::channel::<()>();
        self.shelf.hold(game.clone());
        cx.spawn(async move |trainers, cx| {
            // Ends as the `Hold` drops.
            let _ = released.await;
            trainers
                .update(cx, |trainers, cx| {
                    if trainers.shelf.release(&game, &trainers.profiles) {
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

    /// The hotkeys in effect, by cheat: the user's choices over the source's.
    pub fn bind(&mut self, game: &GameId, hotkeys: Vec<Vec<Option<Chord>>>, cx: &mut Context<Self>) {
        let Some(sheet) = self.shelf.sheet_mut(game) else { return };
        sheet.hotkeys = hotkeys;
        cx.emit(TrainerEvent::Rebound(game.clone()));
        cx.notify();
    }

    pub fn set(&mut self, game: &GameId, index: usize, value: f64, cx: &mut Context<Self>) {
        let Some(sheet) = self.shelf.sheet_mut(game) else { return };
        sheet.values[index] = value;
        cx.emit(TrainerEvent::Set { game: game.clone(), index, value });
        cx.notify();
    }

    /// Every cheat at rest, as a trainer starts.
    pub fn reset(&mut self, game: &GameId, cx: &mut Context<Self>) {
        if let Some(sheet) = self.shelf.sheet_mut(game) {
            sheet.reset();
            cx.notify();
        }
    }

    pub fn toggle_pin(&mut self, game: &GameId, index: usize, cx: &mut Context<Self>) {
        let Some(sheet) = self.shelf.sheet_mut(game) else { return };
        let trainer = sheet.trainer.clone();
        self.profiles.edit_trainer(&trainer, |profile| sheet.toggle_pin(index, profile));
        cx.notify();
    }

    pub fn toggle_fold(&mut self, game: &GameId, section: &str, cx: &mut Context<Self>) {
        let Some(sheet) = self.shelf.sheet_mut(game) else { return };
        let trainer = sheet.trainer.clone();
        self.profiles.edit_trainer(&trainer, |profile| sheet.toggle_fold(section, profile));
        cx.notify();
    }

    pub fn toggle_favorite(&mut self, game: &GameId, cx: &mut Context<Self>) {
        self.profiles.edit_game(game, |profile| profile.favorite = !profile.favorite);
        cx.notify();
    }

    /// Whether a run's mods come back at the next one; what was kept goes either way.
    pub fn set_save_mods(&mut self, game: &GameId, on: bool, cx: &mut Context<Self>) {
        self.edit_trainer(
            game,
            |profile| {
                profile.save_mods = on;
                profile.values.clear();
            },
            cx,
        );
    }

    pub fn forget_kept(&mut self, game: &GameId, cx: &mut Context<Self>) {
        self.edit_trainer(game, |profile| profile.values.clear(), cx);
    }

    /// Renames the collection at `at` and changes its icon, or adds `collection`.
    pub fn keep_collection(&mut self, game: &GameId, at: Option<usize>, collection: Collection, cx: &mut Context<Self>) {
        let change = |profile: &mut TrainerProfile| match at.and_then(|at| profile.collections.get_mut(at)) {
            Some(kept) => (kept.name, kept.icon) = (collection.name, collection.icon),
            None => profile.collections.push(collection),
        };
        self.edit_trainer(game, change, cx);
    }

    pub fn delete_collection(&mut self, game: &GameId, at: usize, cx: &mut Context<Self>) {
        let change = |profile: &mut TrainerProfile| {
            if at < profile.collections.len() {
                profile.collections.remove(at);
            }
        };
        self.edit_trainer(game, change, cx);
    }

    /// Counts a run that went live; the values it starts with.
    pub fn run_opened(&mut self, game: &GameId, cx: &mut Context<Self>) -> Option<Values> {
        cx.notify();
        let trainer = self.shelf.sheet(game)?.trainer.clone();
        self.profiles.edit_run(game, &trainer, run::opened)
    }

    /// Keeps a run's time and what it leaves on.
    pub fn run_closed(&mut self, game: &GameId, secs: u64, cx: &mut Context<Self>) {
        match self.shelf.sheet(game).map(|sheet| (sheet.trainer.clone(), sheet.live_values())) {
            Some((trainer, left)) => self.profiles.edit_run(game, &trainer, |game, trainer| run::closed(game, trainer, secs, Some(left))),
            // The caches were cleared: only the time is kept.
            None => self.profiles.edit_game(game, |game| run::closed(game, &mut TrainerProfile::default(), secs, None)),
        }
        cx.notify();
    }

    /// The open game's trainer. Nothing is downloaded for a game the user did not open.
    fn open(&mut self, cx: &mut Context<Self>) {
        let Some(game) = self.library.read(cx).opened().cloned() else { return };
        let Some(offer) = game.source.trainer.as_ref() else { return };
        if game.support == Support::NoTrainer || self.shelf.is_current(&game.id, &game.stamp) {
            return;
        }
        // One task per game: a second open while it runs changes nothing.
        if !self.loading.insert(game.id.clone()) {
            return;
        }
        let (id, stamp, trainer_id) = (game.id.clone(), game.stamp.clone(), offer.id.clone());
        let mut snapshots = self.registry.trainer(offer, &game.install);
        cx.spawn(async move |trainers, cx| {
            while let Some(snapshot) = snapshots.next().await {
                trainers
                    .update(cx, |trainers, cx| {
                        match snapshot {
                            Snapshot::Cached(Some(trainer)) | Snapshot::Fresh(Ok(trainer)) => {
                                let sheet = Sheet::new(trainer, trainer_id.clone(), stamp.clone(), trainers.profiles.trainer(&trainer_id));
                                if trainers.shelf.put(id.clone(), sheet) {
                                    trainers.installed(id.clone(), cx);
                                }
                            }
                            // The network is next.
                            Snapshot::Cached(None) => trainers.shelf.pending(id.clone()),
                            Snapshot::Fresh(Err(e)) => trainers.shelf.failed(id.clone(), e),
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

    fn installed(&mut self, game: GameId, cx: &mut Context<Self>) {
        cx.emit(TrainerEvent::Installed(game));
        cx.notify();
    }
}
