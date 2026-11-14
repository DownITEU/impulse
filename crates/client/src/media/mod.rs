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

/// `url`'s file in `cache`.
pub fn file(cache: &Path, url: &str) -> PathBuf {
    let keep = |c: char| c.is_ascii_alphanumeric() || c == '.';
    let name: String = url.trim_start_matches("https://").chars().map(|c| if keep(c) { c } else { '_' }).collect();
    cache.join(name)
}

/// Whether the CDN already answered 404 for `url` (blocking), so nothing asks for it again.
pub fn known_missing(cache: &Path, url: &str) -> bool {
    fs::metadata(file(cache, url)).is_ok_and(|meta| meta.len() == 0)
}

pub fn steam_hero(app: u32) -> String {
    format!("https://cdn.cloudflare.steamstatic.com/steam/apps/{app}/library_hero.jpg")
}
