//! A map's picture icons, cut from their sheets once per copy of the map; the boards showing that copy share them.

use std::path::{Path, PathBuf};

use blocking::unblock;
use gpui_kit::{Context, Task};
use image::{RgbaImage, imageops};
use model::{Icon, Map};
use rustc_hash::FxHashMap as HashMap;

use crate::ui::art;

// A sheet is decoded whole: a larger one is refused.
const SHEET: u32 = 4096;
// Icons are drawn small: one is kept at most this many pixels across.
const KEPT: u32 = 64;

/// A group, by category and group index, and its cell of a sheet.
type Cells = Vec<((usize, usize), Option<[u32; 4]>)>;

pub struct Icons {
    cut: HashMap<(usize, usize), RgbaImage>,
    // Dropped with the last copy that holds these icons: no sheet is fetched for a map no board shows.
    _cut: Task<()>,
}

impl Icons {
    /// Starts cutting `map`'s picture icons from the sheets in `cache`; notifies as each sheet's are in.
    pub fn new(map: &Map, cache: PathBuf, cx: &mut Context<Self>) -> Self {
        let mut sheets: HashMap<String, Cells> = HashMap::default();
        for (category, groups) in map.categories.iter().enumerate() {
            for (group, at) in groups.groups.iter().enumerate() {
                if let Some(Icon::Image { url, cell }) = &at.icon {
                    sheets.entry(url.clone()).or_default().push(((category, group), *cell));
                }
            }
        }
        let task = cx.spawn(async move |icons, cx| {
            for (url, cells) in sheets {
                let cache = cache.clone();
                let cut = unblock(move || cut(&cache, &url, cells)).await;
                let added = icons.update(cx, |icons, cx| {
                    icons.cut.extend(cut);
                    cx.notify();
                });
                if added.is_err() {
                    return;
                }
            }
        });
        Self { cut: HashMap::default(), _cut: task }
    }

    pub fn get(&self, group: (usize, usize)) -> Option<&RgbaImage> {
        self.cut.get(&group)
    }
}

/// `cells` of the sheet at `url` (blocking); a group whose sheet or cell is unusable keeps the default mark.
fn cut(cache: &Path, url: &str, cells: Cells) -> Vec<((usize, usize), RgbaImage)> {
    let sheet = match art::load(cache, url).and_then(|bytes| art::picture(&bytes, SHEET)) {
        Ok(sheet) => sheet.into_rgba8(),
        Err(e) => {
            log::debug!("icon sheet {url}: {e:#}");
            return Vec::new();
        }
    };
    let within = |start: u32, length: u32, end: u32| length > 0 && start.checked_add(length).is_some_and(|stop| stop <= end);
    let cut = |[left, top, width, height]: [u32; 4]| {
        let icon = imageops::crop_imm(&sheet, left, top, width, height).to_image();
        let scale = f64::from(KEPT) / f64::from(width.max(height));
        let side = |length: u32| ((f64::from(length) * scale).round() as u32).max(1);
        if scale < 1. { imageops::resize(&icon, side(width), side(height), imageops::FilterType::Triangle) } else { icon }
    };
    cells
        .into_iter()
        .map(|(group, cell)| (group, cell.unwrap_or([0, 0, sheet.width(), sheet.height()])))
        .filter(|(_, [left, top, width, height])| within(*left, *width, sheet.width()) && within(*top, *height, sheet.height()))
        .map(|(group, cell)| (group, cut(cell)))
        .collect()
}

#[cfg(test)]
mod tests {
    use image::{ExtendedColorType, ImageEncoder, codecs::png::PngEncoder};

    #[test]
    fn icons_are_cut_from_their_sheet_and_kept_small() {
        let cache = std::env::temp_dir().join(format!("impulse-icons-{}", std::process::id()));
        std::fs::create_dir_all(&cache).unwrap();
        // A 200 x 100 sheet: red on the left half, blue on the right.
        let texels: Vec<u8> = (0..200 * 100).flat_map(|at| if at % 200 < 100 { [255, 0, 0, 255] } else { [0, 0, 255, 255] }).collect();
        let mut png = Vec::new();
        PngEncoder::new(&mut png).write_image(&texels, 200, 100, ExtendedColorType::Rgba8).unwrap();
        let url = "https://cdn/sheet.png";
        std::fs::write(client::media::file(&cache, url), png).unwrap();
        // Empty: the server lacks it, so nothing is downloaded.
        std::fs::write(client::media::file(&cache, "https://cdn/gone.png"), []).unwrap();
        let cells = vec![((0, 0), Some([100, 0, 100, 100])), ((0, 1), None), ((1, 0), Some([150, 0, 100, 100]))];
        let cut: Vec<_> =
            super::cut(&cache, url, cells).into_iter().map(|(group, icon)| (group, icon.dimensions(), icon.get_pixel(10, 10).0)).collect();
        assert_eq!(cut, [((0, 0), (64, 64), [0, 0, 255, 255]), ((0, 1), (64, 32), [255, 0, 0, 255])], "a cell past the edge is left out");
        assert!(super::cut(&cache, "https://cdn/gone.png", vec![((1, 1), None)]).is_empty(), "a sheet not there");
        let _ = std::fs::remove_dir_all(&cache);
    }
}
