use std::path::PathBuf;

use serde_json::json;

use super::*;

fn folder(name: &str) -> (PathBuf, Paths) {
    let dir = std::env::temp_dir().join(format!("impulse-migrate-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(dir.join("map-state")).unwrap();
    (dir.clone(), Paths::new(dir))
}

fn put(file: PathBuf, value: Value) {
    fs::write(file, serde_json::to_vec(&value).unwrap()).unwrap();
}

fn get(file: PathBuf) -> Value {
    serde_json::from_slice(&fs::read(file).unwrap()).unwrap()
}

fn source() -> SourceId {
    SourceId::new("s").unwrap()
}

#[test]
fn v1_files_take_the_source_once_and_keep_a_copy() {
    let (dir, paths) = folder("v1");
    let profile = json!({"favorite": true, "sessions": 2, "played": 9, "last": 4, "save_mods": true, "folded": ["PLAYER"]});
    put(paths.games(), json!({"7": profile}));
    put(paths.hotkeys(), json!({"enabled": false, "games": {"7": {"c": {"0": [112]}}}}));
    put(paths.picked(), json!({"7": {"path": "C:\\g.exe", "used": true}}));
    put(paths.settings(), json!({"accent": "LIME", "game": "7"}));
    put(paths.minimap(), json!({"zoom": 1.0}));
    put(paths.docks(), json!([{"id": "mods"}]));
    fs::write(paths.map_state().join("Ab1.json"), "{}").unwrap();
    migrate(&paths, &source());

    let games = get(paths.games());
    assert_eq!(games["games"]["s:7"], json!({"favorite": true, "sessions": 2, "played": 9, "last": 4}));
    assert_eq!(games["trainers"]["s:7"], json!({"save_mods": true, "folded_labels": ["PLAYER"]}));
    assert_eq!(get(paths.hotkeys()), json!({"version": 2, "enabled": false, "trainers": {"s:7": {"c": {"0": [112]}}}}));
    assert_eq!(get(paths.picked())["games"]["s:7"]["used"], true);
    assert_eq!(get(paths.settings())["game"], "s:7");
    assert_eq!(get(paths.minimap()), json!({"version": 2, "zoom": 1.0}));
    assert_eq!(get(paths.docks()), json!([{"id": "mods"}]));
    assert_eq!(get(dir.join("games.v1.json")), json!({"7": profile}));
    assert!(paths.map_state().join("s").join("v1").join("Ab1.json").is_file());

    let before: Vec<_> = [paths.games(), paths.hotkeys(), paths.settings()].map(|file| fs::read(file).unwrap()).into();
    migrate(&paths, &source());
    let after: Vec<_> = [paths.games(), paths.hotkeys(), paths.settings()].map(|file| fs::read(file).unwrap()).into();
    assert_eq!(before, after);
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn a_file_an_older_build_wrote_back_is_not_qualified_twice_nor_its_copy_replaced() {
    let (dir, paths) = folder("older");
    put(dir.join("settings.v1.json"), json!({"game": "7"}));
    put(paths.settings(), json!({"game": "s:7", "accent": "RED"}));
    migrate(&paths, &source());
    assert_eq!(get(paths.settings()), json!({"version": 2, "game": "s:7", "accent": "RED"}));
    assert_eq!(get(dir.join("settings.v1.json")), json!({"game": "7"}));
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn newer_and_damaged_files_stay_as_they_are() {
    let (dir, paths) = folder("odd");
    put(paths.games(), json!({"version": 3, "games": {}}));
    fs::write(paths.hotkeys(), "{").unwrap();
    migrate(&paths, &source());
    assert_eq!(get(paths.games()), json!({"version": 3, "games": {}}));
    assert_eq!(fs::read_to_string(paths.hotkeys()).unwrap(), "{");
    assert!(!dir.join("games.v1.json").exists() && !dir.join("hotkeys.v1.json").exists());
    fs::remove_dir_all(dir).unwrap();
}
