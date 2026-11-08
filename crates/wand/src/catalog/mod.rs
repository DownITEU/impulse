//! The public catalog, read into model games.

mod cache;

use std::collections::HashMap;

use anyhow::{Context, Result};
use model::{Game, Locator, Offer, Standalone};
use serde::Deserialize;

pub use cache::{cached, forget, refresh};

const ART_CDN: &str = "https://api-cdn.wemod.com";

#[derive(Deserialize)]
struct Catalog {
    games: HashMap<String, Entry>,
    titles: HashMap<String, Title>,
    platforms: Platforms,
    #[serde(default)]
    maps: Vec<Map>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Entry {
    id: String,
    title_id: String,
    #[serde(default)]
    correlation_ids: Vec<String>,
    version_path: Option<String>,
    trainer: Option<Trainer>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Trainer {
    #[serde(default)]
    cheat_count: u32,
    #[serde(default)]
    supported_versions: Vec<u32>,
    updated_at: Option<String>,
}

#[derive(Deserialize)]
struct Title {
    name: String,
    /// CDN path with a `{size}` placeholder.
    thumbnail: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Map {
    title_id: String,
    name: String,
}

#[derive(Deserialize)]
struct Platforms {
    standalone: StandaloneApps,
}

#[derive(Deserialize)]
struct StandaloneApps {
    apps: Vec<StandaloneApp>,
}

#[derive(Deserialize)]
struct StandaloneApp {
    id: String,
    launcher: String,
    locators: Vec<RawLocator>,
}

#[derive(Deserialize)]
struct RawLocator {
    #[serde(rename = "type")]
    kind: LocatorKind,
    value: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum LocatorKind {
    Directory,
    RegistryValue,
    UninstallKey,
    #[serde(other)]
    Unknown,
}

/// A game whose title is missing is left out.
fn parse(json: &[u8]) -> Result<Vec<Game>> {
    let Catalog { games, titles, platforms, maps } = serde_json::from_slice(json).context("parse catalog")?;
    let mut map_of = HashMap::new();
    for map in &maps {
        map_of.entry(map.title_id.as_str()).or_insert(map.name.as_str());
    }
    let standalone: HashMap<String, &StandaloneApp> =
        platforms.standalone.apps.iter().map(|app| (format!("standalone:{}", app.id).to_ascii_lowercase(), app)).collect();
    let games = games.into_values().filter_map(|entry| {
        let title = titles.get(&entry.title_id)?;
        let art = |size: u32| title.thumbnail.as_deref().map(|path| format!("{ART_CDN}{}", path.replace("{size}", &size.to_string())));
        let standalone = entry.correlation_ids.iter().filter_map(|id| standalone.get(&id.to_ascii_lowercase()));
        Some(Game {
            name: title.name.clone(),
            standalone: standalone
                .map(|app| Standalone {
                    store_id: format!("standalone:{}", app.id),
                    launcher: app.launcher.clone(),
                    locators: app.locators.iter().filter_map(locator).collect(),
                })
                .collect(),
            cover: art(460),
            thumbnail: art(100),
            map: map_of.get(entry.title_id.as_str()).map(|name| (*name).to_owned()),
            trainer: entry.trainer.map(|trainer| Offer {
                cheats: trainer.cheat_count,
                builds: trainer.supported_versions,
                revision: trainer.updated_at,
            }),
            id: entry.id,
            title_id: entry.title_id,
            store_ids: entry.correlation_ids,
            exe: entry.version_path,
        })
    });
    Ok(games.collect())
}

fn locator(raw: &RawLocator) -> Option<Locator> {
    let value = raw.value.clone();
    Some(match raw.kind {
        LocatorKind::Directory => Locator::Directory(value),
        LocatorKind::RegistryValue => Locator::RegistryValue(value),
        LocatorKind::UninstallKey => Locator::UninstallKey(value),
        LocatorKind::Unknown => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_games_in_model_terms() {
        let games = parse(
            br#"{"games":{
                "1":{"id":"1","titleId":"t","correlationIds":["steam:1","Standalone:App","standalone:other"],"versionPath":"a.exe",
                     "trainer":{"cheatCount":3,"supportedVersions":[7],"updatedAt":"u"}},
                "2":{"id":"2","titleId":"gone"}},
              "titles":{"t":{"name":"Game","thumbnail":"/t/{size}.jpg"}},
              "maps":[{"titleId":"t","name":"First"},{"titleId":"t","name":"Second"}],
              "platforms":{"standalone":{"apps":[{"id":"app","launcher":"l.exe",
                "locators":[{"type":"directory","value":"C:\\G"},{"type":"from-the-future","value":"x"}]},
                {"id":"other","launcher":"o.exe","locators":[]}]}}}"#,
        )
        .unwrap();
        let [game] = &games[..] else { panic!("one game with a title") };
        assert_eq!((game.name.as_str(), game.exe.as_deref(), game.map.as_deref()), ("Game", Some("a.exe"), Some("First")));
        assert_eq!(game.cover.as_deref(), Some("https://api-cdn.wemod.com/t/460.jpg"));
        let offer = game.trainer.as_ref().unwrap();
        assert_eq!((offer.cheats, &offer.builds[..], offer.revision.as_deref()), (3, &[7][..], Some("u")));
        let standalone: Vec<_> = game.standalone.iter().map(|app| (app.store_id.as_str(), app.locators.len())).collect();
        assert_eq!(standalone, [("standalone:app", 1), ("standalone:other", 0)]);
    }
}
