//! Installed games, found through each launcher's own records.

mod appinfo;
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
mod version;
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
    /// The launcher's own name for the game, as `words`: which of the records sharing its store id it is.
    name: Vec<String>,
}

impl App {
    fn new(store_id: String, location: PathBuf) -> Self {
        Self { store_id, location, alternates: Vec::new(), name: Vec::new() }
    }

    fn named(self, name: Option<&str>) -> Self {
        Self { name: name.map(words).unwrap_or_default(), ..self }
    }

    fn names(&self, game: &Game) -> bool {
        !self.name.is_empty() && words(&game.name) == self.name
    }
}

/// A name as compared: its words of letters and digits, in lower case.
fn words(name: &str) -> Vec<String> {
    name.split(|c: char| !c.is_alphanumeric()).filter(|word| !word.is_empty()).map(str::to_lowercase).collect()
}

/// Whether `record` is one install with `game` under `store_id`: it lists the store id and has the same version file.
pub fn same_install(record: &Game, game: &Game, store_id: &StoreId) -> bool {
    let lists = record.store_ids.iter().any(|id| id.as_str().eq_ignore_ascii_case(store_id.as_str()));
    lists && record.exe.as_deref().zip(game.exe.as_deref()).is_some_and(|(a, b)| a.eq_ignore_ascii_case(b))
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

/// One install per game: of its store ids found, the one its record lists first. An install whose record's version file
/// gives no build is none: the game is gone or no longer the one the record describes. A record gives way to one with its
/// version file that the launcher names; then a title has one install per location.
fn launched(games: &[Game], by_store_id: &HashMap<String, Vec<&Game>>, apps: impl Iterator<Item = App>) -> Vec<Installation> {
    let mut best = HashMap::<&Ref, (usize, Installation)>::new();
    for app in apps {
        let Some(id) = StoreId::parse(&app.store_id) else { continue };
        let editions = by_store_id.get(&app.store_id.to_ascii_lowercase()).map_or(&[][..], Vec::as_slice);
        // A collection's games share a store id too: only a record with the same files gives way.
        let renamed = |game: &Game| !app.names(game) && editions.iter().any(|other| app.names(other) && same_install(other, game, &id));
        for game in editions.iter().filter(|game| !renamed(game)) {
            let rank = game.store_ids.iter().position(|own| own.as_str().eq_ignore_ascii_case(id.as_str())).unwrap_or(usize::MAX);
            if best.get(&game.id).is_some_and(|(had, _)| *had <= rank) {
                continue;
            }
            let install = installation(game, Origin::Launcher(id.clone()), app.location.clone(), app.alternates.clone());
            // A record naming no version file stays: another release of its title may have the trainer.
            if install.build.is_some() || game.exe.is_none() {
                best.insert(&game.id, (rank, install));
            }
        }
    }
    one_per_title(games.iter().filter_map(|game| Some((game, best.remove(&game.id)?.1))))
}

/// A title's records on one install are one game: the record whose trainer knows the build, else one with a trainer,
/// else the first.
fn one_per_title<'a>(installs: impl Iterator<Item = (&'a Game, Installation)>) -> Vec<Installation> {
    let rank = |game: &Game, install: &Installation| {
        let offer = game.trainer.as_ref();
        (offer.zip(install.build).is_some_and(|(offer, build)| offer.builds.contains(&build)), offer.is_some())
    };
    let mut kept = Vec::<(&Game, Installation)>::new();
    for (game, install) in installs {
        match kept.iter_mut().find(|(had, was)| had.title_id == game.title_id && same_path(&was.location, &install.location)) {
            Some(slot) if rank(game, &install) > rank(slot.0, &slot.1) => *slot = (game, install),
            Some(_) => {}
            None => kept.push((game, install)),
        }
    }
    kept.into_iter().map(|(_, install)| install).collect()
}

/// One launcher's records, timed: a slow disk or a scanning antivirus shows in the log.
fn timed(launcher: &str, apps: impl FnOnce() -> Vec<App>) -> Vec<App> {
    let started = Instant::now();
    let apps = apps();
    log::debug!("{launcher}: {} apps in {:?}", apps.len(), started.elapsed());
    apps
}

/// The build is the stamp of the game's version file in the install (`version::build`). The location's final path is one
/// of its folders too, for the process search: a library behind a junction, a package on a mount.
fn installation(game: &Game, origin: Origin, location: PathBuf, mut alternates: Vec<PathBuf>) -> Installation {
    let package = origin.store_id().filter(|id| id.store() == "uwp");
    let build = game.exe.as_deref().and_then(|exe| version::build(&location, exe, package.is_some()));
    let app = package.and_then(|id| game.package_apps.iter().find(|(of, _)| of.as_str().eq_ignore_ascii_case(id.as_str())));
    let app = app.map(|(_, app)| app.clone());
    let resolved = final_path(&location).filter(|resolved| !same_path(resolved, &location));
    if let Some(resolved) = resolved.filter(|resolved| !alternates.iter().any(|had| same_path(had, resolved))) {
        alternates.insert(0, resolved);
    }
    Installation { game: game.id.clone(), origin, location, alternates, build, app }
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
            package_apps: Vec::new(),
            cover: None,
            thumbnail: None,
            maps: Vec::new(),
            trainer: None,
        }
    }

    #[test]
    fn a_game_found_by_several_launchers_takes_the_store_its_record_lists_first() {
        let base = std::env::temp_dir().join(format!("impulse-launched-{}", std::process::id()));
        // An archive as the version file: its first entry date is the build.
        for store in ["steam_5", "steam_6", "gog_7", "gog_8"] {
            fs::create_dir_all(base.join(store)).unwrap();
            fs::write(base.join(store).join("v.jar"), b"PK\x03\x04\0\0\0\0\0\0\0\0\x84\x52").unwrap();
        }
        fs::create_dir_all(base.join("steam_9")).unwrap();
        let versioned = |id, ids| game(id, Some("v.jar"), ids);
        let games = [
            versioned("1", &["gog:7", "steam:5"]),
            versioned("2", &["steam:6"]),
            // No build where its record lists first: the other store's install is the one.
            versioned("3", &["steam:9", "gog:8"]),
            // No version file named: listed, for another release's trainer.
            game("4", None, &["steam:5"]),
            versioned("5", &["steam:9"]),
        ];
        let mut by_store_id = HashMap::<String, Vec<&Game>>::new();
        for game in &games {
            for id in &game.store_ids {
                by_store_id.entry(id.as_str().to_owned()).or_default().push(game);
            }
        }
        let app = |id: &str| App::new(id.into(), base.join(id.replace(':', "_")));
        let apps = ["steam:5", "steam:6", "gog:7", "steam:9", "gog:8"].map(app);
        let installs = launched(&games, &by_store_id, apps.into_iter());
        let found: Vec<_> = installs.iter().map(|install| (install.game.id(), install.origin.store_id().unwrap().as_str())).collect();
        let expected = [("1", "gog:7"), ("2", "steam:6"), ("3", "gog:8"), ("4", "steam:5")];
        assert_eq!(found, expected, "a game whose version file is missing is not installed");
        fs::remove_dir_all(&base).unwrap();
    }

    #[test]
    fn an_app_is_the_record_its_launcher_names_a_title_one_game_and_a_collections_own_files_stay() {
        let base = std::env::temp_dir().join(format!("impulse-editions-{}", std::process::id()));
        let dir = base.join("steam_1");
        fs::create_dir_all(&dir).unwrap();
        for exe in ["v.jar", "dlc.jar"] {
            fs::write(dir.join(exe), b"PK\x03\x04\0\0\0\0\0\0\0\0\x84\x52").unwrap();
        }
        let record =
            |id, title: &str, name: &str, exe| Game { title_id: title.into(), name: name.into(), ..game(id, Some(exe), &["steam:1"]) };
        let mut games = [
            record("1", "a", "Mafia III", "v.jar"),
            record("2", "a", "Mafia III", "v.jar"),
            record("3", "b", "Mafia III: Definitive Edition", "v.jar"),
            record("4", "c", "Mafia III: Faster, Baby!", "dlc.jar"),
        ];
        let builds = version::build(&dir, "v.jar", false).into_iter().collect();
        games[1].trainer = Some(model::Offer { id: games[1].id.clone(), cheats: 1, builds, revision: None });
        let by_store_id = HashMap::from([("steam:1".to_owned(), games.iter().collect::<Vec<_>>())]);
        let found = |name| {
            let app = App::new("steam:1".into(), dir.clone()).named(name);
            launched(&games, &by_store_id, [app].into_iter()).iter().map(|install| install.game.id().to_owned()).collect::<Vec<_>>()
        };
        assert_eq!(found(Some("MAFIA III - Definitive Edition™")), ["3", "4"]);
        assert_eq!(found(Some("Mafia III")), ["2", "4"], "of a title's records, the one whose trainer knows the build");
        assert_eq!(found(None), ["2", "3", "4"]);
        assert_eq!(found(Some("Mafia III: Deluxe")), ["2", "3", "4"], "only a whole name names a record");
        fs::remove_dir_all(&base).unwrap();
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
    fn a_package_takes_the_application_its_game_names_for_it() {
        let mut halo = game("1", None, &["uwp:Halo_8w", "uwp:Other_8w"]);
        halo.package_apps = vec![(StoreId::parse("uwp:Halo_8w").unwrap(), "NoEAC".into())];
        let install = |id: &str| installation(&halo, Origin::Launcher(StoreId::parse(id).unwrap()), "C:\\nowhere".into(), Vec::new()).app;
        assert_eq!((install("uwp:halo_8w").as_deref(), install("uwp:Other_8w")), (Some("NoEAC"), None));
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
