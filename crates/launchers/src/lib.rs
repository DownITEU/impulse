//! Installed games, found through each launcher's own records.

mod custom;
mod ea;
mod epic;
mod gog;
mod registry;
mod rockstar;
mod standalone;
mod start;
mod steam;
mod ubisoft;
mod vdf;
mod xbox;

use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
};

use model::{Game, Installation, Origin, Ref, StoreId};

pub use custom::{Custom, install, owners};
pub use start::start;

/// An install as a launcher reports it.
struct App {
    store_id: String,
    location: PathBuf,
}

/// Scans every launcher for `games`, then the executables the user picked (blocking: registry and
/// file reads). A game has one launcher's install at most, and its picked one after it.
pub fn scan(games: &[Game], custom: &[Custom]) -> Vec<Installation> {
    // Several editions can share a store id.
    let mut by_store_id = HashMap::<String, Vec<&Game>>::new();
    for game in games {
        for id in &game.store_ids {
            by_store_id.entry(id.as_str().to_ascii_lowercase()).or_default().push(game);
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
        let Some(id) = StoreId::parse(&app.store_id) else { continue };
        for game in by_store_id.get(&app.store_id.to_ascii_lowercase()).into_iter().flatten() {
            if seen.insert(&game.id) {
                out.push(installation(game, Origin::Launcher(id.clone()), app.location.clone()));
            }
        }
    }
    let by_id: HashMap<&Ref, &Game> = games.iter().map(|game| (&game.id, game)).collect();
    out.extend(custom.iter().filter_map(|picked| custom::install(by_id.get(&picked.game)?, &picked.exe)));
    out
}

/// The build is the PE TimeDateStamp of the game's `exe` in the install.
fn installation(game: &Game, origin: Origin, location: PathBuf) -> Installation {
    let build = game.exe.as_deref().and_then(|exe| pe::header(&location.join(exe))).map(|header| header.timestamp);
    Installation { game: game.id.clone(), origin, location, build }
}

/// The launcher's name and the URI of the game's page in it, for the stores that have one.
pub fn page(store_id: &StoreId) -> Option<(&'static str, String)> {
    let id = store_id.id();
    // Ids come from local records: only plain ones reach a URI.
    let only = |ok: fn(u8) -> bool| !id.is_empty() && id.bytes().all(ok);
    let digits = only(|b| b.is_ascii_digit());
    match store_id.store() {
        "steam" if digits => Some(("Steam", format!("steam://nav/games/details/{id}"))),
        "gog" if digits => Some(("GOG Galaxy", format!("goggalaxy://openGameView/{id}"))),
        "uwp" if only(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b)) => {
            Some(("Microsoft Store", format!("ms-windows-store://pdp/?PFN={id}")))
        }
        _ => None,
    }
}

fn same_path(a: &Path, b: &Path) -> bool {
    let norm = |p: &Path| p.to_string_lossy().replace('/', "\\").trim_end_matches('\\').to_lowercase();
    norm(a) == norm(b)
}

#[cfg(test)]
pub(crate) mod tests {
    use model::SourceId;

    use super::*;

    /// A catalog game of source `t`.
    pub(crate) fn game(id: &str, exe: Option<&str>, store_ids: &[&str]) -> Game {
        Game {
            id: Ref::new(&SourceId::new("t").unwrap(), id),
            title_id: String::new(),
            name: String::new(),
            store_ids: store_ids.iter().filter_map(|id| StoreId::parse(id)).collect(),
            exe: exe.map(str::to_owned),
            standalone: Vec::new(),
            cover: None,
            thumbnail: None,
            maps: Vec::new(),
            trainer: None,
        }
    }

    #[test]
    fn pages_only_for_plain_ids_of_known_launchers() {
        let page = |id: &str| page(&StoreId::parse(id).unwrap());
        assert_eq!(page("steam:1203620").unwrap().1, "steam://nav/games/details/1203620");
        assert_eq!(page("uwp:Game.Name_8wekyb3d8bbwe").unwrap().0, "Microsoft Store");
        for id in ["steam:", "steam:12&x", "gog:1 2", "uwp:a/b", "epic:Fortnite"] {
            assert!(page(id).is_none(), "{id}");
        }
    }
}
