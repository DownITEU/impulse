//! Installed games, found through each launcher's own records.

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
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
};

use model::{Game, Installation};

/// An install as a launcher reports it.
struct App {
    store_id: String,
    location: PathBuf,
}

/// Scans every launcher for `games` (blocking: registry and file reads).
pub fn scan(games: &[Game]) -> Vec<Installation> {
    // Several editions can share a store id.
    let mut by_store_id = HashMap::<String, Vec<&Game>>::new();
    for game in games {
        for id in &game.store_ids {
            by_store_id.entry(id.to_ascii_lowercase()).or_default().push(game);
        }
    }
    let apps = [
        steam::apps(),
        epic::apps(),
        gog::apps(),
        ubisoft::apps(),
        xbox::apps(|id| by_store_id.contains_key(&id.to_ascii_lowercase())),
        rockstar::apps(),
        ea::apps(games),
        standalone::apps(games),
    ];
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for app in apps.into_iter().flatten() {
        for game in by_store_id.get(&app.store_id.to_ascii_lowercase()).into_iter().flatten() {
            if !seen.insert(game.id.as_str()) {
                continue;
            }
            let build = game
                .exe
                .as_deref()
                .and_then(|exe| pe::header(&app.location.join(exe)))
                .map(|header| header.timestamp);
            out.push(Installation {
                game_id: game.id.clone(),
                store_id: app.store_id.clone(),
                location: app.location.clone(),
                build,
            });
        }
    }
    out
}

/// The launcher's name and the URI of the game's page in it, for the stores that have one.
pub fn page(store_id: &str) -> Option<(&'static str, String)> {
    let (store, id) = store_id.split_once(':')?;
    // Ids come from local records: only plain ones reach a URI.
    let only = |ok: fn(u8) -> bool| !id.is_empty() && id.bytes().all(ok);
    let digits = only(|b| b.is_ascii_digit());
    match store {
        "steam" if digits => Some(("Steam", format!("steam://nav/games/details/{id}"))),
        "gog" if digits => Some(("GOG Galaxy", format!("goggalaxy://openGameView/{id}"))),
        "uwp" if only(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b)) => {
            Some(("Microsoft Store", format!("ms-windows-store://pdp/?PFN={id}")))
        }
        _ => None,
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pages_only_for_plain_ids_of_known_launchers() {
        assert_eq!(page("steam:1203620").unwrap().1, "steam://nav/games/details/1203620");
        assert_eq!(page("uwp:Game.Name_8wekyb3d8bbwe").unwrap().0, "Microsoft Store");
        for id in ["steam:", "steam:12&x", "gog:1 2", "uwp:a/b", "epic:Fortnite", "custom:C:\\a.exe"] {
            assert!(page(id).is_none(), "{id}");
        }
    }
}
