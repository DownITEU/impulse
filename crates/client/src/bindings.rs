//! `hotkeys.json`: the user's own hotkey choices over the provider's, and whether and where hotkeys fire.

use std::collections::{BTreeMap, HashMap};

use model::{Cheat, Chord};
use serde::{Deserialize, Serialize};

use crate::storage::{Paths, Stored};

/// Slot: VKs, or `null` for unbound; a missing slot is the provider's.
type Slots = BTreeMap<usize, Option<Vec<u16>>>;

/// What `choose` sets a slot to.
pub enum Choice {
    Bound(Chord),
    Unbound,
    /// Back to the provider's binding.
    Default,
}

#[derive(Serialize, Deserialize)]
#[serde(default)]
struct Saved {
    /// Hotkeys fire during a run.
    enabled: bool,
    /// Hotkeys fire only while the game owns the foreground window.
    game_only: bool,
    /// By game id, then cheat id.
    games: HashMap<String, HashMap<String, Slots>>,
}

impl Default for Saved {
    fn default() -> Self {
        Self { enabled: true, game_only: false, games: HashMap::new() }
    }
}

pub struct Store {
    saved: Stored<Saved>,
}

impl Store {
    /// A missing or damaged file means no choices.
    pub fn load(paths: &Paths) -> Self {
        Self { saved: Stored::load(paths.hotkeys()) }
    }

    pub fn enabled(&self) -> bool {
        self.saved.enabled
    }

    pub fn set_enabled(&mut self, on: bool) {
        self.saved.edit(|saved| saved.enabled = on);
    }

    pub fn game_only(&self) -> bool {
        self.saved.game_only
    }

    pub fn set_game_only(&mut self, on: bool) {
        self.saved.edit(|saved| saved.game_only = on);
    }

    /// The cheat's slots: the provider's bindings with the user's choices laid over them. A default
    /// holding the keys of a choice is dropped: a newer cheat list may bring it after the choice.
    pub fn slots(&self, game: &str, cheat: &Cheat) -> Vec<Option<Chord>> {
        let chosen = self.saved.games.get(game).and_then(|cheats| cheats.get(&cheat.id));
        let choice = |slot: usize| chosen.and_then(|slots| slots.get(&slot)).map(|vks| vks.as_deref().and_then(Chord::from_vks));
        let choices: Vec<_> = (0..cheat.kind.slots()).map(choice).collect();
        let chosen_keys = |chord: Chord| choices.iter().flatten().flatten().any(|other| other.same_keys(chord));
        choices
            .iter()
            .enumerate()
            .map(|(slot, choice)| match choice {
                Some(chord) => *chord,
                None => cheat.hotkeys.get(slot).copied().flatten().filter(|chord| !chosen_keys(*chord)),
            })
            .collect()
    }

    pub fn choose(&mut self, game: &str, cheat: &str, slot: usize, choice: Choice) {
        self.saved.edit(|saved| {
            let cheats = saved.games.entry(game.to_owned()).or_default();
            let slots = cheats.entry(cheat.to_owned()).or_default();
            match choice {
                Choice::Bound(chord) => slots.insert(slot, Some(chord.vks().map(u16::from).collect())),
                Choice::Unbound => slots.insert(slot, None),
                Choice::Default => slots.remove(&slot),
            };
            if slots.is_empty() {
                cheats.remove(cheat);
            }
            if cheats.is_empty() {
                saved.games.remove(game);
            }
        });
    }

    /// Sets a slot of the cheat unless its chord is taken; the cheat's new slots. As in Wand, a cheat never
    /// holds one chord twice, nor one `reserved` elsewhere.
    pub fn rebind(
        &mut self,
        game: &str,
        cheat: &Cheat,
        slot: usize,
        choice: Choice,
        reserved: impl Fn(Chord) -> bool,
    ) -> Option<Vec<Option<Chord>>> {
        let wanted = match choice {
            Choice::Bound(chord) => Some(chord),
            Choice::Unbound => None,
            Choice::Default => cheat.hotkeys.get(slot).copied().flatten(),
        };
        let slots = self.slots(game, cheat);
        let other =
            |chord: Chord| slots.iter().enumerate().any(|(at, bound)| at != slot && bound.is_some_and(|bound| bound.same_keys(chord)));
        if wanted.is_some_and(|chord| reserved(chord) || other(chord)) {
            return None;
        }
        self.choose(game, &cheat.id, slot, choice);
        Some(self.slots(game, cheat))
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use model::{Args, Kind};

    use super::*;

    fn cheat() -> Cheat {
        Cheat {
            id: "a".into(),
            key: None,
            name: String::new(),
            category: String::new(),
            kind: Kind::Number,
            args: Args::default(),
            hotkeys: vec![Chord::from_vks(&[112])],
            instructions: None,
            description: None,
        }
    }

    #[test]
    fn choices_lay_over_the_provider_and_survive_a_reload() {
        let dir = std::env::temp_dir().join(format!("impulse-hotkeys-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let paths = Paths::new(dir.clone());
        let mut store = Store::load(&paths);
        assert!(store.enabled());
        assert_eq!(store.slots("g", &cheat()), [Chord::from_vks(&[112]), None]);
        store.choose("g", "a", 0, Choice::Unbound);
        store.choose("g", "a", 1, Choice::Bound(Chord::from_vks(&[17, 113]).unwrap()));
        store.set_game_only(true);
        store.set_enabled(false);
        let reloaded = Store::load(&paths);
        assert_eq!(reloaded.slots("g", &cheat()), [None, Chord::from_vks(&[17, 113])]);
        assert!(reloaded.game_only() && !reloaded.enabled());
        store.choose("g", "a", 0, Choice::Default);
        store.choose("g", "a", 1, Choice::Default);
        assert!(Store::load(&paths).saved.games.is_empty());
        store.choose("g", "a", 1, Choice::Bound(Chord::from_vks(&[112]).unwrap()));
        assert_eq!(store.slots("g", &cheat()), [None, Chord::from_vks(&[112])]);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_chord_held_elsewhere_or_reserved_is_refused() {
        let dir = std::env::temp_dir().join(format!("impulse-rebind-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let mut store = Store::load(&Paths::new(dir.clone()));
        let (f1, f2) = (Chord::from_vks(&[112]).unwrap(), Chord::from_vks(&[113]).unwrap());
        assert_eq!(store.rebind("g", &cheat(), 1, Choice::Bound(f1), |_| false), None);
        assert_eq!(store.rebind("g", &cheat(), 1, Choice::Bound(f2), |chord| chord == f2), None);
        assert_eq!(store.rebind("g", &cheat(), 0, Choice::Bound(f1), |_| false), Some(vec![Some(f1), None]));
        assert_eq!(store.rebind("g", &cheat(), 1, Choice::Bound(f2), |_| false), Some(vec![Some(f1), Some(f2)]));
        assert_eq!(store.rebind("g", &cheat(), 0, Choice::Bound(f2), |_| false), None);
        assert_eq!(store.rebind("g", &cheat(), 0, Choice::Unbound, |_| true), Some(vec![None, Some(f2)]));
        fs::remove_dir_all(&dir).unwrap();
    }
}
