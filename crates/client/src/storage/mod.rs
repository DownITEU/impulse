//! The app's own files in the data folder: `Paths` names them, `Stored` keeps one of them typed.
//! Blocking, in place and in order: the files are tiny and change on a click.

mod keys;
mod migrate;

use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

pub use disk::{Stored, VERSION, read, write};
use model::SourceId;

pub use keys::{by_ref, name};
pub use migrate::migrate;

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
    installs: "installs.json",
    docks: "docks.json",
    overlay: "overlay.json",
    minimap: "minimap.json",
    resources: "resources.json",
    remote_devices: "remote.json",
    map_links: "map-links.json",
    trainer_links: "trainer-links.json",
    map_state: "map-state",
    pictures: "art",
    tiles: "tiles",
    videos: "videos",
    plugins: "plugins",
    plugin_lock: "plugins.lock",
    plugin_cache: "plugin-cache",
    update: "update.json",
}

impl Paths {
    pub fn new(root: PathBuf) -> Self {
        Self(root.into())
    }
}

/// A source's folder of map states, under `map_state`; `v1`: the v1 files `migrate` moved there.
pub fn maps_of(map_state: &Path, source: &SourceId, v1: bool) -> PathBuf {
    let dir = map_state.join(name(source.as_str()));
    if v1 { dir.join("v1") } else { dir }
}
