//! The app's own files in the data folder: `Paths` names them, `Stored` keeps one of them typed.
//! Blocking, in place and in order: the files are tiny and change on a click.

mod keys;
mod migrate;

use std::{
    fs, io,
    ops::Deref,
    path::{Path, PathBuf},
    sync::Arc,
};

use model::SourceId;
use serde::{Serialize, de::DeserializeOwned};
use serde_json::Value;

pub use keys::{by_ref, name};
pub use migrate::migrate;

/// What `Stored` writes into an object; an object without it is not this build's.
const VERSION: u64 = 2;

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
    remote_devices: "remote.json",
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

/// A source's folder of map states, under `map_state`; `v1`: the v1 files `migrate` moved there.
pub fn maps_of(map_state: &Path, source: &SourceId, v1: bool) -> PathBuf {
    let dir = map_state.join(name(source.as_str()));
    if v1 { dir.join("v1") } else { dir }
}

/// A typed JSON file, read once and written whole on every edit. Missing means the defaults; not JSON is
/// moved to `<name>.damaged.json`, then the defaults. A file this build cannot read gives the defaults and is
/// never written: an older one waits for `migrate`, a newer one for its build.
#[derive(Clone)]
pub struct Stored<T> {
    file: PathBuf,
    value: T,
    saves: bool,
}

impl<T: Serialize + DeserializeOwned + Default> Stored<T> {
    pub fn load(file: PathBuf) -> Self {
        let (value, saves) = match fs::read(&file) {
            Ok(bytes) => match serde_json::from_slice::<Value>(&bytes) {
                Ok(json) => match current(json).and_then(|json| serde_json::from_value(json).ok()) {
                    Some(value) => (value, true),
                    None => {
                        log::warn!("{} is not this version's: kept as it is, changes are not saved", file.display());
                        (T::default(), false)
                    }
                },
                Err(_) => {
                    let aside = file.with_extension("damaged.json");
                    if let Err(e) = fs::rename(&file, &aside) {
                        log::warn!("{} not moved aside: {e}", file.display());
                    }
                    (T::default(), true)
                }
            },
            Err(e) if e.kind() == io::ErrorKind::NotFound => (T::default(), true),
            Err(e) => {
                log::warn!("{} not read: {e}", file.display());
                (T::default(), false)
            }
        };
        Self { file, value, saves }
    }

    /// Changes the value and saves it.
    pub fn edit<R>(&mut self, change: impl FnOnce(&mut T) -> R) -> R {
        let changed = change(&mut self.value);
        if self.saves {
            let json = serde_json::to_value(&self.value).map(|mut json| {
                if let Value::Object(object) = &mut json {
                    object.insert("version".into(), VERSION.into());
                }
                json
            });
            match json {
                Ok(json) => write(&self.file, &json),
                Err(e) => log::warn!("{} not saved: {e}", self.file.display()),
            }
        }
        changed
    }
}

/// A list has no version; an object must carry this one.
fn current(json: Value) -> Option<Value> {
    match &json {
        Value::Object(object) if object.get("version").and_then(Value::as_u64) != Some(VERSION) => None,
        _ => Some(json),
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

#[cfg(test)]
mod tests {
    use serde::Deserialize;

    use super::*;

    #[derive(Default, Serialize, Deserialize)]
    #[serde(default)]
    struct Note {
        text: String,
    }

    fn dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("impulse-stored-{name}-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn edited(file: &Path) -> String {
        Stored::<Note>::load(file.to_owned()).edit(|note| note.text = "new".into());
        fs::read_to_string(file).unwrap()
    }

    #[test]
    fn an_object_carries_the_version_and_a_list_none() {
        let dir = dir("version");
        let file = dir.join("note.json");
        assert!(edited(&file).contains("\"version\": 2"));
        assert_eq!(Stored::<Note>::load(file).text, "new");
        let list = dir.join("list.json");
        Stored::<Vec<u32>>::load(list.clone()).edit(|list| list.push(1));
        assert_eq!(Stored::<Vec<u32>>::load(list).as_slice(), [1]);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn an_older_or_newer_file_is_never_written_and_a_damaged_one_moves_aside() {
        let dir = dir("policy");
        for (name, text) in [("v1", r#"{"text":"old"}"#), ("v3", r#"{"version":3,"text":"later"}"#), ("odd", r#"{"version":2,"text":7}"#)] {
            let file = dir.join(format!("{name}.json"));
            fs::write(&file, text).unwrap();
            assert_eq!(edited(&file), text, "{name}");
        }
        let damaged = dir.join("damaged.json");
        fs::write(&damaged, "{").unwrap();
        assert!(edited(&damaged).contains("new"));
        assert_eq!(fs::read_to_string(dir.join("damaged.damaged.json")).unwrap(), "{");
        fs::remove_dir_all(dir).unwrap();
    }
}
