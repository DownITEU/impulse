//! Group icons: one-glyph subsets of WeMod's icon fonts, kept on disk and turned into outlines.

use std::{
    collections::HashMap,
    fs,
    path::Path,
    sync::atomic::{AtomicUsize, Ordering},
    thread,
};

use anyhow::{Result, anyhow, bail};
use model::Glyph;
use ttf_parser::{Face, OutlineBuilder, Rect};

use super::DIR;

const FONTS: &str = "https://fonts.wemod.com/";
const PARALLEL: usize = 8;

struct Source {
    url: String,
    file: String,
}

// Names come from a remote file and reach a URL and a file name.
fn source(name: &str) -> Option<Source> {
    let (font, bare, glyph) = match name.split_once(':') {
        None => ("uicons-solid-rounded", name, format!("fi-sr-{name}")),
        Some(("roundies", bare)) => ("roundies", bare, bare.to_owned()),
        Some(("material", bare)) => ("material-symbols-rounded", bare, bare.to_owned()),
        Some(_) => return None,
    };
    let sound = !bare.is_empty() && bare.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    sound.then(|| Source { url: format!("{FONTS}?font={font}&glyphs={glyph}"), file: format!("{font}-{bare}.ttf") })
}

/// Sets each group's icon from `names` (as `parse::map` returns them), downloading the missing ones when `online`;
/// blocking. Returns whether some icon is still not on disk.
pub fn attach(map: &mut model::Map, names: &[Vec<String>], data_dir: &Path, online: bool) -> bool {
    let dir = data_dir.join(DIR).join("icons");
    let sources: Vec<Vec<Option<Source>>> = names
        .iter()
        .map(|row| {
            row.iter()
                .map(|name| {
                    let source = source(name);
                    if source.is_none() && !name.is_empty() {
                        log::debug!("icon {name:?} is not supported");
                    }
                    source
                })
                .collect()
        })
        .collect();

    let mut missing: Vec<&Source> = Vec::new();
    for source in sources.iter().flatten().flatten() {
        if !dir.join(&source.file).exists() && !missing.iter().any(|known| known.file == source.file) {
            missing.push(source);
        }
    }
    let next = AtomicUsize::new(0);
    thread::scope(|scope| {
        for _ in 0..if online { missing.len().min(PARALLEL) } else { 0 } {
            scope.spawn(|| {
                while let Some(source) = missing.get(next.fetch_add(1, Ordering::Relaxed)) {
                    if let Err(e) = download(&dir, source) {
                        log::debug!("icon {} not downloaded: {e:#}", source.file);
                    }
                }
            });
        }
    });

    let mut outlines: HashMap<&str, Option<Glyph>> = HashMap::new();
    for (category, row) in map.categories.iter_mut().zip(&sources) {
        for (group, source) in category.groups.iter_mut().zip(row) {
            let Some(source) = source else { continue };
            group.icon = outlines
                .entry(&source.file)
                .or_insert_with(|| {
                    load(&dir, source)
                        .inspect_err(|e| log::debug!("icon {} has no outline: {e:#}", source.file))
                        .ok()
                })
                .clone();
        }
    }
    missing.iter().any(|source| !dir.join(&source.file).exists())
}

// Disk first and never revalidated: a subset under one URL does not change.
fn download(dir: &Path, source: &Source) -> Result<()> {
    let response = fetch::get(&source.url, None)?;
    // An empty file marks a glyph the server does not have.
    let keep: &[u8] = match response.status {
        200 => &response.body,
        404 => &[],
        status => bail!("HTTP {status}"),
    };
    disk::replace(&dir.join(&source.file), keep)
}

fn load(dir: &Path, source: &Source) -> Result<Glyph> {
    let font = fs::read(dir.join(&source.file))?;
    outline(&font).ok_or_else(|| anyhow!("no usable glyph"))
}

fn outline(font: &[u8]) -> Option<Glyph> {
    let face = Face::parse(font, 0).ok()?;
    let mut glyph = None;
    for table in face.tables().cmap?.subtables {
        table.codepoints(|code| glyph = glyph.or_else(|| table.glyph_index(code)));
    }
    let glyph = glyph?;
    let mut path = Normalized::new(face.glyph_bounding_box(glyph)?)?;
    face.outline_glyph(glyph, &mut path)?;
    Some(Glyph { path: path.path })
}

/// Builds SVG path data fitted to the unit square, centered, y down.
struct Normalized {
    scale: f64,
    center: (f64, f64),
    path: String,
}

impl Normalized {
    fn new(bbox: Rect) -> Option<Self> {
        let (width, height) = (f64::from(bbox.x_max) - f64::from(bbox.x_min), f64::from(bbox.y_max) - f64::from(bbox.y_min));
        let size = width.max(height);
        (size > 0.).then(|| Self {
            scale: 1. / size,
            center: ((f64::from(bbox.x_min) + f64::from(bbox.x_max)) / 2., (f64::from(bbox.y_min) + f64::from(bbox.y_max)) / 2.),
            path: String::new(),
        })
    }

    fn put(&mut self, op: char, points: &[(f32, f32)]) {
        if !self.path.is_empty() {
            self.path.push(' ');
        }
        self.path.push(op);
        for &(x, y) in points {
            let x = (f64::from(x) - self.center.0) * self.scale + 0.5;
            let y = 0.5 - (f64::from(y) - self.center.1) * self.scale;
            // Adding zero turns -0 into 0.
            self.path.push_str(&format!(" {} {}", round(x), round(y)));
        }
    }
}

fn round(v: f64) -> f64 {
    (v * 1e4).round() / 1e4 + 0.
}

impl OutlineBuilder for Normalized {
    fn move_to(&mut self, x: f32, y: f32) {
        self.put('M', &[(x, y)]);
    }

    fn line_to(&mut self, x: f32, y: f32) {
        self.put('L', &[(x, y)]);
    }

    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        self.put('Q', &[(x1, y1), (x, y)]);
    }

    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        self.put('C', &[(x1, y1), (x2, y2), (x, y)]);
    }

    fn close(&mut self) {
        self.put('Z', &[]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parts(name: &str) -> Option<(String, String)> {
        source(name).map(|s| (s.url, s.file))
    }

    #[test]
    fn names_map_to_one_glyph_fonts() {
        let url = |font: &str, glyph: &str| format!("https://fonts.wemod.com/?font={font}&glyphs={glyph}");
        assert_eq!(parts("gem"), Some((url("uicons-solid-rounded", "fi-sr-gem"), "uicons-solid-rounded-gem.ttf".into())));
        assert_eq!(
            parts("roundies:food-cauliflower"),
            Some((url("roundies", "food-cauliflower"), "roundies-food-cauliflower.ttf".into()))
        );
        assert_eq!(
            parts("material:mark_email_unread"),
            Some((url("material-symbols-rounded", "mark_email_unread"), "material-symbols-rounded-mark_email_unread.ttf".into()))
        );
    }

    #[test]
    fn foreign_names_are_rejected() {
        for name in ["", "other:gem", "roundies:", "material:a:b", "a/b", "../gem", "gem&x=1", "a b", "ge.m", "gém", "roundies:a?b"] {
            assert!(source(name).is_none(), "{name:?}");
        }
    }

    #[test]
    fn the_outline_is_fitted_centered_and_flipped() {
        let mut path = Normalized::new(Rect { x_min: 0, y_min: 0, x_max: 200, y_max: 100 }).unwrap();
        path.move_to(0., 0.);
        path.line_to(200., 0.);
        path.quad_to(200., 100., 100., 100.);
        path.curve_to(50., 100., 0., 50., 0., 0.);
        path.close();
        assert_eq!(path.path, "M 0 0.75 L 1 0.75 Q 1 0.25 0.5 0.25 C 0.25 0.25 0 0.5 0 0.75 Z");
        assert!(Normalized::new(Rect { x_min: 1, y_min: 1, x_max: 1, y_max: 1 }).is_none());
    }

    #[test]
    fn rounding_keeps_four_decimals_and_no_negative_zero() {
        assert_eq!(round(1. / 3.).to_string(), "0.3333");
        assert_eq!(round(-0.00001).to_string(), "0");
    }

    #[test]
    fn garbage_is_no_font() {
        assert!(outline(b"not a font").is_none());
    }
}
