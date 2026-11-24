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
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    time::Instant,
};

use model::{Game, Installation, Origin, Ref, StoreId};

pub use custom::{Custom, install, owners};
pub use start::start;
pub use store::{store_link, store_name};

struct App {
    store_id: String,
    location: PathBuf,
    /// The install's other folders its process may run from.
    alternates: Vec<PathBuf>,
}

impl App {
    fn new(store_id: String, location: PathBuf) -> Self {
        Self { store_id, location, alternates: Vec::new() }
    }
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
    let mut out = launched(games, &by_store_id, apps.into_iter().flatten());
    let by_id: HashMap<&Ref, &Game> = games.iter().map(|game| (&game.id, game)).collect();
    out.extend(custom.iter().filter_map(|picked| custom::install(by_id.get(&picked.game)?, &picked.exe)));
    log::debug!("builds: {} read in {:?}", out.len(), builds.elapsed());
    out
}

/// One install per game: of its store ids found, the one its record lists first, as Wand prefers them.
fn launched(games: &[Game], by_store_id: &HashMap<String, Vec<&Game>>, apps: impl Iterator<Item = App>) -> Vec<Installation> {
    let mut best = HashMap::<&Ref, (usize, StoreId, &App)>::new();
    let apps: Vec<App> = apps.collect();
    for app in &apps {
        let Some(id) = StoreId::parse(&app.store_id) else { continue };
        for game in by_store_id.get(&app.store_id.to_ascii_lowercase()).into_iter().flatten() {
            let rank = game.store_ids.iter().position(|own| own.as_str().eq_ignore_ascii_case(id.as_str())).unwrap_or(usize::MAX);
            if best.get(&game.id).is_none_or(|(had, ..)| rank < *had) {
                best.insert(&game.id, (rank, id.clone(), app));
            }
        }
    }
    let found = games.iter().filter_map(|game| Some((game, best.remove(&game.id)?)));
    found.map(|(game, (_, id, app))| installation(game, Origin::Launcher(id), app.location.clone(), app.alternates.clone())).collect()
}

/// One launcher's records, timed: a slow disk or a scanning antivirus shows in the log.
fn timed(launcher: &str, apps: impl FnOnce() -> Vec<App>) -> Vec<App> {
    let started = Instant::now();
    let apps = apps();
    log::debug!("{launcher}: {} apps in {:?}", apps.len(), started.elapsed());
    apps
}

/// The build is the PE TimeDateStamp of the game's `exe` in the install. The location's final path is one of its
/// folders too, as Wand resolves it before it looks for the process: a library behind a junction, a package on a mount.
fn installation(game: &Game, origin: Origin, location: PathBuf, mut alternates: Vec<PathBuf>) -> Installation {
    let build = game.exe.as_deref().and_then(|exe| pe::header(&location.join(exe))).map(|header| header.timestamp);
    let resolved = final_path(&location).filter(|resolved| !same_path(resolved, &location));
    if let Some(resolved) = resolved.filter(|resolved| !alternates.iter().any(|had| same_path(had, resolved))) {
        alternates.insert(0, resolved);
    }
    Installation { game: game.id.clone(), origin, location, alternates, build }
}

/// The path once links resolve, written as a process's image path is (blocking).
fn final_path(path: &Path) -> Option<PathBuf> {
    let resolved = fs::canonicalize(path).ok()?;
    let text = resolved.to_str()?;
    Some(match text.strip_prefix(r"\\?\UNC\") {
        Some(share) => format!(r"\\{share}").into(),
        None => text.strip_prefix(r"\\?\").unwrap_or(text).into(),
    })
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
    fn a_game_found_by_several_launchers_takes_the_store_its_record_lists_first() {
        let games = [game("1", None, &["gog:7", "steam:5"]), game("2", None, &["steam:6"])];
        let mut by_store_id = HashMap::<String, Vec<&Game>>::new();
        for game in &games {
            for id in &game.store_ids {
                by_store_id.entry(id.as_str().to_owned()).or_default().push(game);
            }
        }
        let app = |id: &str| App::new(id.into(), id.into());
        let installs = launched(&games, &by_store_id, [app("steam:5"), app("steam:6"), app("gog:7")].into_iter());
        let origins: Vec<_> = installs.iter().map(|install| install.origin.store_id().unwrap().as_str()).collect();
        assert_eq!(origins, ["gog:7", "steam:6"]);
    }

    #[test]
    fn a_location_behind_a_junction_also_counts_as_its_final_path() {
        let base = std::env::temp_dir().join(format!("impulse-junction-{}", std::process::id()));
        let (real, link) = (base.join("real"), base.join("link"));
        fs::create_dir_all(&real).unwrap();
        let made = std::process::Command::new("cmd").args(["/c", "mklink", "/J"]).arg(&link).arg(&real).output().unwrap();
        assert!(made.status.success(), "{made:?}");

        let install = installation(&game("1", None, &[]), Origin::Picked(link.join("g.exe")), link.clone(), Vec::new());
        let final_real = final_path(&real).unwrap();
        assert!(!final_real.to_string_lossy().starts_with(r"\\?\"), "{final_real:?}");
        assert!(install.alternates.iter().any(|folder| same_path(folder, &final_real)), "{:?}", install.alternates);
        fs::remove_dir_all(&base).unwrap();
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
