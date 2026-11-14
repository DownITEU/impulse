//! `games.json`: what the user keeps about a game: the star, the play time, the mods a run starts with,
//! their collections, the pins and the folded sections.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use serde::{Deserialize, Serialize};

use crate::storage::{Paths, Stored};

/// Cheat values by cheat id.
pub type Values = BTreeMap<String, f64>;

#[derive(Clone, Serialize, Deserialize)]
pub struct Collection {
    pub name: String,
    /// A key of `collections::ICONS`.
    pub icon: String,
    pub mods: Values,
}

#[derive(Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Profile {
    pub favorite: bool,
    /// Runs that went live.
    pub sessions: u32,
    /// Seconds live: in all runs, and in the last one.
    pub played: u64,
    pub last: u64,
    /// A run starts with `values` set.
    pub save_mods: bool,
    /// What the last run left on.
    pub values: Values,
    pub collections: Vec<Collection>,
    /// Cheat ids; `None` until the user pins or unpins: the provider's pins.
    pub pinned: Option<BTreeSet<String>>,
    /// Titles of the folded sections.
    pub folded: BTreeSet<String>,
}

static BLANK: Profile = Profile {
    favorite: false,
    sessions: 0,
    played: 0,
    last: 0,
    save_mods: false,
    values: Values::new(),
    collections: Vec::new(),
    pinned: None,
    folded: BTreeSet::new(),
};

pub struct Profiles {
    /// By game id.
    games: Stored<HashMap<String, Profile>>,
}

impl Profiles {
    pub fn load(paths: &Paths) -> Self {
        Self { games: Stored::load(paths.games()) }
    }

    pub fn of(&self, game: &str) -> &Profile {
        self.games.get(game).unwrap_or(&BLANK)
    }

    /// Changes a game's profile and saves.
    pub fn edit<R>(&mut self, game: &str, change: impl FnOnce(&mut Profile) -> R) -> R {
        self.games.edit(|games| change(games.entry(game.to_owned()).or_default()))
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    #[test]
    fn profiles_survive_a_reload_and_older_files() {
        let dir = std::env::temp_dir().join(format!("impulse-profiles-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let paths = Paths::new(dir.clone());
        fs::write(paths.games(), br#"{"old":{"favorite":true}}"#).unwrap();
        let mut profiles = Profiles::load(&paths);
        assert!(profiles.of("old").favorite && profiles.of("old").pinned.is_none());
        assert!(!profiles.of("new").favorite);
        profiles.edit("new", |profile| {
            profile.sessions += 1;
            profile.collections.push(Collection {
                name: "Boss".to_owned(),
                icon: "skull".to_owned(),
                mods: Values::from([("a".to_owned(), 1.)]),
            });
        });
        let back = Profiles::load(&paths);
        assert!(back.of("old").favorite && back.of("new").sessions == 1);
        assert_eq!(back.of("new").collections[0].mods["a"], 1.);
        fs::remove_dir_all(&dir).unwrap();
    }
}
