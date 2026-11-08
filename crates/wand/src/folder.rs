//! Which Wand install the trainer binaries come from: the user's choice in `wand.json`, else the newest.

use std::{
    fs,
    path::{Path, PathBuf},
    sync::{
        Mutex, MutexGuard, PoisonError,
        atomic::{AtomicU64, Ordering},
    },
};

use model::Folder;
use serde::{Deserialize, Serialize};

use crate::install::{self, Install};

pub const ID: &str = "wand";
const FILE: &str = "wand.json";

#[derive(Default, Serialize, Deserialize)]
#[serde(default)]
struct Saved {
    /// `None` follows the newest installed version.
    wand_dir: Option<PathBuf>,
}

pub struct Setting {
    file: PathBuf,
    // Held while saving, so writes happen in the order they are applied.
    saved: Mutex<Option<PathBuf>>,
    calls: AtomicU64,
}

impl Setting {
    /// A missing or damaged file means the default: settings never stop the app.
    pub fn load(data_dir: &Path) -> Self {
        let file = data_dir.join(FILE);
        let saved = fs::read(&file).ok().and_then(|bytes| {
            serde_json::from_slice::<Saved>(&bytes).inspect_err(|e| log::warn!("{FILE} ignored: {e}")).ok()
        });
        Self { file, saved: Mutex::new(saved.unwrap_or_default().wand_dir), calls: AtomicU64::new(0) }
    }

    /// The install runs read from (blocking). A saved choice Squirrel removed falls back to the newest.
    pub fn install(&self) -> Option<Install> {
        let saved = self.saved().clone();
        saved.and_then(|dir| install::resolve(&dir)).or_else(|| install::installs().into_iter().next())
    }

    /// What Settings shows (blocking); a choice that no longer resolves is shown as the newest, not erased.
    pub fn folder(&self) -> Folder {
        let installs = install::installs();
        let saved = self.saved().clone();
        let path = saved.filter(|dir| install::resolve(dir).is_some()).or_else(|| installs.first().map(|install| install.root.clone()));
        let found = path.as_deref().and_then(install::resolve);
        describe(path, found, installs)
    }

    /// Numbers a call as it is made, before the blocking work runs.
    pub fn call(&self) -> u64 {
        self.calls.fetch_add(1, Ordering::SeqCst) + 1
    }

    /// Checks `path` and saves it (blocking), unless a newer call was made meanwhile. The newest
    /// install is the default: choosing it, or a folder in it, clears the choice to keep following updates.
    pub fn set(&self, call: u64, path: PathBuf) -> Folder {
        let installs = install::installs();
        let found = install::resolve(&path);
        if let Some(found) = &found {
            let wanted = (Some(&found.root) != installs.first().map(|install| &install.root)).then(|| path.clone());
            let mut saved = self.saved();
            if call == self.calls.load(Ordering::SeqCst) && *saved != wanted {
                let json = serde_json::to_vec_pretty(&Saved { wand_dir: wanted.clone() });
                match json.map_err(anyhow::Error::from).and_then(|json| disk::replace(&self.file, &json)) {
                    Ok(()) => *saved = wanted,
                    Err(e) => log::warn!("{FILE} not saved: {e:#}"),
                }
            }
        }
        describe(Some(path), found, installs)
    }

    fn saved(&self) -> MutexGuard<'_, Option<PathBuf>> {
        self.saved.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

fn describe(path: Option<PathBuf>, found: Option<Install>, installs: Vec<Install>) -> Folder {
    Folder {
        id: ID,
        label: "WAND FOLDER",
        hint: "where the trainer binaries are read from",
        path,
        status: found.as_ref().map(|found| format!("WAND {}", found.label())).ok_or_else(|| "NO WAND TRAINER FILES IN THIS FOLDER".to_owned()),
        choices: installs.iter().map(|install| (install.label(), install.root.clone())).collect(),
        selected: found.map(|found| found.root),
    }
}

#[cfg(test)]
mod tests {
    use std::env;

    use super::*;

    #[test]
    fn a_damaged_file_means_the_default_and_an_older_call_never_saves() {
        let dir = env::temp_dir().join(format!("impulse-wand-folder-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join(FILE), b"{ not json").unwrap();
        let setting = Setting::load(&dir);
        assert_eq!(*setting.saved(), None);

        // A folder holding an install: `app-<version>` with both DLLs.
        let lib = dir.join(r"Wand\app-1.0.0").join(r"resources\app.asar.unpacked\static\unpacked\trainerlib");
        fs::create_dir_all(&lib).unwrap();
        for dll in ["tophat_service_x64.dll", "TrainerLibPlugin_x64.dll"] {
            fs::write(lib.join(dll), b"").unwrap();
        }
        let chosen = dir.join(r"Wand\app-1.0.0");
        let (older, newer) = (setting.call(), setting.call());
        setting.set(newer, dir.join("nothing here"));
        let folder = setting.set(older, chosen.clone());
        assert_eq!((folder.status.as_deref(), folder.selected.as_ref()), (Ok("WAND 1.0.0"), Some(&chosen)));
        assert!(!dir.join(FILE).exists() || Setting::load(&dir).saved().is_none());

        let latest = setting.call();
        setting.set(latest, chosen.clone());
        assert_eq!(*Setting::load(&dir).saved(), Some(chosen));
        fs::remove_dir_all(&dir).unwrap();
    }
}
