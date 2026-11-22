//! The public catalog, read into model games.

mod cache;

use std::collections::{BTreeMap, HashMap};

use anyhow::{Context, Result};
use model::{Game, Locator, MapOffer, Offer, Ref, Standalone, StoreId};
use serde::{Deserialize, Deserializer};
use serde_json::value::RawValue;

pub use cache::{cached, forget, refresh, size};

pub(crate) const ART_CDN: &str = "https://api-cdn.wemod.com";

#[derive(Deserialize)]
struct Catalog {
    /// Ordered, so a title's releases list the same way at every start.
    games: BTreeMap<String, Entry>,
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
    #[serde(default, deserialize_with = "strings")]
    purchase_uris: Vec<String>,
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
    #[serde(default, deserialize_with = "strings")]
    terms: Vec<String>,
    /// CDN path with a `{size}` placeholder.
    thumbnail: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Map {
    title_id: String,
    external_id: String,
    name: String,
    /// CDN path.
    thumbnail: Option<String>,
    updated_at: Option<String>,
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

fn parse(json: &[u8]) -> Result<Vec<Game>> {
    let Catalog { games, titles, platforms, maps } = serde_json::from_slice(json).context("parse catalog")?;
    let wand = crate::source();
    let mut maps_of: HashMap<String, Vec<MapOffer>> = HashMap::new();
    for map in maps {
        maps_of.entry(map.title_id).or_default().push(MapOffer {
            id: Ref::new(&wand, &map.external_id),
            name: map.name,
            thumbnail: map.thumbnail.map(|path| format!("{ART_CDN}{path}")),
            revision: map.updated_at,
        });
    }
    let standalone: HashMap<String, &StandaloneApp> =
        platforms.standalone.apps.iter().map(|app| (format!("standalone:{}", app.id).to_ascii_lowercase(), app)).collect();
    let games = games.into_values().filter_map(|entry| {
        let title = titles.get(&entry.title_id)?;
        let art = |size: u32| title.thumbnail.as_deref().map(|path| format!("{ART_CDN}{}", path.replace("{size}", &size.to_string())));
        let standalone = entry.correlation_ids.iter().filter_map(|id| standalone.get(&id.to_ascii_lowercase()));
        Some(Game {
            name: title.name.clone(),
            terms: title.terms.clone(),
            standalone: standalone
                .map(|app| Standalone {
                    store_id: format!("standalone:{}", app.id),
                    launcher: app.launcher.clone(),
                    locators: app.locators.iter().filter_map(locator).collect(),
                })
                .collect(),
            cover: art(460),
            thumbnail: art(100),
            maps: maps_of.get(&entry.title_id).cloned().unwrap_or_default(),
            // Wand keeps one trainer per game, under the game's id.
            trainer: entry.trainer.map(|trainer| Offer {
                id: Ref::new(&wand, &entry.id),
                cheats: trainer.cheat_count,
                builds: trainer.supported_versions,
                revision: trainer.updated_at,
            }),
            id: Ref::new(&wand, &entry.id),
            title_id: entry.title_id,
            store_ids: entry.correlation_ids.iter().filter_map(|id| StoreId::parse(id)).collect(),
            exe: entry.version_path,
            purchase: entry.purchase_uris,
        })
    });
    Ok(games.collect())
}

/// The strings of a list; anything else, a null or an item of another kind, is skipped. Read as raw text first: no
/// value is parsed before it is known to be a string.
fn strings<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Vec<String>, D::Error> {
    let raw = Box::<RawValue>::deserialize(deserializer)?;
    let items: Vec<Box<RawValue>> = serde_json::from_str(raw.get()).unwrap_or_default();
    Ok(items.iter().filter_map(|item| serde_json::from_str(item.get()).ok()).collect())
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
                     "purchaseUris":["steam://store/1"],
                     "trainer":{"cheatCount":3,"supportedVersions":[7],"updatedAt":"u"}},
                "2":{"id":"2","titleId":"gone","purchaseUris":null},
                "3":{"id":"3","titleId":"gone","purchaseUris":[1e400,"steam://store/3"]}},
              "titles":{"t":{"name":"Game","terms":["gm"],"thumbnail":"/t/{size}.jpg"}},
              "maps":[{"id":"1","titleId":"t","externalId":"x1","name":"First","thumbnail":"/m/1.webp","updatedAt":"r1"},
                       {"id":"2","titleId":"t","externalId":"x2","name":"Second"},
                       {"id":"3","titleId":"other","externalId":"x3","name":"Elsewhere"}],
              "platforms":{"standalone":{"apps":[{"id":"app","launcher":"l.exe",
                "locators":[{"type":"directory","value":"C:\\G"},{"type":"from-the-future","value":"x"}]},
                {"id":"other","launcher":"o.exe","locators":[]}]}}}"#,
        )
        .unwrap();
        let [game] = &games[..] else { panic!("one game with a title") };
        assert_eq!(game.purchase, ["steam://store/1"]);
        assert_eq!((game.name.as_str(), game.exe.as_deref(), &game.terms[..]), ("Game", Some("a.exe"), &["gm".to_owned()][..]));
        let maps: Vec<_> =
            game.maps.iter().map(|m| (m.id.to_string(), m.name.as_str(), m.thumbnail.as_deref(), m.revision.as_deref())).collect();
        assert_eq!(game.id.to_string(), "wand:1");
        assert_eq!(
            maps,
            [("wand:x1".into(), "First", Some("https://api-cdn.wemod.com/m/1.webp"), Some("r1")), ("wand:x2".into(), "Second", None, None)]
        );
        assert_eq!(game.cover.as_deref(), Some("https://api-cdn.wemod.com/t/460.jpg"));
        let offer = game.trainer.as_ref().unwrap();
        assert_eq!(
            (offer.id.to_string(), offer.cheats, &offer.builds[..], offer.revision.as_deref()),
            ("wand:1".into(), 3, &[7][..], Some("u"))
        );
        let standalone: Vec<_> = game.standalone.iter().map(|app| (app.store_id.as_str(), app.locators.len())).collect();
        assert_eq!(standalone, [("standalone:app", 1), ("standalone:other", 0)]);
    }
}
