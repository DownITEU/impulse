//! Choices made in Settings, kept between runs in the data directory.

use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::disk;

const FILE: &str = "preferences.json";

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Preferences {
    /// The Wand folder the user chose; `None` follows the newest installed version.
    pub wand_dir: Option<PathBuf>,
}

/// A missing or unreadable file means the defaults: settings must never stop the app.
pub fn load(dir: &Path) -> Preferences {
    let Ok(bytes) = fs::read(dir.join(FILE)) else {
        return Preferences::default();
    };
    serde_json::from_slice(&bytes).unwrap_or_else(|e| {
        log::warn!("{FILE} ignored: {e}");
        Preferences::default()
    })
}

pub fn save(dir: &Path, preferences: &Preferences) -> Result<()> {
    disk::replace(&dir.join(FILE), &serde_json::to_vec_pretty(preferences)?)
}

#[cfg(test)]
mod tests {
    use std::env;

    use super::*;

    #[test]
    fn saved_choices_come_back_and_damage_means_defaults() {
        let dir = env::temp_dir().join(format!("impulse-prefs-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        assert_eq!(load(&dir), Preferences::default());

        let chosen = Preferences { wand_dir: Some(PathBuf::from(r"C:\Wand\app-12.72.0")) };
        save(&dir, &chosen).unwrap();
        assert_eq!(load(&dir), chosen);

        fs::write(dir.join(FILE), b"{ not json").unwrap();
        assert_eq!(load(&dir), Preferences::default());
        fs::write(dir.join(FILE), br#"{"somethingNew": 1}"#).unwrap();
        assert_eq!(load(&dir), Preferences::default());
        fs::remove_dir_all(&dir).unwrap();
    }
}
