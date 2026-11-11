//! Interactive maps. Kept on disk, so a map that was seen once works offline
//! and costs no request until the catalog says it changed.

use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::Result;
use blocking::unblock;
use serde::{Deserialize, Serialize};
use serde_json::value::RawValue;

use crate::{session::Session, transport::Transport};

mod icons;
mod parse;

const DIR: &str = "maps";

// The response is stored as received, so a newer reader can use what this one skips.
#[derive(Serialize, Deserialize)]
struct Stored<'a> {
    /// The catalog's `updatedAt` of the map the copy was fetched for.
    stamp: Option<String>,
    #[serde(borrow)]
    map: &'a RawValue,
}

/// The disk copy, or the map just fetched and stored.
pub struct Cached {
    pub map: model::Map,
    /// The catalog's `updatedAt` the copy was fetched for.
    pub stamp: Option<String>,
    /// Set while some group icon is not on disk yet.
    pub icons: Option<Icons>,
}

/// The id of a map whose disk copy is read again once its missing icons are downloaded.
pub struct Icons(String);

impl Icons {
    /// The map with its missing icons downloaded (blocking).
    pub fn download(self, data_dir: &Path) -> Option<model::Map> {
        load(data_dir, &self.0, true).map(|cached| cached.map)
    }
}

/// The disk copy, from disk alone (blocking).
pub fn cached(data_dir: &Path, id: &str) -> Option<Cached> {
    load(data_dir, id, false)
}

fn load(data_dir: &Path, id: &str, online: bool) -> Option<Cached> {
    let bytes = fs::read(file(data_dir, id)).ok()?;
    let stored: Stored = serde_json::from_slice(&bytes).ok()?;
    let (map, missing) = read(stored.map.get(), data_dir, online).ok()?;
    Some(Cached { map, stamp: stored.stamp, icons: missing.then(|| Icons(id.to_owned())) })
}

fn read(raw: &str, data_dir: &Path, online: bool) -> Result<(model::Map, bool)> {
    let (mut map, names) = parse::map(raw)?;
    let missing = icons::attach(&mut map, &names, data_dir, online);
    Ok((map, missing))
}

pub fn size(data_dir: &Path) -> u64 {
    disk::size(&data_dir.join(DIR))
}

pub fn forget(data_dir: &Path) {
    let _ = fs::remove_dir_all(data_dir.join(DIR));
}

/// Fetches the map and keeps it on disk; icons not on disk yet are left to `Icons::download`.
pub async fn fetch(session: &Session, t: &Transport, data_dir: &Path, id: &str, stamp: Option<String>) -> Result<Cached> {
    let raw: Box<RawValue> = session.get(t, &format!("/v3/game_maps/{}?locale=en-US", name(id))).await?;
    let (dir, id) = (data_dir.to_owned(), id.to_owned());
    unblock(move || {
        let (map, missing) = read(raw.get(), &dir, false)?;
        let stored = Stored { stamp, map: &raw };
        let saved = serde_json::to_vec(&stored).map_err(Into::into).and_then(|bytes| disk::replace(&file(&dir, &id), &bytes));
        if let Err(e) = &saved {
            log::warn!("map {id} not cached: {e:#}");
        }
        // Icons complete the disk copy, which an older revision may still be.
        Ok(Cached { map, stamp: stored.stamp, icons: (missing && saved.is_ok()).then_some(Icons(id)) })
    })
    .await
}

// Ids come from the catalog, a remote file: nothing but letters and digits reaches a URL or a path.
fn name(id: &str) -> String {
    id.chars().filter(char::is_ascii_alphanumeric).collect()
}

fn file(data_dir: &Path, id: &str) -> PathBuf {
    data_dir.join(DIR).join(format!("{}.json", name(id)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disk_copy_keeps_the_whole_response() {
        let dir = std::env::temp_dir().join(format!("impulse-map-{}", std::process::id()));
        let raw = serde_json::json!({
            "metadata": {"tileSource": {"type": "xyz", "urlTemplate": "u/{z}/{x}/{y}"}},
            "objects3d": {"kept": true}
        });
        let raw = RawValue::from_string(raw.to_string()).unwrap();
        let stored = Stored { stamp: Some("2026-01-01T00:00:00Z".to_owned()), map: &raw };
        let path = file(&dir, "../wNYf6cFhpk");
        assert!(path.ends_with("maps/wNYf6cFhpk.json"));
        assert_eq!(name("a/b?c=d"), "abcd");
        disk::replace(&path, &serde_json::to_vec(&stored).unwrap()).unwrap();
        let Cached { map, stamp, .. } = cached(&dir, "wNYf6cFhpk").unwrap();
        assert_eq!((stamp.as_deref(), map.tiles.template.as_str()), (Some("2026-01-01T00:00:00Z"), "u/{z}/{x}/{y}"));
        assert!(fs::read_to_string(&path).unwrap().contains("objects3d"));
        fs::write(&path, b"{").unwrap();
        assert!(cached(&dir, "wNYf6cFhpk").is_none());
        forget(&dir);
        assert!(cached(&dir, "wNYf6cFhpk").is_none());
        let _ = fs::remove_dir_all(&dir);
    }
}
