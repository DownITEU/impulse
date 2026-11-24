//! The search: a row per listed game and per title with a trainer the list does not hold, ranked as the user types.

use std::collections::{HashMap, HashSet};

use model::Ref;

use super::{Library, shares};

/// A row of the search.
pub struct Find {
    /// The record, into `Library::catalog`.
    pub at: usize,
    pub listed: bool,
    /// Lower case: the name, then each of the title's other names, a line each.
    pub search: String,
}

impl Library {
    /// The search's rows by name: the listed games, then for each other title with a trainer its Steam record, else
    /// its first with a trainer. A title with an edition of a listed install is that install's: the game stands for it.
    pub fn finds(&self) -> Vec<Find> {
        let listed: HashSet<&Ref> = self.games.iter().map(|game| &game.source.id).collect();
        let installed: Vec<_> = self.games.iter().filter_map(|game| game.install.as_ref()?.origin.store_id()).collect();
        let editions = self.catalog.iter().filter(|record| installed.iter().any(|id| shares(record, id)));
        let listed_titles: HashSet<&str> =
            self.games.iter().map(|game| game.source.title_id.as_str()).chain(editions.map(|record| record.title_id.as_str())).collect();
        let steam = |at: usize| self.catalog[at].store_ids.iter().any(|id| id.steam_app().is_some());
        let mut titles = HashMap::<&str, usize>::new();
        let mut rows = Vec::new();
        for (at, record) in self.catalog.iter().enumerate() {
            if listed.contains(&record.id) {
                rows.push((at, true));
            } else if record.trainer.is_some() && !listed_titles.contains(record.title_id.as_str()) {
                let best = titles.entry(&record.title_id).or_insert(at);
                if !steam(*best) && steam(at) {
                    *best = at;
                }
            }
        }
        rows.extend(titles.into_values().map(|at| (at, false)));
        let mut finds: Vec<Find> = rows.into_iter().map(|(at, listed)| Find { at, listed, search: search(&self.catalog[at]) }).collect();
        finds.sort_by(|a, b| a.search.cmp(&b.search).then(a.at.cmp(&b.at)));
        finds
    }
}

fn search(record: &model::Game) -> String {
    let mut search = record.name.to_lowercase();
    for term in &record.terms {
        search.push('\n');
        search.push_str(&term.to_lowercase());
    }
    search
}

/// The rows `query` finds, by index, best first: a name it starts, a word of a name it starts, a name holding it,
/// another name holding it; listed games first among equals, then by name. None for a blank query.
pub fn rank(finds: &[Find], query: &str) -> Vec<usize> {
    let query = query.trim().to_lowercase();
    if query.is_empty() {
        return Vec::new();
    }
    let class = |find: &Find| {
        let (name, terms) = find.search.split_once('\n').unwrap_or((&find.search, ""));
        if name.starts_with(&query) {
            Some(0)
        } else if name.split(|c: char| !c.is_alphanumeric()).any(|word| word.starts_with(&query)) {
            Some(1)
        } else if name.contains(&query) {
            Some(2)
        } else {
            terms.contains(&query).then_some(3)
        }
    };
    let mut ranked: Vec<(u8, bool, usize)> =
        finds.iter().enumerate().filter_map(|(at, find)| Some((class(find)?, !find.listed, at))).collect();
    ranked.sort_unstable();
    ranked.into_iter().map(|(.., at)| at).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn find(search: &str, listed: bool) -> Find {
        Find { at: 0, listed, search: search.into() }
    }

    #[test]
    fn a_name_it_starts_ranks_over_a_word_a_word_over_a_part_and_another_name_last() {
        let finds = [
            find("gears 5", false),
            find("gears of war 4\ngow4\ngears 4", false),
            find("hogwarts legacy", false),
            find("war gears", false),
        ];
        assert_eq!(rank(&finds, "gears"), [0, 1, 3]);
        assert_eq!(rank(&finds, "war"), [3, 1, 2]);
        assert_eq!(rank(&finds, "gow"), [1]);
        assert_eq!(rank(&finds, "  "), Vec::<usize>::new());
    }

    #[test]
    fn a_listed_game_comes_first_among_equals() {
        let finds = [find("doom", false), find("doom eternal", true)];
        assert_eq!(rank(&finds, "DOOM"), [1, 0]);
    }
}
