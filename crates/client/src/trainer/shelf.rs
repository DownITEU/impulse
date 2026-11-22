//! The sheets by game: which copy shows, and the newer one that waits while a run holds its game.

use std::collections::HashMap;

use model::{Failure, Ref};

use super::Sheet;
use crate::{
    Load,
    library::{GameId, Stamp},
    profile::Profiles,
};

#[derive(Default)]
pub struct Shelf {
    shown: HashMap<GameId, Load<Sheet>>,
    /// A run's variables come from the shown list, so it stays while held. A list: a late release never frees a newer hold.
    held: Vec<GameId>,
    waiting: HashMap<GameId, Sheet>,
}

impl Shelf {
    pub fn get(&self, game: &GameId) -> Option<&Load<Sheet>> {
        self.shown.get(game)
    }

    pub fn sheet(&self, game: &GameId) -> Option<&Sheet> {
        ready(self.shown.get(game)?)
    }

    pub fn sheet_mut(&mut self, game: &GameId) -> Option<&mut Sheet> {
        match self.shown.get_mut(game)? {
            Load::Ready(sheet) => Some(sheet),
            _ => None,
        }
    }

    pub fn sheets(&self) -> impl Iterator<Item = &Sheet> {
        self.shown.values().filter_map(ready)
    }

    /// The newest copy's trainer, waiting or shown.
    pub fn trainer(&self, game: &GameId) -> Option<&Ref> {
        self.waiting.get(game).or_else(|| self.sheet(game)).map(|sheet| &sheet.trainer)
    }

    /// The newest copy, waiting or shown, is of `stamp` and needs no refresh.
    pub fn is_current(&self, game: &GameId, stamp: &Stamp) -> bool {
        self.waiting.get(game).or_else(|| self.sheet(game)).is_some_and(|sheet| !sheet.stale && sheet.stamp == *stamp)
    }

    /// Shows `sheet` unless a run holds the game: it then waits. `true` when shown.
    pub fn put(&mut self, game: GameId, sheet: Sheet) -> bool {
        if self.held.contains(&game) {
            self.waiting.insert(game, sheet);
            return false;
        }
        self.shown.insert(game, Load::Ready(sheet));
        true
    }

    /// No copy on disk: the first download runs.
    pub fn pending(&mut self, game: GameId) {
        self.shown.entry(game).or_insert(Load::Pending);
    }

    /// A stale copy still beats an error, and the next open asks again.
    pub fn failed(&mut self, game: GameId, error: Failure) {
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

    /// The copies of `game`, or of every game, ask their source again at their next open: the content language changed.
    pub fn outdate(&mut self, game: Option<&GameId>) {
        let shown = self.shown.iter_mut().filter_map(|(id, load)| match load {
            Load::Ready(sheet) => Some((id, sheet)),
            _ => None,
        });
        for (_, sheet) in shown.chain(self.waiting.iter_mut()).filter(|(id, _)| game.is_none_or(|game| game == *id)) {
            sheet.stale = true;
        }
    }

    /// Another trainer was chosen for `game`: its copies go, but a run's held one stays. Whether a shown one went.
    pub fn forget(&mut self, game: &GameId) -> bool {
        self.waiting.remove(game);
        !self.held.contains(game) && self.shown.remove(game).is_some()
    }

    pub fn hold(&mut self, game: GameId) {
        self.held.push(game);
    }

    /// One hold less. With none left the waiting copy shows, with the pins and folds set while it waited: `true`.
    pub fn release(&mut self, game: &GameId, profiles: &Profiles) -> bool {
        if let Some(at) = self.held.iter().position(|held| held == game) {
            self.held.swap_remove(at);
        }
        if self.held.contains(game) {
            return false;
        }
        let Some((game, mut sheet)) = self.waiting.remove_entry(game) else { return false };
        sheet.restore(profiles.trainer(&sheet.trainer));
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
