//! The media cache: a file per URL, named from it, in a folder per kind. An empty file marks what the CDN
//! does not have.

pub mod limit;

use std::{
    fs,
    path::{Path, PathBuf},
};

use crate::storage::Paths;

/// A kind of cached file, in a folder of its own: sized, cleared and limited apart.
#[derive(Clone, Copy, PartialEq)]
pub enum Media {
    Pictures,
    Tiles,
    Videos,
}

impl Media {
    pub const ALL: [Self; 3] = [Self::Pictures, Self::Tiles, Self::Videos];

    pub fn dir(self, paths: &Paths) -> PathBuf {
        match self {
            Self::Pictures => paths.pictures(),
            Self::Tiles => paths.tiles(),
            Self::Videos => paths.videos(),
        }
    }
}

pub fn file(cache: &Path, url: &str) -> PathBuf {
    cache.join(name(url))
}

/// Whether the CDN already answered 404 for `url` (blocking), so nothing asks for it again.
pub fn known_missing(cache: &Path, url: &str) -> bool {
    fs::metadata(file(cache, url)).is_ok_and(|meta| meta.len() == 0)
}

/// Forgets the absences `cache` remembers for the tiles of `templates` (URLs with `{z}`, `{x}`, `{y}`), so they are
/// asked again (blocking). Matched by the file name's part before the first `{`: a wider match only asks again.
pub fn forget_missing(cache: &Path, templates: &[&str]) {
    let Ok(entries) = fs::read_dir(cache) else { return };
    let heads: Vec<String> = templates.iter().filter_map(|template| template.split_once('{')).map(|(head, _)| name(head)).collect();
    for entry in entries.flatten() {
        let named = entry.file_name().to_str().is_some_and(|file| heads.iter().any(|head| file.starts_with(head.as_str())));
        if named && entry.metadata().is_ok_and(|meta| meta.is_file() && meta.len() == 0) {
            let _ = fs::remove_file(entry.path());
        }
    }
}

fn name(url: &str) -> String {
    let keep = |c: char| c.is_ascii_alphanumeric() || c == '.';
    url.trim_start_matches("https://").chars().map(|c| if keep(c) { c } else { '_' }).collect()
}

pub fn steam_hero(app: u32) -> String {
    format!("https://cdn.cloudflare.steamstatic.com/steam/apps/{app}/library_hero.jpg")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_remembered_absences_of_the_templates_are_forgotten() {
        let cache = std::env::temp_dir().join(format!("impulse-media-{}", std::process::id()));
        fs::create_dir_all(&cache).unwrap();
        let ours = "plugin-maps://tiles.example/night/{z}/{x}/{y}.webp";
        let absent = file(&cache, &ours.replace("{z}/{x}/{y}", "1/0/0"));
        let kept = file(&cache, &ours.replace("{z}/{x}/{y}", "1/0/1"));
        let other = file(&cache, "https://cdn.example/day/1/0/0.webp");
        fs::write(&absent, b"").unwrap();
        fs::write(&kept, b"tile").unwrap();
        fs::write(&other, b"").unwrap();
        forget_missing(&cache, &[ours]);
        assert!(!absent.exists() && kept.exists() && other.exists());
        let _ = fs::remove_dir_all(cache);
    }
}
