//! `picked.json`: the executable the user picked per game, and whether it is used over the launcher's install.

use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

use launchers::Custom;
use serde::{Deserialize, Serialize};

use super::super::store;

const FILE: &str = "picked.json";

#[derive(Clone, Serialize, Deserialize)]
struct Exe {
    path: PathBuf,
    used: bool,
}

#[derive(Clone)]
pub struct Picked {
    file: PathBuf,
    by_game: BTreeMap<String, Exe>,
    /// Grows with each change: a scan of an older one is stale.
    version: u64,
}

impl Picked {
    /// Blocking: the file is tiny.
    pub fn load(data_dir: &Path) -> Self {
        let file = data_dir.join(FILE);
        Self { by_game: store::load(&file).unwrap_or_default(), file, version: 0 }
    }

    pub fn custom(&self) -> Vec<Custom> {
        self.by_game.iter().map(|(id, exe)| Custom { game_id: id.clone(), exe: exe.path.clone() }).collect()
    }

    pub fn version(&self) -> u64 {
        self.version
    }

    pub fn used(&self, game: &str) -> bool {
        self.by_game.get(game).is_some_and(|exe| exe.used)
    }

    /// A new executable is used at once.
    pub fn set(&mut self, game: &str, exe: Option<PathBuf>) {
        match exe {
            Some(path) => self.by_game.insert(game.to_owned(), Exe { path, used: true }),
            None => self.by_game.remove(game),
        };
        self.save();
    }

    pub fn set_used(&mut self, game: &str, used: bool) {
        if let Some(exe) = self.by_game.get_mut(game) {
            exe.used = used;
            self.save();
        }
    }

    fn save(&mut self) {
        self.version += 1;
        store::save(&self.file, &self.by_game);
    }
}
