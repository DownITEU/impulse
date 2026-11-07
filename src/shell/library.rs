//! The page's game list: every installed catalog game with what the page shows about it.

use std::{
    collections::{BTreeMap, HashMap, HashSet},
    path::Path,
    sync::Arc,
};

use gpui_kit::SharedString;

use super::art;
use crate::{
    catalog::Catalog,
    library::{self, Installed},
    trainer::Stamp,
};

#[derive(Clone, Copy, PartialEq)]
pub enum Support {
    Ready,
    /// The installed build is newer than the trainer knows.
    UpdatePending,
    /// There is a trainer, but the game version could not be read.
    Unverified,
    NoTrainer,
}

pub struct Game {
    pub id: Arc<str>,
    /// Upper case, as the page prints it.
    pub name: SharedString,
    /// Lower case, for the filter.
    pub search: String,
    pub store: String,
    pub version: Option<u32>,
    pub support: Support,
    pub mods: u32,
    pub stamp: Stamp,
    pub hero: Option<String>,
    pub thumbnail: Option<String>,
    pub map: Option<SharedString>,
}

pub struct Library {
    pub games: Vec<Game>,
    /// Titles with a trainer in the whole catalog.
    pub trainers: usize,
}

/// Scans the launchers and joins the result with the catalog (blocking).
pub fn scan(catalog: &Catalog, art_cache: &Path) -> Library {
    // Steam art also serves an Epic or GOG install when the title has a Steam release.
    let mut steam_apps = HashMap::<&str, u32>::new();
    let mut with_trainer = HashSet::<&str>::new();
    for game in catalog.games.values() {
        if game.trainer.is_some() {
            with_trainer.insert(&game.title_id);
        }
        if let Some(app) = game.correlation_ids.iter().find_map(|id| steam_app(id)) {
            steam_apps.entry(&game.title_id).or_insert(app);
        }
    }
    let installed = library::scan(catalog);
    let mut per_store = BTreeMap::<&str, usize>::new();
    let mut games: Vec<Game> = installed
        .iter()
        .map(|found| {
            let game = game(catalog, found, &steam_apps, art_cache);
            *per_store.entry(store(&found.store_id)).or_default() += 1;
            log::debug!(
                "{} [{}] v{} @ {}",
                game.name,
                found.store_id,
                found.version.map_or("?".to_owned(), |v| v.to_string()),
                found.location.display()
            );
            game
        })
        .collect();
    games.sort_by(|a, b| a.search.cmp(&b.search));
    let stores: Vec<String> = per_store.iter().map(|(s, n)| format!("{s} {n}")).collect();
    log::info!("Library: {} installed: {}", games.len(), stores.join(", "));
    Library {
        games,
        trainers: with_trainer.len(),
    }
}

fn game(
    catalog: &Catalog,
    found: &Installed,
    steam_apps: &HashMap<&str, u32>,
    art_cache: &Path,
) -> Game {
    let entry = &catalog.games[&found.game_id];
    let title = &catalog.titles[&entry.title_id];
    let thumbnail = |size| {
        title
            .thumbnail
            .as_deref()
            .map(|path| art::thumbnail(path, size))
    };
    let app =
        steam_app(&found.store_id).or_else(|| steam_apps.get(entry.title_id.as_str()).copied());
    let support = match (found.supported, &entry.trainer) {
        (_, None) => Support::NoTrainer,
        (Some(true), _) => Support::Ready,
        (Some(false), _) => Support::UpdatePending,
        (None, _) => Support::Unverified,
    };
    Game {
        id: found.game_id.as_str().into(),
        name: title.name.to_uppercase().into(),
        search: title.name.to_lowercase(),
        store: store(&found.store_id).to_uppercase(),
        version: found.version,
        support,
        mods: entry.trainer.as_ref().map_or(0, |t| t.cheat_count),
        stamp: Stamp {
            updated_at: entry.trainer.as_ref().and_then(|t| t.updated_at.clone()),
            version: found.version,
        },
        // Not every Steam app has hero art.
        hero: app
            .map(art::steam_hero)
            .filter(|url| !art::known_missing(art_cache, url))
            .or_else(|| thumbnail(460)),
        thumbnail: thumbnail(100),
        map: catalog
            .maps
            .iter()
            .find(|map| map.title_id == entry.title_id)
            .map(|map| map.name.to_uppercase().into()),
    }
}

fn store(store_id: &str) -> &str {
    store_id.split(':').next().unwrap_or_default()
}

fn steam_app(store_id: &str) -> Option<u32> {
    store_id.strip_prefix("steam:")?.parse().ok()
}
