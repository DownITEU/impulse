//! The page's game list: every installed game with what the page shows about it.

use std::{
    collections::{BTreeMap, HashMap, HashSet},
    path::Path,
    sync::Arc,
};

use gpui_kit::SharedString;
use model::Installation;

use super::art;
use crate::library;

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

pub struct Game {
    pub id: Arc<str>,
    /// Upper case, as the page prints it.
    pub name: SharedString,
    /// Lower case, for the filter.
    pub search: String,
    pub store: String,
    pub support: Support,
    pub mods: u32,
    pub stamp: Stamp,
    pub hero: Option<String>,
    pub thumbnail: Option<String>,
    pub map: Option<SharedString>,
    /// As the provider knows it, for asking for the trainer.
    pub source: model::Game,
    pub install: Installation,
}

pub struct Library {
    pub games: Vec<Game>,
    /// Titles with a trainer among all the provider's games.
    pub trainers: usize,
}

/// Scans the launchers and joins the result with `all` (blocking).
pub fn scan(all: Vec<model::Game>, art_cache: &Path) -> Library {
    let installed = library::scan(&all);
    // Steam art also serves an Epic or GOG install when the title has a Steam release.
    let mut steam_apps = HashMap::<String, u32>::new();
    let mut with_trainer = HashSet::<&str>::new();
    for game in &all {
        if game.trainer.is_some() {
            with_trainer.insert(&game.title_id);
        }
        if let Some(app) = game.store_ids.iter().find_map(|id| steam_app(id)) {
            steam_apps.entry(game.title_id.clone()).or_insert(app);
        }
    }
    let trainers = with_trainer.len();
    let mut by_id: HashMap<String, model::Game> = all.into_iter().map(|game| (game.id.clone(), game)).collect();
    let mut per_store = BTreeMap::<String, usize>::new();
    let mut games: Vec<Game> = installed
        .into_iter()
        .filter_map(|found| {
            let game = game(by_id.remove(&found.game_id)?, found, &steam_apps, art_cache);
            let install = &game.install;
            *per_store.entry(store(&install.store_id).to_owned()).or_default() += 1;
            log::debug!(
                "{} [{}] v{} @ {}",
                game.name,
                install.store_id,
                install.build.map_or("?".to_owned(), |v| v.to_string()),
                install.location.display()
            );
            Some(game)
        })
        .collect();
    games.sort_by(|a, b| a.search.cmp(&b.search));
    let stores: Vec<String> = per_store.iter().map(|(s, n)| format!("{s} {n}")).collect();
    log::info!("Library: {} installed: {}", games.len(), stores.join(", "));
    Library { games, trainers }
}

fn game(
    entry: model::Game,
    found: Installation,
    steam_apps: &HashMap<String, u32>,
    art_cache: &Path,
) -> Game {
    let app =
        steam_app(&found.store_id).or_else(|| steam_apps.get(entry.title_id.as_str()).copied());
    let support = match (&entry.trainer, found.build) {
        (None, _) => Support::NoTrainer,
        (Some(offer), Some(build)) if offer.builds.contains(&build) => Support::Ready,
        (Some(_), Some(_)) => Support::UpdatePending,
        (Some(_), None) => Support::Unverified,
    };
    Game {
        id: found.game_id.as_str().into(),
        name: entry.name.to_uppercase().into(),
        search: entry.name.to_lowercase(),
        store: store(&found.store_id).to_uppercase(),
        support,
        mods: entry.trainer.as_ref().map_or(0, |offer| offer.cheats),
        stamp: (entry.trainer.as_ref().and_then(|offer| offer.revision.clone()), found.build),
        // Not every Steam app has hero art.
        hero: app
            .map(art::steam_hero)
            .filter(|url| !art::known_missing(art_cache, url))
            .or_else(|| entry.cover.clone()),
        thumbnail: entry.thumbnail.clone(),
        map: entry.map.as_ref().map(|name| name.to_uppercase().into()),
        source: entry,
        install: found,
    }
}

fn store(store_id: &str) -> &str {
    store_id.split(':').next().unwrap_or_default()
}

fn steam_app(store_id: &str) -> Option<u32> {
    store_id.strip_prefix("steam:")?.parse().ok()
}
