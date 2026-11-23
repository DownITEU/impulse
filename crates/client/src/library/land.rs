//! What a new list does to the open game.

use std::sync::Arc;

use model::Ref;

use super::{GameId, Library};

#[derive(Debug, PartialEq)]
pub enum Landing {
    /// The browsed record is listed now: it opens as listed, kept in the prefs.
    Listed(GameId),
    /// `open`: the listed game open before while still listed, else the first. `browse`: the browsed record, still not
    /// listed, made again from a new catalog.
    Kept { open: Option<GameId>, browse: Option<(Ref, bool)> },
}

impl Library {
    /// `before`: the list shown; `open`: its listed game open; `browsed`: the record browsed and whether it opens.
    pub fn land(&self, before: Option<&Library>, open: Option<&GameId>, browsed: Option<(Ref, bool)>) -> Landing {
        let listed = |id: &GameId| self.games.iter().any(|game| game.id == *id);
        if let Some(id) = browsed.as_ref().map(|(record, _)| GameId::of(record)).filter(|id| listed(id)) {
            return Landing::Listed(id);
        }
        let open = open.filter(|id| listed(id)).or_else(|| self.games.first().map(|game| &game.id)).cloned();
        // The same catalog makes the same game.
        let new_catalog = before.is_none_or(|before| !Arc::ptr_eq(&before.catalog, &self.catalog));
        Landing::Kept { open, browse: browsed.filter(|_| new_catalog) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::{
        browse,
        tests::{paths, record},
    };

    fn catalog() -> Arc<Vec<model::Game>> {
        Arc::new(["wand:1", "wand:2", "wand:3"].map(|id| record(id, id, "steam:1", None)).to_vec())
    }

    fn list(catalog: &Arc<Vec<model::Game>>, listed: &[&str]) -> Library {
        let games = listed.iter().map(|text| Arc::new(browse(catalog, &id(text), &paths()).unwrap())).collect();
        Library { games, trainers: 0, catalog: catalog.clone() }
    }

    fn id(text: &str) -> Ref {
        Ref::parse(text).unwrap()
    }

    fn game(text: &str) -> GameId {
        GameId::of(&id(text))
    }

    #[test]
    fn a_browsed_record_installed_meanwhile_opens_as_listed() {
        let all = catalog();
        let (before, now) = (list(&all, &["wand:1"]), list(&all, &["wand:1", "wand:2"]));
        for opens in [true, false] {
            let landing = now.land(Some(&before), Some(&game("wand:1")), Some((id("wand:2"), opens)));
            assert_eq!(landing, Landing::Listed(game("wand:2")));
        }
    }

    #[test]
    fn a_browse_still_not_listed_is_made_again_only_from_a_new_catalog() {
        let all = catalog();
        let before = list(&all, &["wand:1", "wand:2"]);
        for opens in [true, false] {
            let browsed = Some((id("wand:3"), opens));
            let open = Some(game("wand:1"));
            let landing = list(&catalog(), &["wand:1"]).land(Some(&before), open.as_ref(), browsed.clone());
            assert_eq!(landing, Landing::Kept { open: open.clone(), browse: browsed.clone() });
            // A game forgotten: the same catalog, another list.
            let landing = list(&all, &["wand:2"]).land(Some(&before), open.as_ref(), browsed);
            assert_eq!(landing, Landing::Kept { open: Some(game("wand:2")), browse: None });
        }
    }

    #[test]
    fn the_open_game_stays_while_listed_else_the_first_opens() {
        let all = catalog();
        let now = list(&all, &["wand:2", "wand:3"]);
        assert_eq!(now.land(None, Some(&game("wand:3")), None), Landing::Kept { open: Some(game("wand:3")), browse: None });
        assert_eq!(now.land(None, Some(&game("wand:1")), None), Landing::Kept { open: Some(game("wand:2")), browse: None });
        assert_eq!(list(&all, &[]).land(None, Some(&game("wand:1")), None), Landing::Kept { open: None, browse: None });
    }
}
