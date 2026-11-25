//! The public catalog, read into model games.

mod cache;

use std::{
    collections::{BTreeMap, HashMap},
    sync::{Arc, PoisonError, RwLock},
};

use anyhow::{Context, Result};
use model::{Game, Locator, MapOffer, Offer, Ref, Standalone, StoreId};
use serde::{Deserialize, Deserializer};
use serde_json::value::RawValue;

pub use cache::{cached, forget, refresh, size};

pub(crate) const ART_CDN: &str = "https://api-cdn.wemod.com";
/// A game's `flags` bit: Wand's overlay supports it.
const OVERLAY_SUPPORTED: u64 = 256;
// Bits of a map's `flags`.
const LIVE_LOCATION: u64 = 2;
const TELEPORT: u64 = 4;

/// What the last catalog read says of each game beyond its record, by game id.
#[derive(Clone, Default)]
pub struct Marks(Arc<RwLock<HashMap<String, Mark>>>);

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Mark {
    /// Wand's overlay draws in it.
    pub overlay: bool,
    /// A map of its title follows the player.
    pub live: bool,
    /// A map of its title teleports the player.
    pub teleport: bool,
}

impl Marks {
    pub fn of(&self, id: &str) -> Mark {
        self.0.read().unwrap_or_else(PoisonError::into_inner).get(id).copied().unwrap_or_default()
    }
}

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
    /// Any value: a bad one means no overlay for that game, not an unreadable catalog.
    #[serde(default)]
    flags: serde_json::Value,
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
    // Any value, as a game's.
    #[serde(default)]
    flags: serde_json::Value,
    #[serde(default)]
    supports_teleport: serde_json::Value,
}

#[derive(Deserialize)]
struct Platforms {
    standalone: StandaloneApps,
    /// `containers`: the application to start by package family (Halo MCC's without its anti-cheat). Any value: a bad one
    /// is skipped.
    #[serde(default)]
    uwp: serde_json::Value,
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

/// The games, with what else the catalog says of them in `marks`.
fn parse(json: &[u8], marks: &Marks) -> Result<Vec<Game>> {
    let Catalog { games, titles, platforms, maps } = serde_json::from_slice(json).context("parse catalog")?;
    let wand = crate::source();
    let mut maps_of: HashMap<String, Vec<MapOffer>> = HashMap::new();
    // By title: a map follows the player, a map teleports (its `supportsTeleport`, else its flag).
    let mut reach: HashMap<String, Mark> = HashMap::new();
    for map in maps {
        let flags = map.flags.as_u64().unwrap_or_default();
        let title = reach.entry(map.title_id.clone()).or_default();
        title.live |= flags & LIVE_LOCATION != 0;
        title.teleport |= map.supports_teleport.as_bool().unwrap_or(flags & TELEPORT != 0);
        maps_of.entry(map.title_id).or_default().push(MapOffer {
            id: Ref::new(&wand, &map.external_id),
            name: map.name,
            thumbnail: map.thumbnail.map(|path| format!("{ART_CDN}{path}")),
            revision: map.updated_at,
        });
    }
    let marked = games.values().map(|entry| {
        let overlay = entry.flags.as_u64().is_some_and(|flags| flags & OVERLAY_SUPPORTED != 0);
        (entry.id.clone(), Mark { overlay, ..reach.get(&entry.title_id).copied().unwrap_or_default() })
    });
    *marks.0.write().unwrap_or_else(PoisonError::into_inner) = marked.filter(|(_, mark)| *mark != Mark::default()).collect();
    let standalone: HashMap<String, &StandaloneApp> =
        platforms.standalone.apps.iter().map(|app| (format!("standalone:{}", app.id).to_ascii_lowercase(), app)).collect();
    let containers = platforms.uwp["containers"].as_object().into_iter().flatten();
    let containers: HashMap<String, &str> =
        containers.filter_map(|(family, app)| Some((family.to_ascii_lowercase(), app.as_str()?))).collect();
    let games = games.into_values().filter_map(|entry| {
        let title = titles.get(&entry.title_id)?;
        let store_ids: Vec<StoreId> = entry.correlation_ids.iter().filter_map(|id| StoreId::parse(id)).collect();
        let packaged = store_ids.iter().filter(|id| id.store().eq_ignore_ascii_case("uwp"));
        let package_apps =
            packaged.filter_map(|id| Some((id.clone(), containers.get(&id.id().to_ascii_lowercase())?.to_string()))).collect();
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
            package_apps,
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
            store_ids,
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

    fn read(json: &[u8]) -> Result<Vec<Game>> {
        parse(json, &Marks::default())
    }

    #[test]
    fn reads_games_in_model_terms() {
        let games = read(
            br#"{"games":{
                "1":{"id":"1","titleId":"t","correlationIds":["steam:1","Standalone:App","standalone:other","uwp:Game.Pkg_8w","uwp:Odd_8w"],
                     "versionPath":"a.exe",
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
                {"id":"other","launcher":"o.exe","locators":[]}]},
                "uwp":{"containers":{"game.pkg_8w":"NoEAC","Odd_8w":7},"ignore":["^x"]}}}"#,
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
        let apps: Vec<_> = game.package_apps.iter().map(|(id, app)| (id.as_str(), app.as_str())).collect();
        assert_eq!(apps, [("uwp:Game.Pkg_8w", "NoEAC")], "by family in any case; a value not text is skipped");
        let odd = br#"{"games":{},"titles":{},"platforms":{"standalone":{"apps":[]},"uwp":null}}"#;
        assert!(read(odd).unwrap().is_empty(), "odd package data is skipped, not the catalog");
    }

    #[test]
    fn the_overlay_draws_only_in_a_game_flagged_for_it() {
        let json = br#"{"games":{"k":{"id":"1739","titleId":"t","flags":772},"2":{"id":"2","titleId":"t","flags":4},
            "3":{"id":"3","titleId":"t"},"4":{"id":"4","titleId":"t","flags":"256"}},
            "titles":{},"platforms":{"standalone":{"apps":[]}}}"#;
        let marks = Marks::default();
        parse(json, &marks).unwrap();
        let draws: Vec<bool> = ["1739", "2", "3", "4", "k"].iter().map(|id| marks.of(id).overlay).collect();
        assert_eq!(draws, [true, false, false, false, false]);
        parse(br#"{"games":{},"titles":{},"platforms":{"standalone":{"apps":[]}}}"#, &marks).unwrap();
        assert!(!marks.of("1739").overlay, "a newer catalog replaces the set");
    }

    #[test]
    fn a_title_s_maps_say_whether_its_games_follow_and_teleport_the_player() {
        let json = br#"{"games":{"1":{"id":"1","titleId":"a"},"2":{"id":"2","titleId":"b"},"3":{"id":"3","titleId":"c"}},
            "titles":{},"platforms":{"standalone":{"apps":[]}},
            "maps":[{"titleId":"a","externalId":"x","name":"A1","flags":1},{"titleId":"a","externalId":"y","name":"A2","flags":7},
                {"titleId":"b","externalId":"z","name":"B","flags":7,"supportsTeleport":false},
                {"titleId":"c","externalId":"w","name":"C","flags":"7"}]}"#;
        let marks = Marks::default();
        parse(json, &marks).unwrap();
        let reach = |id| (marks.of(id).live, marks.of(id).teleport);
        assert_eq!([reach("1"), reach("2"), reach("3")], [(true, true), (true, false), (false, false)]);
    }
}
