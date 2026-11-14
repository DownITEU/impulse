//! `overlay.json`: the overlay's and the minimap's hotkeys.

use overlay::Key;
use serde::{Deserialize, Serialize};

use client::storage::{Paths, Stored};

#[derive(Default, Serialize, Deserialize)]
#[serde(default)]
struct Saved {
    /// Virtual keys; missing or unusable means the default.
    hotkey: Option<Vec<u8>>,
    /// The same; empty means none.
    minimap: Option<Vec<u8>>,
}

pub(super) struct Store {
    saved: Stored<Saved>,
}

impl Store {
    pub fn load(paths: &Paths) -> Self {
        Self { saved: Stored::load(paths.overlay()) }
    }

    pub fn hotkey(&self) -> Key {
        self.saved.hotkey.as_ref().and_then(|vks| Key::from_vks(vks.iter().copied())).unwrap_or(Key::DEFAULT)
    }

    pub fn set_hotkey(&mut self, key: Key) {
        self.saved.edit(|saved| saved.hotkey = Some(key.vks().collect()));
    }

    pub fn minimap(&self) -> Option<Key> {
        match &self.saved.minimap {
            Some(vks) if vks.is_empty() => None,
            vks => vks.as_ref().and_then(|vks| Key::from_vks(vks.iter().copied())).or(Some(Key::MINIMAP)),
        }
    }

    pub fn set_minimap(&mut self, key: Option<Key>) {
        self.saved.edit(|saved| saved.minimap = Some(key.map_or_else(Vec::new, |key| key.vks().collect())));
    }
}
