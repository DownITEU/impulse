//! `overlay.json`: the overlay hotkey.

use std::path::{Path, PathBuf};

use overlay::Key;
use serde::{Deserialize, Serialize};

use super::super::store;

const FILE: &str = "overlay.json";

#[derive(Default, Serialize, Deserialize)]
#[serde(default)]
struct Saved {
    /// Virtual keys; missing or unusable means the default.
    hotkey: Option<Vec<u8>>,
}

pub(super) struct Store {
    file: PathBuf,
    saved: Saved,
}

impl Store {
    /// A missing or damaged file means the defaults. Blocking, like the saves: the file is tiny.
    pub fn load(data_dir: &Path) -> Self {
        let file = data_dir.join(FILE);
        Self { saved: store::load(&file).unwrap_or_default(), file }
    }

    pub fn hotkey(&self) -> Key {
        self.saved.hotkey.as_ref().and_then(|vks| Key::from_vks(vks.iter().copied())).unwrap_or(Key::DEFAULT)
    }

    pub fn set_hotkey(&mut self, key: Key) {
        self.saved.hotkey = Some(key.vks().collect());
        store::save(&self.file, &self.saved);
    }
}
