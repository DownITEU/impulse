//! Pin markers and group icons as images: each look is an SVG rendered once and kept.

use std::sync::Arc;

use gpui_kit::{AnyElement, App, DevicePixels, RenderImage, SvgSize, Window, component::h_flex, img, prelude::*, px, rgb, size};
use image::{Frame, RgbaImage, imageops};
use model::{Glyph, Level};
use rustc_hash::FxHashMap as HashMap;

use crate::ui::{theme::*, widgets::square};

/// A marker's box in view pixels.
pub const SIZE: (f64, f64) = (26., 38.);
/// The pin's point: the marker's tip, from the box's top left.
pub const TIP: (f64, f64) = (13., 36.5);
/// The round head, from the tip.
pub const HEAD_ABOVE: f64 = 19.5;
pub const HEAD_RADIUS: f64 = 11.;
/// A picture icon over the head: its left and top in the box, and its side.
pub const PICTURE: (f64, f64, f64) = (5.5, 9.5, 15.);

// Drawn in a 24 x 32 box with the tip at (12, 31.5); the view box leaves room for the stroke and the level mark.
const VIEW_BOX: &str = "-1 -5 26 38";
const SHAPE: &str = "M12 31.5C12 31.5 1 20.6 1 12a11 11 0 0 1 22 0c0 8.6-11 19.5-11 19.5z";

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub enum Look {
    Plain,
    Hovered,
    Selected,
}

impl Look {
    pub fn scale(self) -> f64 {
        match self {
            Self::Plain => 1.,
            Self::Hovered => 1.2,
            Self::Selected => 1.35,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Marker {
    /// `None`: the user's own pin.
    pub group: Option<(usize, usize)>,
    pub fill: u32,
    pub look: Look,
    pub found: bool,
    pub level: Option<Level>,
    /// A picture icon is drawn over its head, which then shows no mark.
    pub picture: bool,
}

impl Marker {
    /// What its mark is drawn in: a selected marker turns bright, its mark dark or its fill.
    pub fn ink(&self) -> u32 {
        let fill = self.fill;
        if self.look != Look::Selected {
            ink(fill)
        } else if ink(fill) == BG {
            fill
        } else {
            BG
        }
    }
}

#[derive(Clone, Copy)]
pub enum GroupIcon<'a> {
    Glyph(&'a Glyph),
    Picture(&'a RgbaImage),
    Default,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum Key {
    Marker(Marker, u32),
    Icon((usize, usize), u32, u32),
    Picture((usize, usize), u32, Option<u32>),
}

#[derive(Default)]
pub struct Images {
    made: HashMap<Key, Option<Arc<RenderImage>>>,
}

impl Images {
    /// `scale` is the window's: images are made at device pixels.
    pub fn marker(&mut self, marker: Marker, glyph: Option<&Glyph>, scale: f32, cx: &App) -> Option<Arc<RenderImage>> {
        let factor = f64::from(scale) * marker.look.scale();
        let (width, height) = ((SIZE.0 * factor).round() as u32, (SIZE.1 * factor).round() as u32);
        let key = Key::Marker(marker, height);
        self.made.entry(key).or_insert_with(|| render(&marker_svg(marker, glyph, accent(cx)), width, height, cx)).clone()
    }

    /// A group's glyph alone, `side` view pixels square.
    pub fn icon(&mut self, group: (usize, usize), glyph: &Glyph, color: u32, side: f32, scale: f32, cx: &App) -> Option<Arc<RenderImage>> {
        let pixels = (side * scale).round() as u32;
        let key = Key::Icon(group, pixels, color);
        let svg = || {
            format!(r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1 1"><path d="{}" fill="{}"/></svg>"#, glyph.path, hex(color))
        };
        self.made.entry(key).or_insert_with(|| render(&svg(), pixels, pixels, cx)).clone()
    }

    /// A group's picture icon, fitted and centered in a square `pixels` across; `tint`: its shape in that color.
    pub fn picture(&mut self, group: (usize, usize), picture: &RgbaImage, pixels: u32, tint: Option<u32>) -> Arc<RenderImage> {
        self.made.entry(Key::Picture(group, pixels, tint)).or_default().get_or_insert_with(|| fitted(picture, pixels, tint)).clone()
    }

    /// Group `at`'s icon, a glyph in `color`; a small square for the default mark.
    pub fn glyph(&mut self, mark: GroupIcon, at: (usize, usize), color: u32, side: f32, window: &Window, cx: &App) -> AnyElement {
        let scale = window.scale_factor();
        let image = match mark {
            GroupIcon::Glyph(glyph) => self.icon(at, glyph, color, side, scale, cx),
            GroupIcon::Picture(picture) => Some(self.picture(at, picture, (side * scale).round() as u32, None)),
            GroupIcon::Default => None,
        };
        match image {
            Some(image) => img(image).size(px(side)).flex_none().into_any_element(),
            None => h_flex().size(px(side)).flex_none().justify_center().child(square(side * 0.45, rgb(color))).into_any_element(),
        }
    }

    /// Forgets every image; the caller drops them from the atlas.
    pub fn clear(&mut self) -> Vec<Arc<RenderImage>> {
        self.made.drain().filter_map(|(_, image)| image).collect()
    }
}

fn marker_svg(marker: Marker, glyph: Option<&Glyph>, accent: u32) -> String {
    let (selected, fill, ink) = (marker.look == Look::Selected, marker.fill, marker.ink());
    let (body, stroke) = if selected { (TEXT_BRIGHT, accent) } else { (fill, TEXT_BRIGHT) };
    let mark = match glyph {
        _ if marker.picture => String::new(),
        Some(glyph) => format!(r#"<g transform="translate(5.5 5.5) scale(13)"><path d="{}" fill="{}"/></g>"#, glyph.path, hex(ink)),
        None => format!(r#"<circle cx="12" cy="12" r="3.5" fill="{}"/>"#, hex(ink)),
    };
    let level = match marker.level {
        Some(Level::Above) => r##"<path d="M8.5-1h7L12-4.6z" fill="#F2F5EE" stroke="#0A0B0A" stroke-width="0.8"/>"##,
        Some(Level::Below) => r##"<path d="M8.5-4.6h7L12-1z" fill="#F2F5EE" stroke="#0A0B0A" stroke-width="0.8"/>"##,
        None => "",
    };
    let check = if marker.found {
        format!(
            r#"<circle cx="20" cy="4" r="4.6" fill="{}" stroke="{}"/><path d="M17.8 4.1l1.5 1.5 2.9-3" fill="none" stroke="{}" stroke-width="1.5"/>"#,
            hex(TEXT_BRIGHT),
            hex(BG),
            hex(BG)
        )
    } else {
        String::new()
    };
    let opacity = if marker.found && !selected { "0.5" } else { "1" };
    format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="{VIEW_BOX}"><g opacity="{opacity}"><path d="{SHAPE}" fill="none" stroke="{}" stroke-opacity="0.45" stroke-width="3"/><path d="{SHAPE}" fill="{}" stroke="{}" stroke-width="1.5"/>{mark}</g>{level}{check}</svg>"#,
        hex(BG),
        hex(body),
        hex(stroke),
    )
}

/// `picture` scaled to fit a clear square `side` across, in the atlas's BGRA; `tint` keeps only its alpha.
fn fitted(picture: &RgbaImage, side: u32, tint: Option<u32>) -> Arc<RenderImage> {
    let (width, height) = picture.dimensions();
    let scale = f64::from(side) / f64::from(width.max(height));
    let fit = |length: u32| ((f64::from(length) * scale).round() as u32).clamp(1, side.max(1));
    let scaled = imageops::resize(picture, fit(width), fit(height), imageops::FilterType::Triangle);
    let mut square = RgbaImage::new(side.max(1), side.max(1));
    let offset = |length: u32| i64::from((side.max(1) - length) / 2);
    imageops::overlay(&mut square, &scaled, offset(scaled.width()), offset(scaled.height()));
    let [_, red, green, blue] = tint.unwrap_or_default().to_be_bytes();
    for texel in square.pixels_mut() {
        if tint.is_some() {
            texel.0[..3].copy_from_slice(&[red, green, blue]);
        }
        texel.0.swap(0, 2);
    }
    Arc::new(RenderImage::new([Frame::new(square)]))
}

fn render(svg: &str, width: u32, height: u32, cx: &App) -> Option<Arc<RenderImage>> {
    let renderer = cx.svg_renderer();
    let parsed = renderer.parse_svg(svg.as_bytes()).map_err(|e| log::debug!("map icon: {e}")).ok()?;
    let exact = SvgSize::ExactSize(size(DevicePixels(width as i32), DevicePixels(height as i32)));
    renderer.render_parsed(&parsed, exact).ok()
}

fn hex(color: u32) -> String {
    format!("#{:06x}", color & 0xFF_FF_FF)
}

/// Dark or bright, whichever reads on `fill`.
pub fn ink(fill: u32) -> u32 {
    if brightness(fill) > 140 { BG } else { TEXT_BRIGHT }
}

/// `color`, or a light gray where it would vanish on the dark panels.
pub fn legible(color: u32) -> u32 {
    if brightness(color) < 60 { MUTED } else { color }
}

fn brightness(color: u32) -> u32 {
    let [_, red, green, blue] = color.to_be_bytes().map(u32::from);
    (red * 299 + green * 587 + blue * 114) / 1000
}

#[cfg(test)]
mod tests {
    use image::{Rgba, RgbaImage};

    #[test]
    fn a_tinted_picture_keeps_its_shape_in_the_tint() {
        let mut picture = RgbaImage::from_pixel(4, 4, Rgba([255, 255, 255, 0]));
        picture.put_pixel(1, 1, Rgba([255, 255, 255, 255]));
        let tinted = super::fitted(&picture, 4, Some(0x10_20_30));
        let bytes = tinted.as_bytes(0).unwrap();
        let at = |x: usize, y: usize| &bytes[(y * 4 + x) * 4..][..4];
        assert_eq!(at(1, 1), [0x30, 0x20, 0x10, 255], "BGRA in the tint");
        assert_eq!(at(0, 0)[3], 0, "the clear stays clear");
        let plain = super::fitted(&picture, 4, None);
        assert_eq!(&plain.as_bytes(0).unwrap()[(4 + 1) * 4..][..4], [255, 255, 255, 255]);
    }
}
