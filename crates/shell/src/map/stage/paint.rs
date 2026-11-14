//! Painting the map: tiles, regions, labels, pins, the user's own, the player.

use std::{ops::RangeInclusive, rc::Rc, sync::Arc};

use anyhow::Context as _;
use blocking::unblock;
use client::map::{heading::Bearing, state::Saved};
use gpui_kit::{
    App, BorderStyle, Bounds, ContentMask, Context, Corners, DispatchPhase, Font, FontWeight, Hitbox, MouseMoveEvent, MouseUpEvent,
    PathBuilder, Pixels, RenderImage, Rgba, SharedString, TextAlign, TextRun, TransformationMatrix, Window, font, point, px, quad, radians,
    rgb, size,
};
use image::{DynamicImage, Frame, RgbaImage};
use model::{Glyph, Map, Point};

use super::{
    Stage, Walk,
    camera::{Camera, Pixel, extent},
    pins::{self, GROUP_RADIUS},
    tiles::Key,
};
use crate::{
    Load,
    map::{
        board::plane,
        markers::{Look, Marker, SIZE, TIP, ink},
    },
    ui::{art, assets::Icon, theme::*},
};

const PLAYER: f32 = 34.;
const CONE: f32 = 96.;
// The way the player last went, while they stand.
const STALE: f32 = 0.4;
// Opacity of a region's fill while hovered, and of its outline at rest.
const REGION_FILL: f32 = 0.16;
const REGION_LINE: f32 = 0.7;
// A group of pins: its count's size, the ring's width and opacity.
const COUNT_TEXT: f32 = 10.;
const RING: (f32, f32) = (1.5, 0.7);
// Place names stay readable at every map's own sizes.
const LABEL_SIZES: (f32, f32) = (11., 18.);

/// What a frame paints from the board, copied out of it: painting text needs the app, not a borrow of the board.
struct Shot {
    map: Rc<Map>,
    saved: Rc<Saved>,
    selected: Option<usize>,
    mark: Option<usize>,
    accent: u32,
}

impl Stage {
    /// `hitbox` is the map's own: cards, menus and the lightbox over it occlude it.
    pub(super) fn paint(&mut self, bounds: Bounds<Pixels>, hitbox: Hitbox, window: &mut Window, cx: &mut Context<Self>) {
        let (player, shot) = {
            let board = self.board.read(cx);
            let Load::Ready(map) = &board.map else { return };
            let player =
                board.player.zip(map.frame.as_ref()).map(|(position, frame)| Walk { at: frame.to_map(position), bearing: board.bearing });
            (player, Shot { map: map.clone(), saved: board.saved.clone(), selected: board.selected, mark: board.mark, accent: accent(cx) })
        };
        let (map, saved, selected, accent) = (shot.map.clone(), &shot.saved, shot.selected, shot.accent);
        // The overlays were laid out for the old size: one more frame places them.
        if self.bounds != bounds {
            window.request_animation_frame();
        }
        self.bounds = bounds;
        self.tile = f64::from(map.tiles.size) / f64::from(window.scale_factor());
        let area = self.size();
        let spans = self.spans(&map);
        if self.camera.is_none() {
            self.camera = Some(home(&map, area, &spans));
        }
        let mini = self.live.as_ref().map(|live| live.read(cx).span(&map, self.tile).clamp(*spans.start(), *spans.end()));
        if let (Some(span), Some(camera)) = (mini, &mut self.camera) {
            camera.span = span;
        }
        self.animate(&map, selected, player, window);
        let Some(camera) = self.camera else { return };

        // On the window, not the element: a drag goes on outside the map. The minimap only zooms.
        let view = cx.entity();
        if mini.is_none() {
            window.on_mouse_event({
                let view = view.clone();
                move |event: &MouseMoveEvent, phase, window, cx| {
                    if phase == DispatchPhase::Bubble {
                        let over = hitbox.is_hovered(window);
                        view.update(cx, |stage, cx| stage.moved(event, over, cx));
                    }
                }
            });
            window.on_mouse_event(move |event: &MouseUpEvent, phase, _, cx| {
                if phase == DispatchPhase::Bubble {
                    view.update(cx, |stage, cx| stage.released(event, cx));
                }
            });
        }

        let place = |(x, y): Pixel| bounds.origin + point(px(x as f32), px(y as f32));
        let disc = |center: gpui_kit::Point<Pixels>, side: f32| {
            Bounds::new(center - point(px(side / 2.), px(side / 2.)), size(px(side), px(side)))
        };
        window.with_content_mask(Some(ContentMask { bounds }), |window| {
            self.paint_tiles(&map, &camera, window, cx);

            let zoom = (camera.span / self.tile).log2() as f32;
            let shows = |(low, high): (f32, f32)| (low..=high).contains(&zoom);
            let regions = map
                .regions
                .iter()
                .zip(&self.shown.regions)
                .enumerate()
                .filter(|(_, (region, _))| saved.regions && shows(region.zoom) && !saved.hidden_regions.contains(&region.id));
            for (index, (region, extent)) in regions {
                // An outline is tessellated anew each frame: none for a region off the view.
                let Some((low, high)) = *extent else { continue };
                let (((left, top), (right, bottom)), pad) =
                    ((camera.to_view(low, area), camera.to_view(high, area)), f64::from(region.thickness));
                if right < -pad || bottom < -pad || left > area.0 + pad || top > area.1 + pad {
                    continue;
                }
                let hovered = self.shown.hovered_region == Some(index);
                let corners: Vec<_> = region.outline.iter().map(|at| place(camera.to_view(*at, area))).collect();
                if hovered && let Some(fill) = path(&corners, PathBuilder::fill()) {
                    window.paint_path(fill, alpha(region.color, REGION_FILL));
                }
                if let Some(outline) = path(&corners, PathBuilder::stroke(px(region.thickness))) {
                    window.paint_path(outline, alpha(region.color, if hovered { 1. } else { REGION_LINE }));
                }
            }

            let markers = self.markers(&map, &camera, cx);
            let mut lifted = Vec::new();
            for marker in markers.iter() {
                let (x, y) = camera.to_view(marker.at, area);
                if x < -SIZE.0 || y < -GROUP_RADIUS || x > area.0 + SIZE.0 || y > area.1 + SIZE.1 {
                    continue;
                }
                if marker.count > 1 {
                    let color = map.categories[map.pins[marker.pin].group.0].color;
                    let (center, side) = (place((x, y)), GROUP_RADIUS as f32 * 2.);
                    let ring = alpha(ink(color), RING.1);
                    window.paint_quad(quad(disc(center, side), px(side / 2.), rgb(color), px(RING.0), ring, BorderStyle::default()));
                    let count = self.shown.counts.entry(marker.count).or_insert_with(|| marker.count.to_string().into()).clone();
                    text(count, COUNT_TEXT, ink(color), false, center, window, cx);
                } else if let Some(scale) = self.lift(marker.pin) {
                    lifted.push((marker.pin, scale));
                } else {
                    self.paint_marker(&shot, marker.pin, 1., (x, y), window, cx);
                }
            }
            // The selected pin shows even when a filter or a group hides it, and over the rest.
            if let Some(pin) = selected.filter(|pin| lifted.iter().all(|(lifted, _)| lifted != pin)) {
                lifted.push((pin, self.lift(pin).unwrap_or(1.)));
            }
            lifted.sort_by_key(|(pin, _)| selected == Some(*pin));
            for (pin, scale) in lifted {
                let tip = camera.to_view(map.pins[pin].at, area);
                self.paint_marker(&shot, pin, scale, tip, window, cx);
            }
            for index in mark_order(saved, shot.mark).collect::<Vec<_>>() {
                let look = self.mark_look(index, shot.mark);
                let marker = Marker { group: None, fill: saved.marks[index].color, look, found: false, level: None };
                self.paint_look(marker, None, look.scale(), camera.to_view(plane(saved.marks[index].at), area), window, cx);
            }

            // Over the pins, as place names are on a map.
            for (label, line) in map.labels.iter().zip(&self.shown.labels).filter(|(label, _)| saved.labels && shows(label.zoom)) {
                let at = place(camera.to_view(label.at, area));
                text(line.clone(), label.size.clamp(LABEL_SIZES.0, LABEL_SIZES.1), label.color, true, at, window, cx);
            }

            if let Some(Walk { at, bearing }) = self.player {
                let (center, scale) = (place(camera.to_view(at, area)), window.scale_factor());
                if let Some(Bearing { angle, fresh }) = bearing {
                    // About the player, in device pixels; the icon points up.
                    let turn = TransformationMatrix::unit()
                        .translate(center.scale(scale))
                        .rotate(radians(angle as f32))
                        .translate(center.scale(-scale));
                    let color = alpha(accent, if fresh { 1. } else { STALE }).into();
                    let _ = window.paint_svg(disc(center, CONE), Icon::Heading.path().into(), None, turn, color, cx);
                }
                let color = rgb(accent).into();
                let _ = window.paint_svg(disc(center, PLAYER), Icon::Player.path().into(), None, TransformationMatrix::unit(), color, cx);
            }
        });
    }

    /// The shown pins, grouped for the camera's zoom.
    pub(super) fn markers(&mut self, map: &Map, camera: &Camera, cx: &App) -> Rc<Vec<pins::Marker>> {
        // Half-level steps: pins do not regroup on every wheel tick.
        let step = (camera.span.log2() * 2.).round() as i32;
        if let Some((_, markers)) = self.shown.markers.as_ref().filter(|(held, _)| *held == step) {
            return markers.clone();
        }
        let board = self.board.read(cx);
        let span = 2f64.powf(step as f64 / 2.);
        let close = span >= self.tile * 2f64.powi(map.tiles.max_zoom.into()) * 0.9;
        let markers = Rc::new(pins::markers(&map.pins, |index| board.shows(map, index), span, close || !board.saved.clusters));
        self.shown.markers = Some((step, markers.clone()));
        markers
    }

    fn paint_marker(&mut self, shot: &Shot, index: usize, scale: f64, tip: Pixel, window: &mut Window, cx: &App) {
        let pin = &shot.map.pins[index];
        let category = &shot.map.categories[pin.group.0];
        let look = self.look(index, shot.selected);
        let marker =
            Marker { group: Some(pin.group), fill: category.color, look, found: shot.saved.found.contains(&pin.id), level: pin.level };
        self.paint_look(marker, category.groups[pin.group.1].icon.as_ref(), scale, tip, window, cx);
    }

    /// A marker `scale` times its size with its tip on `tip`, on whole device pixels so the image stays sharp.
    /// The image is made at the look's own scale; other sizes are passing ones.
    fn paint_look(&mut self, marker: Marker, glyph: Option<&Glyph>, scale: f64, tip: Pixel, window: &mut Window, cx: &App) {
        let (look, device) = (marker.look, window.scale_factor());
        let Some(image) = self.images.marker(marker, glyph, device, cx) else { return };
        let snap = |at: f64| ((at * f64::from(device)).round() / f64::from(device)) as f32;
        let corner = self.bounds.origin + point(px(snap(tip.0 - TIP.0 * scale)), px(snap(tip.1 - TIP.1 * scale)));
        let (pixels, shrink) = (image.size(0), (scale / look.scale()) as f32 / device);
        let rect = Bounds::new(corner, size(px(pixels.width.0 as f32 * shrink), px(pixels.height.0 as f32 * shrink)));
        let _ = window.paint_image(rect, rect, Corners::default(), image, 0, false);
    }

    pub(super) fn mark_look(&self, index: usize, mark: Option<usize>) -> Look {
        match Some(index) {
            index if index == mark => Look::Selected,
            index if index == self.shown.hovered_mark => Look::Hovered,
            _ => Look::Plain,
        }
    }

    fn paint_tiles(&mut self, map: &Rc<Map>, camera: &Camera, window: &mut Window, cx: &mut Context<Self>) {
        let (area, origin) = (self.size(), self.bounds.origin);
        let level = camera.level(self.tile, &(map.tiles.min_zoom..=map.tiles.max_zoom));
        let (columns, rows) = camera.tiles(level, area);
        let rect = |[left, top, right, bottom]: [f64; 4]| {
            let corner = |x: f64, y: f64| origin + point(px(x as f32), px(y as f32));
            Bounds::from_corners(corner(left, top), corner(right, bottom))
        };
        let middle = ((columns.start() + columns.end()) / 2, (rows.start() + rows.end()) / 2);
        let mut wanted = Vec::new();
        for row in rows {
            for column in columns.clone() {
                let place = rect(camera.tile(level, column, row, area));
                // A stand-in paints only its share of the tile: an ancestor's part scaled up, a descendant whole.
                let exact = self.tiles.draw((level, column, row), |(from, column, row), image| {
                    let whole = rect(camera.tile(from, column, row, area));
                    let part = place.intersect(&whole);
                    // Out to the padded image's edge, so `part` maps onto the real texels.
                    let whole = whole.dilate(whole.size.width / map.tiles.size as f32);
                    let _ = window.paint_image(part, whole, Corners::default(), image, 0, false);
                });
                if !exact && let Some(load) = self.tiles.wanted((level, column, row)) {
                    // The middle of the view loads first.
                    wanted.push((column.abs_diff(middle.0).pow(2) + row.abs_diff(middle.1).pow(2), load));
                }
            }
        }
        // Levels a zoom passes would take the load slots; the ancestors stand in until it ends.
        if self.glide.is_some() {
            return;
        }
        wanted.sort_by_key(|(distance, _)| *distance);
        for (_, key) in wanted {
            if self.tiles.claim(key) {
                self.load_tile(map, key, cx);
            }
        }
    }

    fn load_tile(&self, map: &Map, key: Key, cx: &mut Context<Self>) {
        let (url, cache, epoch) = (map.tiles.url(key.0, key.1, key.2), self.tile_dir.clone(), self.epoch);
        cx.spawn(async move |stage, cx| {
            let image = unblock(move || decode(&art::load(&cache, &url)?)).await;
            stage
                .update(cx, |stage, cx| {
                    if stage.epoch == epoch {
                        stage.tiles.loaded(key, image.ok());
                        cx.notify();
                    }
                })
                .ok();
        })
        .detach();
    }
}

/// The user's own pins as drawn, bottom to top: the oldest first, the open one last.
pub(super) fn mark_order(saved: &Saved, mark: Option<usize>) -> impl DoubleEndedIterator<Item = usize> + '_ {
    (0..saved.marks.len()).filter(move |&index| saved.marks_shown && mark != Some(index)).chain(mark)
}

/// A tile in the atlas's BGRA with its edge texels repeated around it:
/// the atlas has no gaps and filters across an image's border, which shows as seams.
fn decode(bytes: &[u8]) -> anyhow::Result<Arc<RenderImage>> {
    // Most tiles are lossy RGB: read as decoded, without an RGBA copy first.
    let ((width, height), texels, depth) = match image::load_from_memory_with_format(bytes, image::ImageFormat::WebP)? {
        DynamicImage::ImageRgb8(tile) => (tile.dimensions(), tile.into_raw(), 3),
        tile => {
            let tile = tile.into_rgba8();
            (tile.dimensions(), tile.into_raw(), 4)
        }
    };
    let (columns, rows) = (width as usize, height as usize);
    let mut padded = Vec::with_capacity((columns + 2) * (rows + 2) * 4);
    for y in 0..rows + 2 {
        let row = &texels[y.saturating_sub(1).min(rows - 1) * columns * depth..][..columns * depth];
        for x in 0..columns + 2 {
            let texel = &row[x.saturating_sub(1).min(columns - 1) * depth..][..depth];
            padded.extend_from_slice(&[texel[2], texel[1], texel[0], texel.get(3).copied().unwrap_or(u8::MAX)]);
        }
    }
    let padded = RgbaImage::from_raw(width + 2, height + 2, padded).context("tile size")?;
    Ok(Arc::new(RenderImage::new([Frame::new(padded)])))
}

/// The pins whole, with room around them; the middle of the plane for a map without any.
fn home(map: &Map, area: Pixel, spans: &RangeInclusive<f64>) -> Camera {
    match extent(map.pins.iter().map(|pin| pin.at)) {
        Some((low, high)) => Camera::fit(low, high, (area.0 * 0.9, area.1 * 0.9), spans),
        None => Camera { center: Point { x: 0.5, y: 0.5 }, span: spans.start() * 2. },
    }
}

/// A closed polygon through `corners`.
fn path(corners: &[gpui_kit::Point<Pixels>], mut builder: PathBuilder) -> Option<gpui_kit::Path<Pixels>> {
    let (first, rest) = corners.split_first()?;
    builder.move_to(*first);
    rest.iter().for_each(|corner| builder.line_to(*corner));
    builder.close();
    builder.build().ok()
}

/// One line centered on `at`, unless it is off the view; a shadow keeps it readable on any tile.
fn text(text: SharedString, height: f32, color: u32, shadow: bool, at: gpui_kit::Point<Pixels>, window: &mut Window, cx: &mut App) {
    let line_height = px(height * 1.3);
    // GPUI keeps shapes between frames, and a shape does not hold its color: the shadow's costs a lookup.
    let shape = |color: Rgba, window: &mut Window| {
        let font = Font { weight: FontWeight::BOLD, ..font(UI) };
        let run = TextRun { len: text.len(), font, color: color.into(), background_color: None, underline: None, strikethrough: None };
        window.text_system().shape_line(text.clone(), px(height), &[run], None)
    };
    let line = shape(rgb(color), window);
    let origin = point(at.x - line.width / 2., at.y - line_height / 2.);
    if !window.content_mask().bounds.intersects(&Bounds::new(origin, size(line.width + px(1.), line_height + px(1.)))) {
        return;
    }
    if shadow {
        let _ = shape(alpha(BG, 0.9), window).paint(origin + point(px(1.), px(1.)), line_height, TextAlign::Left, None, window, cx);
    }
    let _ = line.paint(origin, line_height, TextAlign::Left, None, window, cx);
}

#[cfg(test)]
mod tests {
    use image::{ExtendedColorType, codecs::webp::WebPEncoder};

    #[test]
    fn a_tile_is_padded_with_its_edges_in_bgra() {
        // Red, then green: without alpha, and with the green half transparent.
        for (texels, color, alpha) in [
            (vec![255, 0, 0, 0, 255, 0], ExtendedColorType::Rgb8, 255),
            (vec![255, 0, 0, 255, 0, 255, 0, 128], ExtendedColorType::Rgba8, 128),
        ] {
            let mut webp = Vec::new();
            WebPEncoder::new_lossless(&mut webp).encode(&texels, 2, 1, color).unwrap();
            let tile = super::decode(&webp).unwrap();
            assert_eq!((tile.size(0).width.0, tile.size(0).height.0), (4, 3));
            let row = [0, 0, 255, 255, 0, 0, 255, 255, 0, 255, 0, alpha, 0, 255, 0, alpha];
            assert_eq!(tile.as_bytes(0).unwrap(), row.repeat(3));
        }
    }
}
