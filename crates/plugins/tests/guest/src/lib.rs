//! A guest that uses every feature of the v1 wire, for the `plugins` crate's tests to build and run.

use sdk::guest::{Context, Provider, http_get, mercator};
use sdk::wire::{Category, Error, ErrorKind, Group, Icon, Label, Map, MapOffer, Picture, Pin, Point, Region, TileSet, Title};

struct Guest;

sdk::export!(Guest);

impl Provider for Guest {
    fn titles(_: &Context) -> Result<Vec<Title>, Error> {
        Ok(vec![Title { id: "night".into(), name: "Night City".into(), stores: vec!["steam:1091500".into()] }])
    }

    fn maps(_: &Context, title: &str) -> Result<Vec<MapOffer>, Error> {
        match title {
            "night" => Ok(vec![MapOffer { id: "city".into(), name: "City".into(), thumbnail: None, revision: Some("r1".into()) }]),
            _ => Err(not_found("title")),
        }
    }

    fn map(context: &Context, map: &str) -> Result<Map, Error> {
        match map {
            "city" => Ok(city(context)),
            // Exercises the host import: the body of the fetch becomes a pin's name.
            "net" => {
                let body = http_get("https://tiles.example.com/data.json")?;
                let mut map = city(context);
                map.pins[0].name = String::from_utf8_lossy(&body).into_owned();
                Ok(map)
            }
            _ => Err(not_found("map")),
        }
    }
}

fn not_found(what: &str) -> Error {
    Error::new(ErrorKind::NotFound, format!("no such {what}"))
}

fn at(x: f64, y: f64) -> Point {
    Point { x, y }
}

fn city(context: &Context) -> Map {
    Map {
        tile_sets: vec![
            TileSet {
                id: "sat".into(),
                name: "Satellite".into(),
                template: "https://tiles.example.com/sat/{z}/{x}/{y}.jpg".into(),
                flipped: false,
                size: 256,
                min_zoom: 8,
                max_zoom: 16,
                bounds: Some([at(0.25, 0.25), at(0.75, 0.5)]),
            },
            TileSet {
                id: "drawn".into(),
                name: "Drawn".into(),
                template: "https://tiles.example.com/drawn/{z}/{x}/{y}.png".into(),
                flipped: false,
                size: 512,
                min_zoom: 0,
                max_zoom: 4,
                bounds: None,
            },
        ],
        background: 0x101010,
        categories: vec![
            Category {
                name: "Places".into(),
                color: 0xC0A040,
                shown: true,
                groups: vec![Group { id: "shrine".into(), name: "Shrine".into(), icon: Some(Icon::Glyph { path: "M0 0H1V1Z".into() }) }],
            },
            Category {
                name: "Loot".into(),
                color: 0x40A0C0,
                shown: true,
                groups: vec![
                    Group {
                        id: "chest".into(),
                        name: "Chest".into(),
                        icon: Some(Icon::Image { url: "https://tiles.example.com/sprite.png".into(), cell: Some([32, 0, 32, 32]) }),
                    },
                    Group { id: "loot".into(), name: "Loot".into(), icon: None },
                ],
            },
        ],
        pins: vec![
            Pin {
                id: "p1".into(),
                at: mercator(10., 20.),
                name: "Gate shrine".into(),
                description: "Near [the gate](pin:p2), see [wiki](https://wiki.example.com/p1) or [elsewhere](https://evil.example/x)."
                    .into(),
                group: "shrine".into(),
                image: Some(Picture {
                    preview: "https://tiles.example.com/p1.jpg".into(),
                    full: "https://tiles.example.com/p1-full.jpg".into(),
                }),
                video: None,
            },
            Pin {
                id: "p2".into(),
                at: at(0.5, 0.4),
                name: "Gate chest".into(),
                description: String::new(),
                group: "chest".into(),
                image: Some(Picture { preview: "https://evil.example/p2.jpg".into(), full: "https://evil.example/p2.jpg".into() }),
                video: Some("https://tiles.example.com/p2.mp4".into()),
            },
            Pin {
                id: "p3".into(),
                at: at(0.6, 0.45),
                name: format!("{} {}", context.locale, context.settings["level"]),
                description: String::new(),
                group: "loot".into(),
                image: None,
                video: None,
            },
        ],
        regions: vec![Region {
            id: "docks".into(),
            name: "Docks".into(),
            color: 0x2060A0,
            thickness: 2.,
            outlines: vec![
                vec![at(0.30, 0.30), at(0.40, 0.30), at(0.40, 0.40), at(0.30, 0.40)],
                vec![at(0.50, 0.30), at(0.60, 0.30), at(0.60, 0.40), at(0.50, 0.40)],
            ],
            zoom: Some([9., 14.]),
        }],
        labels: vec![Label { text: "Watson".into(), color: 0xFFFFFF, size: 14., at: at(0.45, 0.35), zoom: None }],
        unfinished: false,
    }
}
