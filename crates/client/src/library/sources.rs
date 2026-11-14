//! A game's sources, as Wand calls them: its launcher's install and an executable the user picked.

use std::{collections::HashSet, path::Path};

use model::Installation;

use super::Game;

/// What an entry of a game's source menu does.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Entry {
    /// `true`: the install not in use.
    Use(bool),
    Pick,
    Forget,
}

/// The menu: the launcher's install first, then the picked executable, then what to do with one; and the
/// entry in use.
pub fn entries(game: &Game) -> (Vec<(Entry, String)>, usize) {
    let mut sources: Vec<(bool, &Installation)> = [(false, Some(&game.install)), (true, game.other.as_ref())]
        .into_iter()
        .filter_map(|(other, install)| Some((other, install?)))
        .collect();
    sources.sort_by_key(|(_, install)| exe(install).is_some());
    let current = sources.iter().position(|(other, _)| !other).unwrap_or_default();
    let picked = sources.iter().any(|(_, install)| exe(install).is_some());
    let mut entries: Vec<(Entry, String)> = sources
        .into_iter()
        .map(|(other, install)| {
            let label = match exe(install) {
                Some(exe) => format!("EXE  {}", exe.file_name().unwrap_or_default().to_string_lossy()),
                None => super::store(&install.store_id).to_uppercase(),
            };
            (Entry::Use(other), label)
        })
        .collect();
    entries.push((Entry::Pick, if picked { "CHANGE .EXE" } else { "+ ADD .EXE" }.into()));
    if picked {
        entries.push((Entry::Forget, "REMOVE .EXE".into()));
    }
    (entries, current)
}

/// The executable the user picked as this install.
pub fn exe(install: &Installation) -> Option<&Path> {
    model::custom_exe(&install.store_id)
}

/// The catalog games that keep their executable where `exe` is, by index (blocking).
pub fn owners(games: &[model::Game], exe: &Path) -> Vec<usize> {
    let owners: HashSet<&str> = launchers::owners(games, exe).into_iter().map(|(game, _)| game.id.as_str()).collect();
    (0..games.len()).filter(|&at| owners.contains(games[at].id.as_str())).collect()
}

/// `exe` as an install of `game` (blocking); `None` when it is not a file.
pub fn install(game: &model::Game, exe: &Path) -> Option<Installation> {
    launchers::install(game, exe)
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use model::{Installation, custom_store_id};

    use super::{super::Game, Entry, entries};
    use crate::library::Support;

    fn install(store_id: String) -> Installation {
        Installation { game_id: "g".into(), store_id, location: Default::default(), build: None }
    }

    fn game(install: Installation, other: Option<Installation>) -> Game {
        Game {
            id: "g".into(),
            name: "G".into(),
            search: "g".into(),
            store: String::new(),
            support: Support::NoTrainer,
            mods: 0,
            stamp: (None, None),
            hero: None,
            thumbnail: None,
            source: Default::default(),
            install,
            other,
            page: None,
        }
    }

    #[test]
    fn the_launcher_comes_first_and_a_picked_exe_can_be_changed_or_removed() {
        let (steam, exe) = (install("steam:1".into()), install(custom_store_id(Path::new(r"C:\g\game.exe"))));
        let (only, current) = entries(&game(steam.clone(), None));
        assert_eq!((only, current), (vec![(Entry::Use(false), "STEAM".into()), (Entry::Pick, "+ ADD .EXE".into())], 0));
        let (both, current) = entries(&game(exe, Some(steam)));
        assert_eq!(current, 1);
        assert_eq!(
            both,
            [
                (Entry::Use(true), "STEAM".into()),
                (Entry::Use(false), "EXE  game.exe".into()),
                (Entry::Pick, "CHANGE .EXE".into()),
                (Entry::Forget, "REMOVE .EXE".into())
            ]
        );
    }
}
