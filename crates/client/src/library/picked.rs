//! `picked.json`: the executable the user picked per game, and whether it is used over the launcher's install.

use std::{collections::BTreeMap, path::PathBuf};

use launchers::Custom;
use serde::{Deserialize, Serialize};

use super::GameId;
use crate::storage::{Paths, Stored};

#[derive(Clone, Serialize, Deserialize)]
struct Exe {
    path: PathBuf,
    used: bool,
}

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default)]
struct Games {
    games: BTreeMap<GameId, Exe>,
}

#[derive(Clone)]
pub struct Picked {
    by_game: Stored<Games>,
    /// Grows with each change: a scan of an older one is stale.
    version: u64,
}

impl Picked {
    pub fn load(paths: &Paths) -> Self {
        Self { by_game: Stored::load(paths.picked()), version: 0 }
    }

    pub fn custom(&self) -> Vec<Custom> {
        self.by_game.games.iter().filter_map(|(id, exe)| Some(Custom { game: id.game()?, exe: exe.path.clone() })).collect()
    }

    pub fn version(&self) -> u64 {
        self.version
    }

    pub fn used(&self, game: &GameId) -> bool {
        self.by_game.games.get(game).is_some_and(|exe| exe.used)
    }

    /// A new executable is used at once.
    pub fn set(&mut self, game: &GameId, exe: Option<PathBuf>) {
        self.version += 1;
        self.by_game.edit(|by_game| match exe {
            Some(path) => by_game.games.insert(game.clone(), Exe { path, used: true }),
            None => by_game.games.remove(game),
        });
    }

    pub fn set_used(&mut self, game: &GameId, used: bool) {
        if self.by_game.games.contains_key(game) {
            self.version += 1;
            self.by_game.edit(|by_game| {
                if let Some(exe) = by_game.games.get_mut(game) {
                    exe.used = used;
                }
            });
        }
    }
}
