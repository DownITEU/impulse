//! `hotkeys.json`: the user's own hotkey choices over the provider's, and where hotkeys fire.

use std::{
    collections::{BTreeMap, HashMap},
    fs,
    path::{Path, PathBuf},
};

use hotkeys::Chord;
use model::Cheat;
use serde::{Deserialize, Serialize};

const FILE: &str = "hotkeys.json";

/// Slot: VKs, or `null` for unbound; a missing slot is the provider's.
type Slots = BTreeMap<usize, Option<Vec<u16>>>;

/// What `choose` sets a slot to.
pub enum Choice {
    Bound(Chord),
    Unbound,
    /// Back to the provider's binding.
    Default,
}

#[derive(Default, Serialize, Deserialize)]
#[serde(default)]
struct Saved {
    /// Hotkeys fire only while the game owns the foreground window.
    game_only: bool,
    /// By game id, then cheat id.
    games: HashMap<String, HashMap<String, Slots>>,
}

pub struct Store {
    file: PathBuf,
    saved: Saved,
}

impl Store {
    /// A missing or damaged file means no choices. Blocking, and so are the saves: the file is small
    /// and changes only on a click, and writing in place keeps the saves in order.
    pub fn load(data_dir: &Path) -> Self {
        let file = data_dir.join(FILE);
        let saved = fs::read(&file).ok().and_then(|raw| serde_json::from_slice(&raw).ok()).unwrap_or_default();
        Self { file, saved }
    }

    pub fn game_only(&self) -> bool {
        self.saved.game_only
    }

    pub fn set_game_only(&mut self, on: bool) {
        self.saved.game_only = on;
        self.save();
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
        let cheats = self.saved.games.entry(game.to_owned()).or_default();
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
            self.saved.games.remove(game);
        }
        self.save();
    }

    fn save(&self) {
        let written = serde_json::to_vec_pretty(&self.saved)
            .map_err(anyhow::Error::from)
            .and_then(|json| disk::replace(&self.file, &json));
        if let Err(e) = written {
            log::warn!("{FILE} not saved: {e:#}");
        }
    }
}

#[cfg(test)]
mod tests {
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
        let mut store = Store::load(&dir);
        assert_eq!(store.slots("g", &cheat()), [Chord::from_vks(&[112]), None]);
        store.choose("g", "a", 0, Choice::Unbound);
        store.choose("g", "a", 1, Choice::Bound(Chord::from_vks(&[17, 113]).unwrap()));
        store.set_game_only(true);
        let reloaded = Store::load(&dir);
        assert_eq!(reloaded.slots("g", &cheat()), [None, Chord::from_vks(&[17, 113])]);
        assert!(reloaded.game_only());
        store.choose("g", "a", 0, Choice::Default);
        store.choose("g", "a", 1, Choice::Default);
        assert!(Store::load(&dir).saved.games.is_empty());
        store.choose("g", "a", 1, Choice::Bound(Chord::from_vks(&[112]).unwrap()));
        assert_eq!(store.slots("g", &cheat()), [None, Chord::from_vks(&[112])]);
        fs::remove_dir_all(&dir).unwrap();
    }
}
