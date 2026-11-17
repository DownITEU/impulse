use model::{FailureKind, SourceId};
use sdk::wire;

use super::map;
use crate::scope::{Hosts, Scope};

fn scope() -> Scope {
    let hosts = |patterns: &[&str]| Hosts::parse(&patterns.iter().map(|pattern| (*pattern).to_owned()).collect::<Vec<_>>()).unwrap();
    Scope { source: SourceId::new("plugin-t").unwrap(), hosts: hosts(&["*.tiles.io"]), links: hosts(&["wiki.io"]) }
}

fn at(x: f64, y: f64) -> wire::Point {
    wire::Point { x, y }
}

fn tile_set(id: &str, template: &str) -> wire::TileSet {
    wire::TileSet {
        id: id.into(),
        name: id.to_uppercase(),
        template: template.into(),
        flipped: false,
        size: 256,
        min_zoom: 2,
        max_zoom: 6,
        bounds: Some([at(0.25, 0.25), at(0.75, 0.5)]),
    }
}

fn pin(id: &str, group: &str) -> wire::Pin {
    wire::Pin {
        id: id.into(),
        at: at(0.5, 0.4),
        name: id.into(),
        description: String::new(),
        group: group.into(),
        image: None,
        video: None,
    }
}

fn sample() -> wire::Map {
    let group = |id: &str, icon| wire::Group { id: id.into(), name: id.into(), icon };
    let sprite = wire::Icon::Image { url: "https://a.tiles.io/sprite.png".into(), cell: Some([32, 0, 32, 32]) };
    let ring = |x: f64| vec![at(x, 0.1), at(x + 0.1, 0.1), at(x + 0.1, 0.2)];
    wire::Map {
        tile_sets: vec![tile_set("sat", "https://a.tiles.io/{z}/{x}/{y}.jpg"), tile_set("far", "https://far.io/{z}/{x}/{y}.jpg")],
        background: 0x101010,
        categories: vec![wire::Category {
            name: "Loot".into(),
            color: 0xFF0000,
            shown: true,
            groups: vec![
                group("chest", Some(sprite)),
                group("gem", Some(wire::Icon::Image { url: "https://far.io/gem.png".into(), cell: None })),
            ],
        }],
        pins: vec![
            wire::Pin {
                description: "By [the gate](pin:b) and [far](https://far.io/x).".into(),
                image: Some(wire::Picture { preview: "https://a.tiles.io/a.jpg".into(), full: "https://a.tiles.io/a-full.jpg".into() }),
                video: Some("https://far.io/a.mp4".into()),
                ..pin("a", "chest")
            },
            pin("b", "gem"),
        ],
        regions: vec![wire::Region {
            id: "docks".into(),
            name: "Docks".into(),
            color: 0x00FF00,
            thickness: 2.,
            outlines: vec![ring(0.1), ring(0.5), vec![at(0.9, 0.9)]],
            zoom: Some([3., 5.]),
        }],
        labels: vec![wire::Label { text: "Watson".into(), color: 0xFFFFFF, size: 14., at: at(0.3, 0.3), zoom: None }],
        unfinished: true,
    }
}

#[test]
fn a_map_keeps_what_its_scope_reaches_and_finds_its_groups() {
    let map = map(&scope(), sample()).unwrap();
    assert_eq!(map.tile_sets.len(), 1, "the tile set off the plugin's hosts is left out");
    let tiles = &map.tile_sets[0].tiles;
    assert_eq!((tiles.template.as_str(), tiles.bounds.map(|[low, _]| low.x)), ("plugin-t:https://a.tiles.io/{z}/{x}/{y}.jpg", Some(0.25)));
    let groups = &map.categories[0].groups;
    assert_eq!(
        groups[0].icon,
        Some(model::Icon::Image { url: "plugin-t:https://a.tiles.io/sprite.png".into(), cell: Some([32, 0, 32, 32]) })
    );
    assert_eq!(groups[1].icon, None);
    let (a, b) = (&map.pins[0], &map.pins[1]);
    assert_eq!((a.group, b.group), ((0, 0), (0, 1)));
    assert_eq!(a.description, "By [the gate](pin:b) and far.");
    assert_eq!(
        (a.image.as_ref().map(|image| image.full.as_str()), a.video.as_deref()),
        (Some("plugin-t:https://a.tiles.io/a-full.jpg"), None)
    );
    assert_eq!((map.regions[0].outlines.len(), map.regions[0].zoom), (2, (3., 5.)), "a ring of one point is left out");
    assert_eq!((map.labels[0].zoom, map.frame.is_none(), map.unfinished), ((0., 24.), true, true));
}

#[test]
fn a_broken_map_is_refused() {
    type Breaks = (&'static str, fn(&mut wire::Map));
    let broken: [Breaks; 9] = [
        ("a pin id twice", |map| map.pins[1].id = "a".into()),
        ("a group id twice across the map", |map| map.categories[0].groups[1].id = "chest".into()),
        ("an empty id", |map| map.regions[0].id.clear()),
        ("a pin of no group", |map| map.pins[0].group = "gone".into()),
        ("zooms out of order", |map| map.tile_sets[0].min_zoom = 9),
        ("bounds turned inside out", |map| map.tile_sets[0].bounds = Some([at(0.5, 0.5), at(0.25, 0.75)])),
        ("a point that is not a number", |map| map.pins[0].at.x = f64::NAN),
        ("no tile set on its hosts", |map| map.tile_sets.retain(|set| set.id == "far")),
        ("too many pins", |map| map.pins = (0..100_001).map(|n| pin(&n.to_string(), "chest")).collect()),
    ];
    for (what, breaks) in broken {
        let mut sample = sample();
        breaks(&mut sample);
        let failure = model::Failure::from(map(&scope(), sample).map(|_| ()).unwrap_err());
        assert_eq!(failure.kind, FailureKind::Refused, "{what}");
    }
}

#[test]
fn the_sdk_writes_pin_links_as_the_app_reads_them() {
    assert_eq!(sdk::wire::PIN_LINK, model::PIN_LINK);
}
