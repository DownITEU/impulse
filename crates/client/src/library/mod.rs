//! The game list: every installed game with what the app shows about it.

mod picked;
pub mod sources;

use std::{
    collections::{BTreeMap, HashMap, HashSet},
    mem,
    path::Path,
    sync::Arc,
};

use model::{Installation, Origin, StoreId};

pub use picked::Picked;

use crate::media;

#[derive(Clone, Copy, PartialEq)]
pub enum Support {
    Ready,
    /// The installed build is newer than the trainer knows.
    UpdatePending,
    /// There is a trainer, but the game version could not be read.
    Unverified,
    NoTrainer,
}

/// The offer's revision and the installed build: a trainer is asked for again when they change.
pub type Stamp = (Option<String>, Option<u32>);

#[derive(Clone)]
pub struct Game {
    pub id: Arc<str>,
    /// Upper case, as the app prints it.
    pub name: Arc<str>,
    /// Lower case, for the filter.
    pub search: String,
    pub store: String,
    pub support: Support,
    pub mods: u32,
    pub stamp: Stamp,
    pub hero: Option<String>,
    pub thumbnail: Option<String>,
    /// As the provider knows it, for asking for the trainer and the maps.
    pub source: Arc<model::Game>,
    /// The install in use: a launcher's, or an executable the user picked.
    pub install: Installation,
    /// The other one, when the game has both.
    pub other: Option<Installation>,
    /// The install's page in its launcher: the launcher's name and the URI.
    pub page: Option<(&'static str, String)>,
}

impl Game {
    /// Uses the other install.
    pub fn switch(&mut self) {
        if let Some(other) = &mut self.other {
            mem::swap(&mut self.install, other);
            self.judge();
        }
    }

    /// Uses `picked`, an executable the user picked; the launcher's install stays as the other.
    pub fn pick(&mut self, picked: Installation) {
        let previous = [Some(mem::replace(&mut self.install, picked)), self.other.take()];
        self.other = previous.into_iter().flatten().find(|install| sources::exe(install).is_none());
        self.judge();
    }

    /// Forgets the picked executable. `false`: it was the only install.
    pub fn unpick(&mut self) -> bool {
        if sources::exe(&self.install).is_some() {
            let Some(launcher) = self.other.take() else { return false };
            self.install = launcher;
            self.judge();
        }
        self.other = None;
        true
    }

    /// What follows from the install: the store, the build's support, the trainer's stamp.
    fn judge(&mut self) {
        let (offer, build) = (&self.source.trainer, self.install.build);
        self.store = store(&self.install.origin);
        self.support = match (offer, build) {
            (None, _) => Support::NoTrainer,
            (Some(offer), Some(build)) if offer.builds.contains(&build) => Support::Ready,
            (Some(_), Some(_)) => Support::UpdatePending,
            (Some(_), None) => Support::Unverified,
        };
        self.stamp = (offer.as_ref().and_then(|offer| offer.revision.clone()), build);
        self.page = self.install.origin.store_id().and_then(launchers::page);
    }
}

pub struct Library {
    /// Shared, so a view holds one while it draws.
    pub games: Vec<Arc<Game>>,
    /// Titles with a trainer among all the provider's games.
    pub trainers: usize,
}

/// Scans the launchers and the picked executables, and joins the result with `all` (blocking).
pub fn scan(all: Vec<model::Game>, art_cache: &Path, picked: &Picked) -> Library {
    let installed = launchers::scan(&all, &picked.custom());
    // Steam art also serves an Epic or GOG install when the title has a Steam release.
    let mut steam_apps = HashMap::<String, u32>::new();
    let mut with_trainer = HashSet::<&str>::new();
    for game in &all {
        if game.trainer.is_some() {
            with_trainer.insert(&game.title_id);
        }
        if let Some(app) = game.store_ids.iter().find_map(StoreId::steam_app) {
            steam_apps.entry(game.title_id.clone()).or_insert(app);
        }
    }
    let trainers = with_trainer.len();
    let mut by_id: HashMap<String, model::Game> = all.into_iter().map(|game| (game.id.clone(), game)).collect();
    let mut per_store = BTreeMap::<String, usize>::new();
    let mut games = Vec::<Game>::new();
    let mut at = HashMap::<String, usize>::new();
    for found in installed {
        *per_store.entry(store(&found.origin)).or_default() += 1;
        log::debug!(
            "{} [{:?}] v{} @ {}",
            found.game_id,
            found.origin,
            found.build.map_or("?".to_owned(), |v| v.to_string()),
            found.location.display()
        );
        // A launcher's install comes first, a picked executable after it.
        if let Some(&index) = at.get(&found.game_id) {
            let game = &mut games[index];
            game.other = Some(found);
            if picked.used(&game.id) {
                game.switch();
            }
        } else if let Some(entry) = by_id.remove(&found.game_id) {
            at.insert(found.game_id.clone(), games.len());
            games.push(game(entry, found, &steam_apps, art_cache));
        }
    }
    games.sort_by(|a, b| a.search.cmp(&b.search));
    let stores: Vec<String> = per_store.iter().map(|(s, n)| format!("{s} {n}")).collect();
    log::info!("Library: {} installed: {}", games.len(), stores.join(", "));
    Library { games: games.into_iter().map(Arc::new).collect(), trainers }
}

fn game(entry: model::Game, found: Installation, steam_apps: &HashMap<String, u32>, art_cache: &Path) -> Game {
    let app = found.origin.store_id().and_then(StoreId::steam_app).or_else(|| steam_apps.get(entry.title_id.as_str()).copied());
    let mut game = Game {
        id: found.game_id.as_str().into(),
        name: entry.name.to_uppercase().into(),
        search: entry.name.to_lowercase(),
        store: String::new(),
        support: Support::NoTrainer,
        mods: entry.trainer.as_ref().map_or(0, |offer| offer.cheats),
        stamp: (None, None),
        // Not every Steam app has hero art.
        hero: app.map(media::steam_hero).filter(|url| !media::known_missing(art_cache, url)).or_else(|| entry.cover.clone()),
        thumbnail: entry.thumbnail.clone(),
        source: Arc::new(entry),
        install: found,
        other: None,
        page: None,
    };
    game.judge();
    game
}

/// `CUSTOM` for a picked executable, as Wand names it.
fn store(origin: &Origin) -> String {
    origin.store_id().map_or("CUSTOM".into(), |id| id.store().to_uppercase())
}
