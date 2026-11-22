//! A `Ref` the user linked a game to: `map-links.json`, the title for maps a source discovers; `trainer-links.json`, the
//! catalog record whose trainer the game uses instead of its own.

use std::{collections::BTreeMap, path::PathBuf};

use model::Ref;
use serde::{Deserialize, Serialize};

use super::GameId;
use crate::storage::{Paths, Stored};

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default)]
struct File {
    /// A `Ref` as `source:id`; one that does not parse is ignored.
    links: BTreeMap<GameId, String>,
}

/// A link to a source or record not there stays, unused, until it is back or the user replaces it.
pub struct Links(Stored<File>);

impl Links {
    pub fn maps(paths: &Paths) -> Self {
        Self::load(paths.map_links())
    }

    pub fn trainers(paths: &Paths) -> Self {
        Self::load(paths.trainer_links())
    }

    fn load(path: PathBuf) -> Self {
        Self(Stored::load(path))
    }

    pub fn get(&self, game: &GameId) -> Option<Ref> {
        self.0.links.get(game).and_then(|to| Ref::parse(to))
    }

    /// Saved at once (blocking: a tiny file).
    pub fn set(&mut self, game: &GameId, to: &Ref) {
        self.0.edit(|file| {
            file.links.insert(game.clone(), to.to_string());
        });
    }

    /// Forgets the link, saved at once; whether there was one.
    pub fn clear(&mut self, game: &GameId) -> bool {
        let linked = self.0.links.contains_key(game);
        if linked {
            self.0.edit(|file| file.links.remove(game));
        }
        linked
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_link_is_kept_and_one_that_does_not_parse_is_ignored() {
        let dir = std::env::temp_dir().join(format!("impulse-links-{}", std::process::id()));
        let paths = Paths::new(dir.clone());
        let (game, title) = (GameId::of(&Ref::parse("wand:1").unwrap()), Ref::parse("plugin-maps:night").unwrap());
        Links::maps(&paths).set(&game, &title);
        assert_eq!(Links::maps(&paths).get(&game), Some(title.clone()));
        assert_eq!(Links::trainers(&paths).get(&game), None, "each file its own links");
        let mut links = Links::maps(&paths);
        assert!(links.clear(&game) && !links.clear(&game));
        assert_eq!(Links::maps(&paths).get(&game), None);
        std::fs::write(paths.map_links(), r#"{"version": 2, "links": {"wand:1": "not a ref", "wand:2": "wand:3"}}"#).unwrap();
        let links = Links::maps(&paths);
        assert_eq!(links.get(&game), None);
        assert_eq!(links.get(&GameId::of(&Ref::parse("wand:2").unwrap())), Ref::parse("wand:3"));
        let _ = std::fs::remove_dir_all(dir);
    }
}
