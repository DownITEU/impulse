//! The sheets by game: which copy shows, and the newer one that waits while a run holds its game.

use std::{collections::HashMap, sync::Arc};

use super::Sheet;
use crate::{Load, library::Stamp, profile::Profile};

#[derive(Default)]
pub struct Shelf {
    shown: HashMap<Arc<str>, Load<Sheet>>,
    /// A run's variables come from the shown list, so it stays while held. A list: a late release never frees a newer hold.
    held: Vec<Arc<str>>,
    waiting: HashMap<Arc<str>, Sheet>,
}

impl Shelf {
    pub fn get(&self, game: &str) -> Option<&Load<Sheet>> {
        self.shown.get(game)
    }

    pub fn sheet(&self, game: &str) -> Option<&Sheet> {
        ready(self.shown.get(game)?)
    }

    pub fn sheet_mut(&mut self, game: &str) -> Option<&mut Sheet> {
        match self.shown.get_mut(game)? {
            Load::Ready(sheet) => Some(sheet),
            _ => None,
        }
    }

    pub fn sheets(&self) -> impl Iterator<Item = &Sheet> {
        self.shown.values().filter_map(ready)
    }

    /// A shown copy of `stamp` that needs no refresh.
    pub fn is_current(&self, game: &str, stamp: &Stamp) -> bool {
        self.sheet(game).is_some_and(|sheet| !sheet.stale && sheet.stamp == *stamp)
    }

    /// Shows `sheet` unless a run holds the game: it then waits. `true` when shown.
    pub fn put(&mut self, game: Arc<str>, sheet: Sheet) -> bool {
        if self.held.contains(&game) {
            self.waiting.insert(game, sheet);
            return false;
        }
        self.shown.insert(game, Load::Ready(sheet));
        true
    }

    /// No copy on disk: the first download runs.
    pub fn pending(&mut self, game: Arc<str>) {
        self.shown.entry(game).or_insert(Load::Pending);
    }

    /// A stale copy still beats an error, and the next open asks again.
    pub fn failed(&mut self, game: Arc<str>, error: String) {
        if let Some(sheet) = self.waiting.get_mut(&game) {
            sheet.stale = true;
        }
        match self.sheet_mut(&game) {
            Some(sheet) => sheet.stale = true,
            None => {
                self.shown.insert(game, Load::Failed(error));
            }
        }
    }

    pub fn hold(&mut self, game: Arc<str>) {
        self.held.push(game);
    }

    /// One hold less. With none left the waiting copy shows, with the pins and folds set while it waited: `true`.
    pub fn release(&mut self, game: &str, profile: &Profile) -> bool {
        if let Some(at) = self.held.iter().position(|held| &**held == game) {
            self.held.swap_remove(at);
        }
        if self.held.iter().any(|held| &**held == game) {
            return false;
        }
        let Some((game, mut sheet)) = self.waiting.remove_entry(game) else { return false };
        sheet.restore(profile);
        self.put(game, sheet)
    }

    /// The copies go; the holds stay with their owners.
    pub fn clear(&mut self) {
        self.shown.clear();
        self.waiting.clear();
    }
}

fn ready(load: &Load<Sheet>) -> Option<&Sheet> {
    match load {
        Load::Ready(sheet) => Some(sheet),
        _ => None,
    }
}
