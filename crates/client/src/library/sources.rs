//! The game source menu offers the launcher's install and an executable the user picked, and another record's trainer.

use std::{collections::HashSet, path::Path};

use model::{Installation, Origin, Ref};

use super::Game;

/// What an entry of a game's source menu does.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Entry {
    /// `true`: the install not in use.
    Use(bool),
    Pick,
    Forget,
    /// Another record's trainer, picked by name.
    Trainer,
    /// Back to its own record's trainer.
    OwnTrainer,
}

/// What an entry of the menu says; the shell words it.
#[derive(Clone, Debug, PartialEq)]
pub enum Label {
    /// A launcher's install, by its store.
    Store(String),
    /// A picked executable, by its file name.
    Exe(String),
    Add,
    Change,
    Remove,
    Trainer,
    OwnTrainer,
}

/// The menu: the launcher's install first, then the picked executable, then what to do with one, then the trainer's
/// choice; and the entry in use.
pub fn entries(game: &Game) -> (Vec<(Entry, Label)>, usize) {
    let mut sources: Vec<(bool, &Installation)> = [(false, game.install.as_ref()), (true, game.other.as_ref())]
        .into_iter()
        .filter_map(|(other, install)| Some((other, install?)))
        .collect();
    sources.sort_by_key(|(_, install)| exe(install).is_some());
    let current = sources.iter().position(|(other, _)| !other).unwrap_or_default();
    let picked = sources.iter().any(|(_, install)| exe(install).is_some());
    let mut entries: Vec<(Entry, Label)> = sources
        .into_iter()
        .map(|(other, install)| {
            let label = match &install.origin {
                Origin::Picked(exe) => Label::Exe(exe.file_name().unwrap_or_default().to_string_lossy().into_owned()),
                Origin::Launcher(id) => Label::Store(launchers::store_name(id.store())),
            };
            (Entry::Use(other), label)
        })
        .collect();
    entries.push((Entry::Pick, if picked { Label::Change } else { Label::Add }));
    if picked {
        entries.push((Entry::Forget, Label::Remove));
    }
    entries.push((Entry::Trainer, Label::Trainer));
    if game.trainer_from.is_some() {
        entries.push((Entry::OwnTrainer, Label::OwnTrainer));
    }
    (entries, current)
}

pub fn exe(install: &Installation) -> Option<&Path> {
    match &install.origin {
        Origin::Picked(exe) => Some(exe),
        Origin::Launcher(_) => None,
    }
}

/// The catalog games that keep their executable where `exe` is, by index (blocking).
pub fn owners(games: &[model::Game], exe: &Path) -> Vec<usize> {
    let owners: HashSet<&Ref> = launchers::owners(games, exe).into_iter().map(|(game, _)| &game.id).collect();
    (0..games.len()).filter(|&at| owners.contains(&games[at].id)).collect()
}

/// `exe` as an install of `game` (blocking); `None` when it is not a file.
pub fn install(game: &model::Game, exe: &Path) -> Option<Installation> {
    launchers::install(game, exe)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use model::{Installation, Origin, Ref, StoreId};

    use super::{super::Game, Entry, Label, entries};
    use crate::library::{GameId, Support};

    fn id() -> Ref {
        Ref::parse("s:g").unwrap()
    }

    fn install(origin: Origin) -> Installation {
        Installation { game: id(), origin, location: Default::default(), alternates: Vec::new(), build: None }
    }

    fn record() -> model::Game {
        model::Game {
            id: id(),
            title_id: String::new(),
            name: "G".into(),
            terms: Vec::new(),
            store_ids: Vec::new(),
            exe: None,
            purchase: Vec::new(),
            standalone: Vec::new(),
            cover: None,
            thumbnail: None,
            maps: Vec::new(),
            trainer: None,
        }
    }

    fn game(install: Installation, other: Option<Installation>) -> Game {
        Game {
            id: GameId::of(&id()),
            name: "G".into(),
            search: "g".into(),
            store: None,
            support: Support::NoTrainer,
            mods: 0,
            stamp: Default::default(),
            hero: None,
            thumbnail: None,
            source: Arc::new(record()),
            trainer_from: None,
            others: Vec::new(),
            install: Some(install),
            other,
            page: None,
            stores: Vec::new(),
        }
    }

    #[test]
    fn the_launcher_comes_first_and_a_picked_exe_can_be_changed_or_removed() {
        let steam = install(Origin::Launcher(StoreId::parse("steam:1").unwrap()));
        let exe = install(Origin::Picked(r"C:\g\game.exe".into()));
        let (only, current) = entries(&game(steam.clone(), None));
        let store = (Entry::Use(false), Label::Store("STEAM".into()));
        assert_eq!((only, current), (vec![store, (Entry::Pick, Label::Add), (Entry::Trainer, Label::Trainer)], 0));
        let (both, current) = entries(&game(exe, Some(steam)));
        assert_eq!(current, 1);
        assert_eq!(
            both,
            [
                (Entry::Use(true), Label::Store("STEAM".into())),
                (Entry::Use(false), Label::Exe("game.exe".into())),
                (Entry::Pick, Label::Change),
                (Entry::Forget, Label::Remove),
                (Entry::Trainer, Label::Trainer)
            ]
        );
    }

    #[test]
    fn a_game_on_another_records_trainer_can_go_back_to_its_own() {
        let mut linked = game(install(Origin::Launcher(StoreId::parse("gog:1").unwrap())), None);
        linked.trainer_from = Some(crate::library::Release::of(&record()));
        assert_eq!(entries(&linked).0.last(), Some(&(Entry::OwnTrainer, Label::OwnTrainer)));
    }
}
