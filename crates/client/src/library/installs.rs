//! `installs.json`: the last scan's installs. The list shows from them at start while the launchers are read again.

use std::path::PathBuf;

use model::{Installation, Origin, Ref, StoreId};
use serde::{Deserialize, Serialize};

use crate::storage::{Paths, Stored};

#[derive(Default, Serialize, Deserialize)]
#[serde(default)]
struct Installs {
    installs: Vec<Install>,
}

/// An `Installation` with its ids as text.
#[derive(PartialEq, Serialize, Deserialize)]
struct Install {
    game: String,
    origin: Kept,
    location: PathBuf,
    build: Option<u32>,
}

#[derive(PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
enum Kept {
    Launcher(String),
    Picked(PathBuf),
}

impl Install {
    fn of(install: &Installation) -> Self {
        let origin = match &install.origin {
            Origin::Launcher(id) => Kept::Launcher(id.as_str().to_owned()),
            Origin::Picked(exe) => Kept::Picked(exe.clone()),
        };
        Self { game: install.game.to_string(), origin, location: install.location.clone(), build: install.build }
    }

    fn installation(&self) -> Option<Installation> {
        let origin = match &self.origin {
            Kept::Launcher(id) => Origin::Launcher(StoreId::parse(id)?),
            Kept::Picked(exe) => Origin::Picked(exe.clone()),
        };
        Some(Installation { game: Ref::parse(&self.game)?, origin, location: self.location.clone(), build: self.build })
    }
}

/// The last scan's installs; none before the first (blocking).
pub(super) fn kept(paths: &Paths) -> Vec<Installation> {
    Stored::<Installs>::load(paths.installs()).installs.iter().filter_map(Install::installation).collect()
}

/// Keeps `installed` for the next start; an unchanged list is not written again (blocking).
pub(super) fn keep(paths: &Paths, installed: &[Installation]) {
    let installs: Vec<Install> = installed.iter().map(Install::of).collect();
    let mut file = Stored::<Installs>::load(paths.installs());
    if file.installs != installs {
        file.edit(|file| file.installs = installs);
    }
}

#[cfg(test)]
mod tests {
    use std::{env, fs, process};

    use model::SourceId;

    use super::*;

    #[test]
    fn the_last_scan_is_kept_for_the_next_start() {
        let dir = env::temp_dir().join(format!("impulse-installs-{}", process::id()));
        let paths = Paths::new(dir.clone());
        assert!(kept(&paths).is_empty());
        let game = Ref::new(&SourceId::new("wand").unwrap(), "82102");
        let launcher = Installation {
            game: game.clone(),
            origin: Origin::Launcher(StoreId::parse("steam:1203620").unwrap()),
            location: r"C:\Games\Enshrouded".into(),
            build: Some(1793791052),
        };
        let picked = Installation { game, origin: Origin::Picked(r"D:\Copy\game.exe".into()), location: r"D:\Copy".into(), build: None };
        keep(&paths, &[launcher.clone(), picked.clone()]);
        let again = kept(&paths);
        assert_eq!(again.len(), 2);
        assert!(again.iter().zip([&launcher, &picked]).all(|(a, b)| a.game == b.game && a.origin == b.origin && a.location == b.location));
        assert_eq!((again[0].build, again[1].build), (Some(1793791052), None));
        fs::remove_dir_all(&dir).unwrap();
    }
}
