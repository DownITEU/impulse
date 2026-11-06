mod cache;

use std::collections::HashMap;

use anyhow::{Context, Result};
use serde::Deserialize;

pub use cache::{Freshness, load};

#[derive(Deserialize)]
pub struct Catalog {
    pub games: HashMap<String, Game>,
    pub titles: HashMap<String, Title>,
    pub platforms: Platforms,
    #[serde(skip)]
    by_store_id: HashMap<String, Vec<String>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Game {
    pub id: String,
    pub title_id: String,
    /// Store ids such as `steam:1203620` or `epic:<namespace>:<item>:<app>`.
    #[serde(default)]
    pub correlation_ids: Vec<String>,
    /// Executable whose PE timestamp is the game version.
    pub version_path: Option<String>,
    pub trainer: Option<Trainer>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Trainer {
    #[serde(default)]
    pub supported_versions: Vec<u32>,
}

#[derive(Deserialize)]
pub struct Title {
    pub name: String,
}

#[derive(Deserialize)]
pub struct Platforms {
    pub standalone: Standalone,
}

#[derive(Deserialize)]
pub struct Standalone {
    pub apps: Vec<StandaloneApp>,
}

#[derive(Deserialize)]
pub struct StandaloneApp {
    pub id: String,
    /// Executable relative to the located directory; its presence confirms the install.
    pub launcher: String,
    pub locators: Vec<Locator>,
}

#[derive(Deserialize)]
pub struct Locator {
    #[serde(rename = "type")]
    pub kind: LocatorKind,
    pub value: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LocatorKind {
    Directory,
    RegistryValue,
    UninstallKey,
    #[serde(other)]
    Unknown,
}

impl Catalog {
    pub fn parse(json: &[u8]) -> Result<Self> {
        let mut catalog: Self = serde_json::from_slice(json).context("parse catalog")?;
        for game in catalog.games.values() {
            for id in &game.correlation_ids {
                catalog
                    .by_store_id
                    .entry(id.to_ascii_lowercase())
                    .or_default()
                    .push(game.id.clone());
            }
        }
        Ok(catalog)
    }

    /// Games listed under a store id; several editions can share one.
    pub fn games_for(&self, store_id: &str) -> &[String] {
        self.by_store_id
            .get(&store_id.to_ascii_lowercase())
            .map_or(&[], Vec::as_slice)
    }
}
