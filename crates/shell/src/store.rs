//! The small JSON files of the data folder: read whole, written whole. Blocking: they are tiny.

use std::{fs, path::Path};

use serde::{Serialize, de::DeserializeOwned};

/// `None` for a missing or damaged file.
pub fn load<T: DeserializeOwned>(file: &Path) -> Option<T> {
    serde_json::from_slice(&fs::read(file).ok()?).ok()
}

/// A failed save is logged: the change holds until the app closes.
pub fn save<T: Serialize>(file: &Path, value: &T) {
    let written = serde_json::to_vec_pretty(value).map_err(anyhow::Error::from).and_then(|json| disk::replace(file, &json));
    if let Err(e) = written {
        log::warn!("{} not saved: {e:#}", file.display());
    }
}
