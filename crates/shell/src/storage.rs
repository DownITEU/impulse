//! The app's own files in the data folder: `Paths` names them, `Stored` keeps one of them typed.
//! Blocking, in place and in order: the files are tiny and change on a click.

use std::{
    fs,
    ops::Deref,
    path::{Path, PathBuf},
    sync::Arc,
};

use serde::{Serialize, de::DeserializeOwned};

/// The data folder: the only place that names the app's own files and folders.
#[derive(Clone)]
pub struct Paths(Arc<Path>);

macro_rules! entries {
    ($($name:ident: $entry:literal),* $(,)?) => {
        impl Paths {
            $(pub fn $name(&self) -> PathBuf {
                self.0.join($entry)
            })*
        }
    };
}

entries! {
    settings: "settings.json",
    hotkeys: "hotkeys.json",
    games: "games.json",
    picked: "picked.json",
    docks: "docks.json",
    overlay: "overlay.json",
    minimap: "minimap.json",
    resources: "resources.json",
    map_state: "map-state",
    pictures: "art",
    tiles: "tiles",
    videos: "videos",
}

impl Paths {
    pub fn new(root: PathBuf) -> Self {
        Self(root.into())
    }
}

/// A typed JSON file, read once and written whole on every edit. Missing or damaged means the defaults.
#[derive(Clone)]
pub struct Stored<T> {
    file: PathBuf,
    value: T,
}

impl<T: Serialize + DeserializeOwned + Default> Stored<T> {
    pub fn load(file: PathBuf) -> Self {
        Self { value: read(&file).unwrap_or_default(), file }
    }

    /// Changes the value and saves it.
    pub fn edit<R>(&mut self, change: impl FnOnce(&mut T) -> R) -> R {
        let changed = change(&mut self.value);
        write(&self.file, &self.value);
        changed
    }
}

impl<T> Deref for Stored<T> {
    type Target = T;

    fn deref(&self) -> &T {
        &self.value
    }
}

/// `None` for a missing or damaged file.
pub fn read<T: DeserializeOwned>(file: &Path) -> Option<T> {
    serde_json::from_slice(&fs::read(file).ok()?).ok()
}

/// A failed save is logged: the change holds until the app closes.
pub fn write<T: Serialize>(file: &Path, value: &T) {
    let written = serde_json::to_vec_pretty(value).map_err(anyhow::Error::from).and_then(|json| disk::replace(file, &json));
    if let Err(e) = written {
        log::warn!("{} not saved: {e:#}", file.display());
    }
}
