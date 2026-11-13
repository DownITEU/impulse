//! Executables the user picked for a game, as Wand's "game source".

use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
    slice,
};

use model::{Game, Installation};

use super::{App, installation};

/// Folders above a picked executable tried as the install: `Binaries\Win64\x.exe` is three down.
const DEPTH: usize = 5;

/// An executable the user picked for a game.
pub struct Custom {
    pub game_id: String,
    pub exe: PathBuf,
}

/// The games whose `exe` is in a folder at or above `exe`, with that folder, closest first (blocking).
pub fn owners<'a>(games: &'a [Game], exe: &Path) -> Vec<(&'a Game, PathBuf)> {
    // A folder's entries pick the few games to look for, by the first part of their `exe`.
    let mut by_head = HashMap::<String, Vec<&Game>>::new();
    for game in games {
        let Some(path) = game.exe.as_deref().filter(|path| !path.is_empty()) else { continue };
        let head = path.split(['\\', '/']).next().unwrap_or_default();
        by_head.entry(head.to_lowercase()).or_default().push(game);
    }
    let (mut found, mut seen) = (Vec::new(), HashSet::new());
    for dir in exe.ancestors().skip(1).take(DEPTH) {
        for entry in fs::read_dir(dir).into_iter().flatten().flatten() {
            let head = entry.file_name().to_string_lossy().to_lowercase();
            for game in by_head.get(&head).into_iter().flatten() {
                let path = game.exe.as_deref().unwrap_or_default();
                if dir.join(path).is_file() && seen.insert(game.id.as_str()) {
                    found.push((*game, dir.to_owned()));
                }
            }
        }
    }
    found
}

/// The install `exe` gives `game`: the folder that holds the game's `exe`, else the executable's own.
/// `None` once the file is gone (blocking).
pub fn install(game: &Game, exe: &Path) -> Option<Installation> {
    if !exe.is_file() {
        return None;
    }
    let location = owners(slice::from_ref(game), exe).pop().map(|(_, dir)| dir).or_else(|| exe.parent().map(Path::to_owned))?;
    Some(installation(game, &App { store_id: model::custom_store_id(exe), location }))
}

#[cfg(test)]
mod tests {
    use std::env;

    use super::*;

    fn game(id: &str, exe: &str) -> Game {
        Game { id: id.into(), exe: Some(exe.into()), ..Default::default() }
    }

    #[test]
    fn owners_are_found_above_the_executable() {
        let root = env::temp_dir().join(format!("impulse-custom-{}", std::process::id()));
        let bin = root.join("Binaries").join("Win64");
        fs::create_dir_all(&bin).unwrap();
        fs::write(bin.join("Game.exe"), []).unwrap();
        fs::write(root.join("GameAssembly.dll"), []).unwrap();
        let games = [
            game("deep", r"Binaries\Win64\game.exe"),
            game("unity", "GameAssembly.dll"),
            game("other", r"Binaries\Win64\Other.exe"),
            Game { id: "none".into(), ..Default::default() },
        ];
        let found: Vec<_> = owners(&games, &bin.join("Game.exe")).into_iter().map(|(game, dir)| (game.id.as_str(), dir)).collect();
        assert_eq!(found, [("deep", root.clone()), ("unity", root.clone())]);

        let installed = install(&games[0], &bin.join("Game.exe")).unwrap();
        assert_eq!(installed.location, root);
        assert_eq!(model::custom_exe(&installed.store_id), Some(bin.join("Game.exe").as_path()));
        // Without its `exe` around, the picked file's folder is the install.
        assert_eq!(install(&games[2], &bin.join("Game.exe")).unwrap().location, bin);
        assert!(install(&games[0], &bin.join("Gone.exe")).is_none());
        fs::remove_dir_all(root).unwrap();
    }
}
