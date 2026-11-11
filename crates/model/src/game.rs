use std::path::PathBuf;

use crate::MapOffer;

/// A game a provider knows; the store releases of one title are separate games.
#[derive(Default)]
pub struct Game {
    pub id: String,
    /// Shared by one title's store releases.
    pub title_id: String,
    pub name: String,
    /// Such as `steam:1203620`, `epic:<namespace>:<item>:<app>`, `uwp:<family>`, `origin:<offer>`.
    pub store_ids: Vec<String>,
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
pub struct Standalone {
    pub store_id: String,
    /// Relative to a located folder; its presence confirms the install.
    pub launcher: String,
    pub locators: Vec<Locator>,
}

pub enum Locator {
    Directory(String),
    /// `HIVE\key\path\ValueName`
    RegistryValue(String),
    /// A key under `...\CurrentVersion\Uninstall`, whose `InstallLocation` is read.
    UninstallKey(String),
}

/// What the game list knows about a trainer before it is opened.
pub struct Offer {
    pub cheats: u32,
    pub builds: Vec<u32>,
    /// Changes whenever the trainer does.
    pub revision: Option<String>,
}

/// An installed game, as a launcher reports it.
pub struct Installation {
    pub game_id: String,
    pub store_id: String,
    pub location: PathBuf,
    /// `None` when the exe could not be read.
    pub build: Option<u32>,
}
