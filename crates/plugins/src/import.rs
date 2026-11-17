//! A guest's map made safe for the app (P5): bounded, consistent and inside its scope. Runs on every fresh answer and
//! on every copy read from disk. A broken structure refuses the map; media outside the scope is left out.

mod links;
#[cfg(test)]
mod tests;

use std::collections::{HashMap, HashSet};

use anyhow::{Context, Result, ensure};
use model::{Category, Failure, FailureKind, Glyph, Group, Icon, Label, Map, Picture, Pin, Point, Region, Text, TileSet, Tiles};
use sdk::wire;

use crate::scope::Scope;

const MOST_TILE_SETS: usize = 8;
const MOST_GROUPS: usize = 4_096;
const MOST_PINS: usize = 100_000;
const MOST_REGIONS: usize = 10_000;
const MOST_LABELS: usize = 20_000;
const MOST_POINTS: usize = 2_000_000;
const ID: usize = 256;
const NAME: usize = 512;
const DESCRIPTION: usize = 64 << 10;
const URL: usize = 2_048;
const GLYPH: usize = 16 << 10;
const RGB: u32 = 0xFF_FFFF;

pub fn map(scope: &Scope, map: wire::Map) -> Result<Map> {
    convert(scope, map).context(Failure::new(FailureKind::Refused))
}

fn convert(scope: &Scope, map: wire::Map) -> Result<Map> {
    let groups = map.categories.iter().map(|category| category.groups.len()).sum();
    let points = map.regions.iter().flat_map(|region| &region.outlines).map(Vec::len).sum();
    let counts = [
        ("tile sets", map.tile_sets.len(), MOST_TILE_SETS),
        ("groups", groups, MOST_GROUPS),
        ("pins", map.pins.len(), MOST_PINS),
        ("regions", map.regions.len(), MOST_REGIONS),
        ("labels", map.labels.len(), MOST_LABELS),
        ("region points", points, MOST_POINTS),
    ];
    for (what, count, most) in counts {
        ensure!(count <= most, "{count} {what}, past {most}");
    }
    ensure!(map.background <= RGB, "background {:#x}", map.background);

    let mut seen = HashSet::new();
    let mut tile_sets = Vec::new();
    for set in map.tile_sets {
        fresh(&mut seen, "tile set", &set.id)?;
        tile_sets.extend(tile_set(scope, set)?);
    }
    ensure!(!tile_sets.is_empty(), "no tile set on the plugin's hosts");

    let mut at = HashMap::new();
    let mut categories = Vec::new();
    for (index, category) in map.categories.into_iter().enumerate() {
        ensure!(category.color <= RGB, "category {:?}: color", category.name);
        let mut groups = Vec::new();
        for (slot, group) in category.groups.into_iter().enumerate() {
            ensure!(at.insert(id("group", &group.id)?, (index, slot)).is_none(), "group {:?} twice", group.id);
            groups.push(Group {
                name: text("group name", group.name, NAME)?,
                icon: group.icon.and_then(|icon| self::icon(scope, icon)),
                id: group.id,
            });
        }
        categories.push(Category {
            name: text("category name", category.name, NAME)?,
            color: category.color,
            shown: category.shown,
            groups,
        });
    }

    let mut seen = HashSet::new();
    let pins = map.pins.into_iter().map(|pin| {
        fresh(&mut seen, "pin", &pin.id)?;
        let group = *at.get(&pin.group).with_context(|| format!("pin {:?}: no group {:?}", pin.id, pin.group))?;
        ensure!(pin.description.len() <= DESCRIPTION, "pin {:?}: description of {} bytes", pin.id, pin.description.len());
        let image = pin.image.and_then(|image| Some(Picture { preview: media(scope, &image.preview)?, full: media(scope, &image.full)? }));
        Ok(Pin {
            at: point(pin.at)?,
            name: text("pin name", pin.name, NAME)?,
            description: links::filter(&pin.description, scope),
            group,
            image,
            video: pin.video.and_then(|video| media(scope, &video)),
            position: None,
            level: None,
            id: pin.id,
        })
    });
    let pins = pins.collect::<Result<_>>()?;

    let mut seen = HashSet::new();
    let regions = map.regions.into_iter().map(|region| {
        fresh(&mut seen, "region", &region.id)?;
        ensure!(region.color <= RGB, "region {:?}: color", region.id);
        ensure!(region.thickness > 0. && region.thickness <= 64., "region {:?}: thickness {}", region.id, region.thickness);
        let rings = region.outlines.into_iter().filter(|ring| ring.len() >= 3);
        Ok(Region {
            name: text("region name", region.name, NAME)?,
            color: region.color,
            thickness: region.thickness,
            outlines: rings.map(|ring| ring.into_iter().map(point).collect()).collect::<Result<_>>()?,
            zoom: zoom(region.zoom)?,
            id: region.id,
        })
    });
    let regions = regions.collect::<Result<_>>()?;

    let labels = map.labels.into_iter().map(|label| {
        ensure!(label.color <= RGB, "label {:?}: color", label.text);
        ensure!((1. ..=200.).contains(&label.size), "label {:?}: size {}", label.text, label.size);
        Ok(Label {
            text: text("label", label.text, NAME)?,
            color: label.color,
            size: label.size,
            at: point(label.at)?,
            zoom: zoom(label.zoom)?,
        })
    });
    let labels = labels.collect::<Result<_>>()?;

    Ok(Map {
        tile_sets,
        background: map.background,
        categories,
        pins,
        regions,
        labels,
        frame: None,
        teleport: false,
        unfinished: map.unfinished,
    })
}

/// `None` for a tile set whose tiles are not on the plugin's hosts.
fn tile_set(scope: &Scope, set: wire::TileSet) -> Result<Option<TileSet>> {
    let at = || format!("tile set {:?}", set.id);
    ensure!(["{z}", "{x}", "{y}"].iter().all(|part| set.template.contains(part)), "{}: template without {{z}}, {{x}} or {{y}}", at());
    ensure!(set.size.is_power_of_two() && (64..=1024).contains(&set.size), "{}: tiles of {} pixels", at(), set.size);
    ensure!(set.min_zoom <= set.max_zoom && set.max_zoom <= Tiles::MAX_ZOOM, "{}: zooms {}..{}", at(), set.min_zoom, set.max_zoom);
    let bounds = match set.bounds {
        Some([low, high]) => {
            let (low, high) = (point(low)?, point(high)?);
            let inside = |at: Point| (0. ..=1.).contains(&at.x) && (0. ..=1.).contains(&at.y);
            ensure!(inside(low) && inside(high) && low.x < high.x && low.y < high.y, "{}: bounds", at());
            Some([low, high])
        }
        None => None,
    };
    let Some(template) = media(scope, &set.template) else {
        log::debug!("{}: {}: tiles off the plugin's hosts", scope.source.as_str(), at());
        return Ok(None);
    };
    let tiles = Tiles { template, flipped: set.flipped, size: set.size, min_zoom: set.min_zoom, max_zoom: set.max_zoom, bounds };
    Ok(Some(TileSet { name: Text::Plain(text("tile set name", set.name, NAME)?), tiles, id: set.id }))
}

fn icon(scope: &Scope, icon: wire::Icon) -> Option<Icon> {
    match icon {
        wire::Icon::Glyph { path } => (path.len() <= GLYPH).then_some(Icon::Glyph(Glyph { path })),
        wire::Icon::Image { url, cell } => {
            let usable = cell.is_none_or(|[_, _, width, height]| width > 0 && height > 0);
            usable.then(|| Some(Icon::Image { url: media(scope, &url)?, cell })).flatten()
        }
    }
}

/// A media URL rewritten under the plugin's scope; `None` for one it may not reach.
fn media(scope: &Scope, url: &str) -> Option<String> {
    (url.len() <= URL).then(|| scope.media(url)).flatten()
}

fn point(at: wire::Point) -> Result<Point> {
    ensure!(at.x.is_finite() && at.y.is_finite(), "a point at {}, {}", at.x, at.y);
    Ok(Point { x: at.x, y: at.y })
}

fn zoom(zoom: Option<[f32; 2]>) -> Result<(f32, f32)> {
    let [low, high] = zoom.unwrap_or([0., f32::from(Tiles::MAX_ZOOM)]);
    ensure!(low.is_finite() && high.is_finite() && low <= high, "zooms {low}..{high}");
    Ok((low, high))
}

fn text(what: &str, text: String, most: usize) -> Result<String> {
    ensure!(text.len() <= most, "{what} of {} bytes", text.len());
    Ok(text)
}

fn id(what: &str, id: &str) -> Result<String> {
    ensure!(!id.is_empty() && id.len() <= ID, "{what} id {id:?}");
    Ok(id.to_owned())
}

/// Ids key the user's state: each is set and unique within its kind.
fn fresh(seen: &mut HashSet<String>, what: &str, id: &str) -> Result<()> {
    ensure!(seen.insert(self::id(what, id)?), "{what} {id:?} twice");
    Ok(())
}
