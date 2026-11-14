//! v1 files to v2, before anything opens them: keys take the source that wrote them, `version: 2` is added.
//! A v1 file is kept once as `<name>.v1.json`; one that cannot be read or written stays as it is.

use std::{fs, path::Path};

use model::SourceId;
use serde_json::{Map, Value};

use super::{Paths, VERSION, maps_of};

type Object = Map<String, Value>;

/// `source` wrote every v1 file: there was no other.
pub fn migrate(paths: &Paths, source: &SourceId) {
    let prefix = format!("{}:", source.as_str());
    // A key qualified already came from a v2 file an older build wrote back.
    let qualify = |key: &str| if key.starts_with(&prefix) { key.to_owned() } else { format!("{prefix}{key}") };
    let qualified = |keys: Object| -> Object { keys.into_iter().map(|(key, value)| (qualify(&key), value)).collect() };
    one(&paths.games(), |v1| {
        let (mut games, mut trainers) = (Object::new(), Object::new());
        for (id, profile) in v1 {
            let Value::Object(mut profile) = profile else { continue };
            let game: Object = ["favorite", "sessions", "played", "last"]
                .into_iter()
                .filter_map(|field| Some((field.to_owned(), profile.remove(field)?)))
                .collect();
            if let Some(labels) = profile.remove("folded") {
                profile.insert("folded_labels".into(), labels);
            }
            games.insert(qualify(&id), game.into());
            // v1 kept a game's trainer under the game's id, as its source names it.
            trainers.insert(qualify(&id), profile.into());
        }
        Object::from_iter([("games".to_owned(), games.into()), ("trainers".to_owned(), trainers.into())])
    });
    one(&paths.hotkeys(), |mut v1| {
        if let Some(Value::Object(games)) = v1.remove("games") {
            v1.insert("trainers".into(), qualified(games).into());
        }
        v1
    });
    one(&paths.picked(), |v1| Object::from_iter([("games".to_owned(), qualified(v1).into())]));
    one(&paths.settings(), |mut v1| {
        if let Some(Value::String(game)) = v1.get_mut("game") {
            *game = qualify(game);
        }
        v1
    });
    for file in [paths.overlay(), paths.minimap(), paths.resources()] {
        one(&file, |v1| v1);
    }
    map_state(&paths.map_state(), source);
}

fn one(file: &Path, change: impl FnOnce(Object) -> Object) {
    let Ok(bytes) = fs::read(file) else { return };
    let Ok(Value::Object(v1)) = serde_json::from_slice(&bytes) else { return };
    if v1.contains_key("version") {
        return;
    }
    let backup = file.with_extension("v1.json");
    if !backup.exists()
        && let Err(e) = disk::replace(&backup, &bytes)
    {
        log::warn!("{} not migrated, no copy: {e:#}", file.display());
        return;
    }
    let mut v2 = change(v1);
    v2.insert("version".into(), VERSION.into());
    let written = serde_json::to_vec_pretty(&v2).map_err(anyhow::Error::from).and_then(|json| disk::replace(file, &json));
    match written {
        Ok(()) => log::info!("{} migrated", file.display()),
        Err(e) => log::warn!("{} not migrated: {e:#}", file.display()),
    }
}

/// The flat v1 files go to `<source>/v1/`, where a map of that source looks for its own.
fn map_state(dir: &Path, source: &SourceId) {
    let Ok(entries) = fs::read_dir(dir) else { return };
    let v1 = maps_of(dir, source, true);
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() || path.extension().is_none_or(|extension| extension != "json") {
            continue;
        }
        if let Err(e) = fs::create_dir_all(&v1).and_then(|()| fs::rename(&path, v1.join(entry.file_name()))) {
            log::warn!("{} not moved: {e}", path.display());
        }
    }
}

#[cfg(test)]
mod tests;
