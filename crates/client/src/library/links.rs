//! `map-links.json`: the title the user linked a game to, for maps a source discovers (M4).

use std::collections::BTreeMap;

use model::Ref;
use serde::{Deserialize, Serialize};

use super::GameId;
use crate::storage::{Paths, Stored};

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default)]
struct File {
    /// A title `Ref` as `source:id`; one that does not parse is ignored.
    links: BTreeMap<GameId, String>,
}

/// A link to a source not installed stays, unused, until the source is back or the user replaces it.
pub struct Links(Stored<File>);

impl Links {
    pub fn load(paths: &Paths) -> Self {
        Self(Stored::load(paths.map_links()))
    }

    pub fn get(&self, game: &GameId) -> Option<Ref> {
        self.0.links.get(game).and_then(|title| Ref::parse(title))
    }

    /// Saved at once (blocking: a tiny file).
    pub fn set(&mut self, game: &GameId, title: &Ref) {
        self.0.edit(|file| {
            file.links.insert(game.clone(), title.to_string());
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
        Links::load(&paths).set(&game, &title);
        assert_eq!(Links::load(&paths).get(&game), Some(title));
        let mut links = Links::load(&paths);
        assert!(links.clear(&game) && !links.clear(&game));
        assert_eq!(Links::load(&paths).get(&game), None);
        std::fs::write(paths.map_links(), r#"{"version": 2, "links": {"wand:1": "not a ref"}}"#).unwrap();
        assert_eq!(Links::load(&paths).get(&game), None);
        let _ = std::fs::remove_dir_all(dir);
    }
}
