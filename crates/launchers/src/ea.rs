use std::path::PathBuf;

use model::Game;

use super::{
    App,
    registry::{self, LOCAL_MACHINE, View},
};

const UNINSTALL: &str = r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall";

/// EA app games. Its local records carry no offer ids the catalog knows,
/// so a game is recognised by its `exe` inside an EA install folder.
pub fn apps(games: &[Game]) -> Vec<App> {
    match_folders(games, &install_folders())
}

// EA's installer leaves `__Installer\installerdata.xml` in every game it installs.
fn install_folders() -> Vec<PathBuf> {
    let mut folders: Vec<PathBuf> = Vec::new();
    for view in [View::Wow64, View::Wow32] {
        let Some(root) = registry::open(LOCAL_MACHINE, UNINSTALL, view) else {
            continue;
        };
        for (_, entry) in registry::children(&root, view) {
            let Some(dir) = registry::string(&entry, "InstallLocation") else {
                continue;
            };
            let dir = PathBuf::from(dir.trim_matches('"'));
            if dir.join(r"__Installer\installerdata.xml").is_file()
                && !folders.iter().any(|f| super::same_path(f, &dir))
            {
                folders.push(dir);
            }
        }
    }
    folders
}

fn match_folders(games: &[Game], folders: &[PathBuf]) -> Vec<App> {
    let games: Vec<(&str, &str)> = games
        .iter()
        .filter_map(|game| {
            let id = game.store_ids.iter().find(|id| id.starts_with("origin:"))?;
            Some((id.as_str(), game.exe.as_deref()?))
        })
        .collect();
    folders
        .iter()
        .flat_map(|dir| {
            games
                .iter()
                .filter(|(_, exe)| dir.join(exe).is_file())
                .map(|(id, _)| App {
                    store_id: (*id).to_owned(),
                    location: dir.clone(),
                })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_origin_games_by_version_path() {
        let game = |store_id: &str, exe: &str| Game {
            store_ids: vec![store_id.to_owned()],
            exe: Some(exe.to_owned()),
            ..Game::default()
        };
        let games = [
            game("origin:Origin.OFR.50.0000001", r"bin\game.exe"),
            game("steam:1", r"bin\game.exe"),
            game("origin:Origin.OFR.50.0000003", "other.exe"),
        ];
        let dir = std::env::temp_dir().join(format!("impulse-ea-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("bin")).unwrap();
        std::fs::write(dir.join(r"bin\game.exe"), b"").unwrap();
        let found = match_folders(&games, std::slice::from_ref(&dir));
        std::fs::remove_dir_all(&dir).unwrap();
        let ids: Vec<_> = found.iter().map(|a| a.store_id.as_str()).collect();
        assert_eq!(ids, ["origin:Origin.OFR.50.0000001"]);
    }
}
