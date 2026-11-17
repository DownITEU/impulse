//! The v1 wire: what crosses between the host and a guest, as JSON. A later field is optional with a default and an
//! unknown one is ignored, so v1 guests and hosts keep reading each other.

use serde::{Deserialize, Serialize};
use serde_json::{Map as Object, Value};

/// How a description links to another pin of its map: this, then the pin's id.
pub const PIN_LINK: &str = "pin:";

/// What the host asks, in the content locale, with the user's settings (each one checked against its field).
#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct Request {
    pub call: Call,
    pub locale: String,
    #[serde(default)]
    pub settings: Object<String, Value>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum Call {
    /// The games the plugin covers: `Vec<Title>`.
    Titles,
    /// A title's maps: `Vec<MapOffer>`.
    Maps { title: String },
    /// A whole map: `Map`.
    Map { map: String },
    /// A call of a later host: answered `unsupported`.
    #[serde(other)]
    Unknown,
}

/// Every answer and every host result that is not raw bytes: `{"ok": ...}` or `{"error": ...}`.
#[derive(Serialize, Deserialize, Debug, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum Reply<T> {
    Ok(T),
    Error(Error),
}

impl<T> From<Result<T, Error>> for Reply<T> {
    fn from(result: Result<T, Error>) -> Self {
        result.map_or_else(Self::Error, Self::Ok)
    }
}

impl<T> From<Reply<T>> for Result<T, Error> {
    fn from(reply: Reply<T>) -> Self {
        match reply {
            Reply::Ok(value) => Ok(value),
            Reply::Error(error) => Err(error),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Error {
    pub kind: ErrorKind,
    /// English, for the log.
    #[serde(default)]
    pub message: String,
}

impl Error {
    pub fn new(kind: ErrorKind, message: impl Into<String>) -> Self {
        Self { kind, message: message.into() }
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum ErrorKind {
    NotFound,
    Offline,
    Refused,
    Unsupported,
    #[serde(other)]
    Internal,
}

/// A game the plugin has maps for.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Title {
    /// Stable: the user's link to it is kept by it.
    pub id: String,
    pub name: String,
    /// Launchers' ids for it, `<store>:<id>` (`steam:1091500`): how it is matched to a game in the library.
    #[serde(default)]
    pub stores: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct MapOffer {
    /// Stable: the user's state of the map is kept by it.
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub thumbnail: Option<String>,
    /// Changes whenever the map does; without one, the host asks again after the manifest's `refresh`.
    #[serde(default)]
    pub revision: Option<String>,
}

/// A point of the unit square the whole tile pyramid covers, y down. Web Mercator: [`crate::guest::mercator`].
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

/// Ids are the provider's stable ones and unique within their kind across the map: the user's state refers to them.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Map {
    /// At least one: the first shows until the user chooses another.
    pub tile_sets: Vec<TileSet>,
    /// RGB behind the tiles.
    #[serde(default)]
    pub background: u32,
    #[serde(default)]
    pub categories: Vec<Category>,
    #[serde(default)]
    pub pins: Vec<Pin>,
    #[serde(default)]
    pub regions: Vec<Region>,
    #[serde(default)]
    pub labels: Vec<Label>,
    /// Still being worked on: pins may be missing or move.
    #[serde(default)]
    pub unfinished: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct TileSet {
    pub id: String,
    pub name: String,
    /// An https URL with `{z}`, `{x}` and `{y}`, on a host of the manifest's `hosts`.
    pub template: String,
    /// Rows count from the bottom.
    #[serde(default)]
    pub flipped: bool,
    #[serde(default = "tile_size")]
    pub size: u32,
    pub min_zoom: u8,
    pub max_zoom: u8,
    /// What the tiles cover, top left and bottom right, those edges excluded; none: the whole square.
    #[serde(default)]
    pub bounds: Option<[Point; 2]>,
}

fn tile_size() -> u32 {
    256
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Category {
    pub name: String,
    /// RGB.
    pub color: u32,
    /// Whether its pins show before the user chooses.
    #[serde(default = "yes")]
    pub shown: bool,
    pub groups: Vec<Group>,
}

fn yes() -> bool {
    true
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Group {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub icon: Option<Icon>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum Icon {
    /// SVG path data in the unit square, y down; the host tints it.
    Glyph { path: String },
    /// Drawn as it is: the whole picture, or a cell of a sheet (left, top, width, height in its pixels).
    Image {
        url: String,
        #[serde(default)]
        cell: Option<[u32; 4]>,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Pin {
    pub id: String,
    pub at: Point,
    pub name: String,
    /// CommonMark without HTML or images; another pin is `[text](pin:<id>)` (`PIN_LINK`). A web link opens only on a
    /// host of the manifest's `links`; any other shows as its text.
    #[serde(default)]
    pub description: String,
    /// Its group's id.
    pub group: String,
    #[serde(default)]
    pub image: Option<Picture>,
    /// A clip file (MP4, H.264) the host plays; a video page is a link.
    #[serde(default)]
    pub video: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Picture {
    /// Sized for a card.
    pub preview: String,
    pub full: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Region {
    pub id: String,
    pub name: String,
    /// RGB.
    pub color: u32,
    #[serde(default = "thickness")]
    pub thickness: f32,
    /// One outer ring per polygon; holes are not drawn.
    pub outlines: Vec<Vec<Point>>,
    /// The zoom levels it shows at, inclusive; none: all.
    #[serde(default)]
    pub zoom: Option<[f32; 2]>,
}

fn thickness() -> f32 {
    2.
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Label {
    pub text: String,
    /// RGB.
    pub color: u32,
    pub size: f32,
    pub at: Point,
    #[serde(default)]
    pub zoom: Option<[f32; 2]>,
}

/// What `http_get` asks the host for.
#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct HttpRequest {
    pub url: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_call_of_a_later_host_reads_as_unknown_and_defaults_fill_in() {
        let request: Request = serde_json::from_str(r#"{"call": {"kind": "routes", "map": "a"}, "locale": "fr", "later": 1}"#).unwrap();
        assert_eq!((request.call, request.settings.len()), (Call::Unknown, 0));
        let map: Map = serde_json::from_str(
            r#"{"tile_sets": [{"id": "sat", "name": "Satellite", "template": "https://t/{z}/{x}/{y}.jpg", "min_zoom": 0, "max_zoom": 4}],
                "categories": [{"name": "Loot", "color": 1, "groups": [{"id": "chest", "name": "Chest", "icon": {"kind": "image", "url": "https://t/s.png"}}]}],
                "regions": [{"id": "r", "name": "R", "color": 2, "outlines": [[{"x": 0.1, "y": 0.2}]]}]}"#,
        )
        .unwrap();
        assert_eq!((map.tile_sets[0].size, map.categories[0].shown, map.regions[0].thickness), (256, true, 2.));
        assert_eq!(map.categories[0].groups[0].icon, Some(Icon::Image { url: "https://t/s.png".into(), cell: None }));
    }

    #[test]
    fn replies_are_ok_or_error_and_an_unknown_kind_is_internal() {
        let reply: Reply<u32> = serde_json::from_str(r#"{"error": {"kind": "quota"}}"#).unwrap();
        assert_eq!(Result::from(reply), Err::<u32, _>(Error::new(ErrorKind::Internal, "")));
        assert_eq!(serde_json::to_string(&Reply::from(Ok::<_, Error>(7))).unwrap(), r#"{"ok":7}"#);
    }
}
