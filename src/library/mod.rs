//! Installed catalog games, found through each launcher's own records.

mod ea;
mod epic;
mod gog;
mod registry;
mod rockstar;
mod standalone;
mod steam;
mod ubisoft;
mod vdf;
mod xbox;

use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};

use crate::{catalog::Catalog, pe};

pub struct Installed {
    pub game_id: String,
    pub store_id: String,
    pub location: PathBuf,
    /// PE timestamp of the game's `versionPath`; `None` when unreadable.
    pub version: Option<u32>,
    /// Whether the trainer lists this version; `None` without a version or a trainer.
    pub supported: Option<bool>,
}

/// An install as a launcher reports it.
struct App {
    store_id: String,
    location: PathBuf,
}

/// Scans every launcher (blocking: registry and file reads).
pub fn scan(catalog: &Catalog) -> Vec<Installed> {
    let apps = [
        steam::apps(),
        epic::apps(),
        gog::apps(),
        ubisoft::apps(),
        xbox::apps(catalog),
        rockstar::apps(),
        ea::apps(catalog),
        standalone::apps(catalog),
    ];
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for app in apps.into_iter().flatten() {
        for game_id in catalog.games_for(&app.store_id) {
            if !seen.insert(game_id.as_str()) {
                continue;
            }
            let game = &catalog.games[game_id];
            let version = game
                .version_path
                .as_deref()
                .and_then(|path| pe::header(&app.location.join(path)))
                .map(|header| header.timestamp);
            let supported = version
                .zip(game.trainer.as_ref())
                .map(|(v, trainer)| trainer.supported_versions.contains(&v));
            out.push(Installed {
                game_id: game_id.clone(),
                store_id: app.store_id.clone(),
                location: app.location.clone(),
                version,
                supported,
            });
        }
    }
    out
}

fn same_path(a: &Path, b: &Path) -> bool {
    let norm = |p: &Path| {
        p.to_string_lossy()
            .replace('/', "\\")
            .trim_end_matches('\\')
            .to_lowercase()
    };
    norm(a) == norm(b)
}
