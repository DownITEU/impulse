use std::path::PathBuf;

use crate::{MapOffer, Origin, Ref, StoreId};

/// A game a source knows; the store releases of one title are separate games.
#[derive(Clone)]
pub struct Game {
    pub id: Ref,
    /// Shared by one title's store releases.
    pub title_id: String,
    pub name: String,
    /// The title's other names, for a search.
    pub terms: Vec<String>,
    pub store_ids: Vec<StoreId>,
    /// The record's pages in its store, an app's link or a web one; remote data.
    pub purchase: Vec<String>,
    /// The executable, relative to the install, whose PE timestamp is the build.
    pub exe: Option<String>,
    /// One per `standalone:` store id.
    pub standalone: Vec<Standalone>,
    /// A wide picture, then a small one.
    pub cover: Option<String>,
    pub thumbnail: Option<String>,
    /// The title's maps, the first one first.
    pub maps: Vec<MapOffer>,
    pub trainer: Option<Offer>,
}

/// Where to find a game no store knows.
#[derive(Clone)]
pub struct Standalone {
    pub store_id: String,
    /// Relative to a located folder; its presence confirms the install.
    pub launcher: String,
    pub locators: Vec<Locator>,
}

#[derive(Clone)]
pub enum Locator {
    Directory(String),
    /// `HIVE\key\path\ValueName`
    RegistryValue(String),
    /// A key under `...\CurrentVersion\Uninstall`, whose `InstallLocation` is read.
    UninstallKey(String),
}

/// What the game list knows about a trainer before it is opened.
#[derive(Clone)]
pub struct Offer {
    pub id: Ref,
    pub cheats: u32,
    pub builds: Vec<u32>,
    /// Changes whenever the trainer does.
    pub revision: Option<String>,
}

/// An installed game, as a launcher reports it or the user picked it.
#[derive(Clone)]
pub struct Installation {
    pub game: Ref,
    pub origin: Origin,
    pub location: PathBuf,
    /// Other folders its process may run from: the location's final path, a package's other folders.
    pub alternates: Vec<PathBuf>,
    /// `None` when the exe could not be read.
    pub build: Option<u32>,
}
