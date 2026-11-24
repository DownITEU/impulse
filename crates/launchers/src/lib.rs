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
mod store;
mod ubisoft;
mod vdf;
mod xbox;

use std::{
    cmp::Reverse,
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    time::Instant,
};

use model::{Game, Installation, Origin, Ref, StoreId};

pub use custom::{Custom, install, owners};
pub use start::start;
pub use store::{store_link, store_name};

struct App {
    store_id: String,
    /// The launcher's own name for the game, when its records hold one: which edition sharing the store id it is.
    name: Option<String>,
    location: PathBuf,
}

/// Scans every launcher for `games`, then the executables the user picked (blocking: registry and
/// file reads). An app is one game's install, a game has one launcher's install at most, and its picked one after it.
pub fn scan(games: &[Game], custom: &[Custom]) -> Vec<Installation> {
    // Several editions can share a store id.
    let mut by_store_id = HashMap::<String, Vec<&Game>>::new();
    for game in games {
        for id in &game.store_ids {
            by_store_id.entry(id.as_str().to_ascii_lowercase()).or_default().push(game);
        }
    }
    let apps = [
        timed("steam", steam::apps),
        timed("epic", epic::apps),
        timed("gog", gog::apps),
        timed("ubisoft", ubisoft::apps),
        timed("xbox", || xbox::apps(|id| by_store_id.contains_key(&id.to_ascii_lowercase()))),
        timed("rockstar", rockstar::apps),
        timed("ea", || ea::apps(games)),
        timed("standalone", || standalone::apps(games)),
    ];
    let builds = Instant::now();
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for app in apps.into_iter().flatten() {
        let Some(id) = StoreId::parse(&app.store_id) else { continue };
        let editions = by_store_id.get(&app.store_id.to_ascii_lowercase()).into_iter().flatten().filter(|game| !seen.contains(&game.id));
        let installs = editions.map(|game| (*game, installation(game, Origin::Launcher(id.clone()), app.location.clone())));
        if let Some((game, install)) = edition(app.name.as_deref(), installs) {
            seen.insert(&game.id);
            out.push(install);
        }
    }
    let by_id: HashMap<&Ref, &Game> = games.iter().map(|game| (&game.id, game)).collect();
    out.extend(custom.iter().filter_map(|picked| custom::install(by_id.get(&picked.game)?, &picked.exe)));
    log::debug!("builds: {} read in {:?}", out.len(), builds.elapsed());
    out
}

/// One launcher's records, timed: a slow disk or a scanning antivirus shows in the log.
fn timed(launcher: &str, apps: impl FnOnce() -> Vec<App>) -> Vec<App> {
    let started = Instant::now();
    let apps = apps();
    log::debug!("{launcher}: {} apps in {:?}", apps.len(), started.elapsed());
    apps
}

/// The build is the PE TimeDateStamp of the game's `exe` in the install.
fn installation(game: &Game, origin: Origin, location: PathBuf) -> Installation {
    let build = game.exe.as_deref().and_then(|exe| pe::header(&location.join(exe))).map(|header| header.timestamp);
    Installation { game: game.id.clone(), origin, location, build }
}

/// The edition an app is, of the games sharing its store id: the one its launcher names, then one whose trainer knows
/// the installed build, then one with a trainer, then the first.
fn edition<'a>(launcher: Option<&str>, installs: impl Iterator<Item = (&'a Game, Installation)>) -> Option<(&'a Game, Installation)> {
    let launcher = launcher.map(words).unwrap_or_default();
    let rank = |at: usize, game: &Game, install: &Installation| {
        let offer = game.trainer.as_ref();
        let knows = offer.zip(install.build).is_some_and(|(offer, build)| offer.builds.contains(&build));
        (named(&launcher, &game.name), knows, offer.is_some(), Reverse(at))
    };
    installs.enumerate().max_by_key(|(at, (game, install))| rank(*at, game, install)).map(|(_, found)| found)
}

/// How well `name` answers to the launcher's name, both as words: all of them, else how many it starts with, else 0.
fn named(launcher: &[String], name: &str) -> usize {
    let name = words(name);
    if name.is_empty() {
        0
    } else if name == launcher {
        usize::MAX
    } else if launcher.starts_with(&name) {
        name.len()
    } else {
        0
    }
}

/// A name as compared: its words of letters and digits, in lower case.
fn words(name: &str) -> Vec<String> {
    name.split(|c: char| !c.is_alphanumeric()).filter(|word| !word.is_empty()).map(str::to_lowercase).collect()
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
            terms: Vec::new(),
            store_ids: store_ids.iter().filter_map(|id| StoreId::parse(id)).collect(),
            exe: exe.map(str::to_owned),
            purchase: Vec::new(),
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

    /// Records sharing `steam:1`, each with a trainer for `builds` when given.
    fn editions(names: &[(&str, Option<&[u32]>)]) -> Vec<Game> {
        let edition = |(at, (name, builds)): (usize, &(&str, Option<&[u32]>))| {
            let mut game = game(&at.to_string(), None, &["steam:1"]);
            game.name = (*name).to_owned();
            game.trainer = builds.map(|builds| model::Offer { id: game.id.clone(), cheats: 1, builds: builds.to_vec(), revision: None });
            game
        };
        names.iter().enumerate().map(edition).collect()
    }

    /// The record `edition` takes for an install of `steam:1` at `build`.
    fn installed_as(launcher: Option<&str>, games: &[Game], build: Option<u32>) -> String {
        let origin = Origin::Launcher(StoreId::parse("steam:1").unwrap());
        let installs = games
            .iter()
            .map(|game| (game, Installation { game: game.id.clone(), origin: origin.clone(), location: PathBuf::new(), build }));
        edition(launcher, installs).map(|(game, _)| game.id.id().to_owned()).unwrap_or_default()
    }

    #[test]
    fn an_app_is_the_edition_its_launcher_names_else_the_longest_start_of_that_name() {
        let games = editions(&[("Mafia III", Some(&[7])), ("Mafia III", Some(&[7])), ("Mafia III: Definitive Edition", Some(&[7]))]);
        assert_eq!(installed_as(Some("Mafia III: Definitive Edition"), &games, Some(7)), "2");
        assert_eq!(installed_as(Some("MAFIA III - Definitive Edition™"), &games, Some(7)), "2", "only letters and digits count");
        let games = editions(&[("Mafia", None), ("Mafia III", None), ("Mafia II", None)]);
        assert_eq!(installed_as(Some("Mafia III: Deluxe"), &games, None), "1");
    }

    #[test]
    fn without_a_name_an_app_is_the_edition_whose_trainer_knows_its_build_else_one_with_a_trainer_else_the_first() {
        let games = editions(&[("A", None), ("B", Some(&[5])), ("C", Some(&[7]))]);
        assert_eq!(installed_as(None, &games, Some(7)), "2");
        assert_eq!(installed_as(Some("Another game"), &games, Some(9)), "1", "a name starts another only with whole words");
        assert_eq!(installed_as(None, &editions(&[("A", None), ("B", None)]), None), "0");
        assert_eq!(installed_as(None, &[], None), "");
    }
}
