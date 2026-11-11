//! What the user set on a map, kept per map on disk: found pins, filters, layers, the view.

use std::path::{Path, PathBuf};

use model::Map;
// Looked up per drawn pin each frame.
use rustc_hash::FxHashSet as HashSet;
use serde::{Deserialize, Serialize};

const DIR: &str = "map-state";
/// Colors of the user's own pins, new ones taking the next.
pub const PALETTE: [u32; 6] = [0x10BDDB, 0xF5C542, 0xFF6B6B, 0x7BD88F, 0xB28DFF, 0xF2F5EE];

#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Saved {
    /// Pin ids.
    pub found: HashSet<String>,
    /// Group ids.
    pub hidden: HashSet<String>,
    /// Region names.
    pub hidden_regions: HashSet<String>,
    pub unfound_only: bool,
    pub teleport_only: bool,
    pub labels: bool,
    pub regions: bool,
    pub clusters: bool,
    /// Camera center and span.
    pub view: Option<(f64, f64, f64)>,
    pub marks: Vec<Mark>,
    pub marks_shown: bool,
}

/// A pin of the user's own.
#[derive(Clone, Serialize, Deserialize)]
pub struct Mark {
    /// A plane point.
    pub at: (f64, f64),
    pub name: String,
    #[serde(default)]
    pub note: String,
    pub color: u32,
}

impl Default for Saved {
    fn default() -> Self {
        Self {
            found: HashSet::default(),
            hidden: HashSet::default(),
            hidden_regions: HashSet::default(),
            unfound_only: false,
            teleport_only: false,
            labels: true,
            regions: true,
            clusters: true,
            view: None,
            marks: Vec::new(),
            marks_shown: true,
        }
    }
}

impl Saved {
    /// A map seen for the first time hides what its provider hides by default.
    pub fn first(map: &Map) -> Self {
        let hidden = map.categories.iter().filter(|category| !category.shown);
        Self { hidden: hidden.flat_map(|category| category.groups.iter().map(|group| group.id.clone())).collect(), ..Self::default() }
    }
}

pub fn dir(data_dir: &Path) -> PathBuf {
    data_dir.join(DIR)
}

/// The copy on disk (blocking).
pub fn load(dir: &Path, id: &str) -> Option<Saved> {
    crate::store::load(&file(dir, id))
}

/// Blocking.
pub fn store(dir: &Path, id: &str, saved: &Saved) {
    crate::store::save(&file(dir, id), saved);
}

// Ids come from the provider: only letters and digits reach a file name.
fn file(dir: &Path, id: &str) -> PathBuf {
    dir.join(format!("{}.json", id.chars().filter(char::is_ascii_alphanumeric).collect::<String>()))
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    #[test]
    fn state_survives_a_round_trip_and_older_files() {
        let dir = std::env::temp_dir().join(format!("impulse-map-state-{}", std::process::id()));
        let mark = Mark { at: (0.5, 0.5), name: "Camp".to_owned(), note: String::new(), color: 0xFF0000 };
        let saved = Saved { found: HashSet::from_iter(["p1".to_owned()]), clusters: false, view: Some((0.5, 0.25, 4096.)), marks: vec![mark], ..Saved::default() };
        store(&dir, "../ab", &saved);
        let back = load(&dir, "ab").unwrap();
        assert_eq!((back.found, back.clusters, back.view, back.labels), (saved.found, false, saved.view, true));
        assert_eq!((back.marks[0].name.as_str(), back.marks[0].color), ("Camp", 0xFF0000));
        fs::write(file(&dir, "old"), br#"{"found":["x"]}"#).unwrap();
        let old = load(&dir, "old").unwrap();
        assert!(old.found.contains("x") && old.clusters && old.regions && old.marks_shown);
        let _ = fs::remove_dir_all(&dir);
    }
}
