//! The map's tiles as images: a bounded number kept, the least recently drawn dropped.

use std::sync::Arc;

use gpui_kit::RenderImage;
use rustc_hash::FxHashMap as HashMap;

/// Level, column, row.
pub type Key = (u8, u32, u32);

// Tiles kept per tile the last frame drew; the rest are the levels and places just left.
const KEPT: usize = 2;
// A zoomed out view draws a few tiles, and a burst of loads must not push its stand-ins out.
const FLOOR: usize = 64;
const AT_ONCE: usize = 6;
// How many levels up a missing tile looks for a stand-in, and down (a zoom out just left those).
const STAND_INS: u8 = 4;
const BELOW: u8 = 2;

enum Slot {
    Loading,
    /// With the frame it was last drawn in.
    Ready(Arc<RenderImage>, u64),
    /// Not on the server, or it failed: not asked for again while this map shows.
    Missing,
}

#[derive(Default)]
pub struct Tiles {
    slots: HashMap<Key, Slot>,
    loading: usize,
    frame: u64,
    // Tiles drawn in this frame, each once.
    shown: usize,
}

impl Tiles {
    /// Starts a frame; the caller drops the returned images from the atlas.
    pub fn begin_frame(&mut self) -> Vec<Arc<RenderImage>> {
        let (last, kept) = (self.frame, (self.shown * KEPT).max(FLOOR));
        (self.frame, self.shown) = (self.frame + 1, 0);
        let ready = self.slots.values().filter(|slot| matches!(slot, Slot::Ready(..))).count();
        if ready <= kept {
            return Vec::new();
        }
        // What the last frame drew or a load just brought stays.
        let mut idle: Vec<(u64, Key)> = self
            .slots
            .iter()
            .filter_map(|(key, slot)| match slot {
                Slot::Ready(_, drawn) if *drawn < last => Some((*drawn, *key)),
                _ => None,
            })
            .collect();
        idle.sort_unstable();
        let extra = (ready - kept).min(idle.len());
        idle[..extra].iter().filter_map(|(_, key)| self.slots.remove(key).and_then(Slot::image)).collect()
    }

    /// Paints the tile, else what stands in for it: a loaded ancestor, with the loaded tiles below it over that.
    /// True when the tile itself was painted.
    pub fn draw(&mut self, (level, column, row): Key, mut paint: impl FnMut(Key, Arc<RenderImage>)) -> bool {
        if let Some(image) = self.touch((level, column, row)) {
            paint((level, column, row), image);
            return true;
        }
        let above = (1..=level.min(STAND_INS)).map(|up| (level - up, column >> up, row >> up)).find_map(|key| self.touch(key).map(|image| (key, image)));
        if let Some((key, image)) = above {
            paint(key, image);
        }
        self.below((level, column, row), BELOW, &mut paint);
        false
    }

    fn below(&mut self, (level, column, row): Key, depth: u8, paint: &mut impl FnMut(Key, Arc<RenderImage>)) {
        for (x, y) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
            let child = (level + 1, column * 2 + x, row * 2 + y);
            match self.touch(child) {
                Some(image) => paint(child, image),
                None if depth > 1 => self.below(child, depth - 1, paint),
                None => {}
            }
        }
    }

    // Marks a loaded tile drawn in this frame.
    fn touch(&mut self, key: Key) -> Option<Arc<RenderImage>> {
        let Some(Slot::Ready(image, drawn)) = self.slots.get_mut(&key) else { return None };
        self.shown += usize::from(*drawn != self.frame);
        *drawn = self.frame;
        Some(image.clone())
    }

    /// What to load for a tile not drawn as itself: the tile, or for one the server lacks, its nearest ancestor.
    pub fn wanted(&self, (level, column, row): Key) -> Option<Key> {
        (0..=level.min(STAND_INS))
            .map(|up| (level - up, column >> up, row >> up))
            .find(|key| !matches!(self.slots.get(key), Some(Slot::Missing)))
            .filter(|key| !self.slots.contains_key(key))
    }

    /// Whether the caller loads it now: it is not known yet and a load is free.
    pub fn claim(&mut self, key: Key) -> bool {
        let free = self.loading < AT_ONCE && !self.slots.contains_key(&key);
        if free {
            self.slots.insert(key, Slot::Loading);
            self.loading += 1;
        }
        free
    }

    pub fn loaded(&mut self, key: Key, image: Option<Arc<RenderImage>>) {
        self.loading -= 1;
        let slot = image.map_or(Slot::Missing, |image| Slot::Ready(image, self.frame));
        self.slots.insert(key, slot);
    }

    /// Forgets every tile; the caller drops the returned images from the atlas.
    /// A load claimed before it must not report back.
    pub fn clear(&mut self) -> Vec<Arc<RenderImage>> {
        self.loading = 0;
        self.slots.drain().filter_map(|(_, slot)| slot.image()).collect()
    }
}

impl Slot {
    fn image(self) -> Option<Arc<RenderImage>> {
        match self {
            Self::Ready(image, _) => Some(image),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn image() -> Option<Arc<RenderImage>> {
        Some(Arc::new(RenderImage::new(Vec::new())))
    }

    fn load(tiles: &mut Tiles, key: Key) {
        assert!(tiles.claim(key));
        tiles.loaded(key, image());
    }

    fn painted(tiles: &mut Tiles, key: Key) -> (bool, Vec<Key>) {
        let mut keys = Vec::new();
        let exact = tiles.draw(key, |key, _| keys.push(key));
        (exact, keys)
    }

    #[test]
    fn stand_ins_come_from_above_and_below() {
        let mut tiles = Tiles::default();
        assert!(tiles.claim((2, 1, 0)) && !tiles.claim((2, 1, 0)));
        tiles.loaded((2, 1, 0), image());
        assert_eq!(painted(&mut tiles, (4, 6, 3)), (false, vec![(2, 1, 0)]));
        assert_eq!(painted(&mut tiles, (4, 9, 3)), (false, vec![]));
        assert_eq!(tiles.wanted((4, 6, 3)), Some((4, 6, 3)));
        assert!(tiles.claim((4, 6, 3)));
        assert_eq!(tiles.wanted((4, 6, 3)), None, "on its way");
        tiles.loaded((4, 6, 3), None);
        assert!(!tiles.claim((4, 6, 3)), "a missing tile is not asked for again");
        assert_eq!(tiles.wanted((4, 6, 3)), Some((3, 3, 1)), "its parent is asked for instead");
        // A zoom out: the tiles just left lie over the ancestor, two levels down at most.
        load(&mut tiles, (4, 7, 2));
        load(&mut tiles, (5, 12, 4));
        load(&mut tiles, (6, 24, 8));
        assert_eq!(painted(&mut tiles, (3, 3, 1)), (false, vec![(2, 1, 0), (5, 12, 4), (4, 7, 2)]));
        assert_eq!(painted(&mut tiles, (4, 7, 2)), (true, vec![(4, 7, 2)]));
    }

    #[test]
    fn loads_are_bounded_and_old_tiles_leave() {
        let mut tiles = Tiles::default();
        let claimed = (0..20).filter(|column| tiles.claim((5, *column, 0))).count();
        assert_eq!(claimed, AT_ONCE);
        assert_eq!(tiles.clear().len(), 0);
        (0..FLOOR as u32 + 10).for_each(|column| load(&mut tiles, (9, column, 0)));
        assert!(tiles.begin_frame().is_empty(), "just loaded, not drawn yet");
        // Drawn twice, counted once: three shown, the floor kept.
        [0, 1, 2, 0].into_iter().for_each(|column| assert!(painted(&mut tiles, (9, column, 0)).0));
        assert_eq!(tiles.begin_frame().len(), 10);
        assert!(painted(&mut tiles, (9, 0, 0)).0 && !painted(&mut tiles, (9, 3, 0)).0);
        assert_eq!(tiles.clear().len(), FLOOR);
    }

    #[test]
    fn a_burst_of_loads_keeps_what_the_view_draws() {
        let mut tiles = Tiles::default();
        (0..FLOOR as u32).for_each(|column| load(&mut tiles, (9, column, 0)));
        load(&mut tiles, (1, 0, 0));
        tiles.begin_frame();
        // Zoomed out: one ancestor stands in for the view, and its tiles arrive at once.
        assert_eq!(painted(&mut tiles, (3, 3, 3)), (false, vec![(1, 0, 0)]));
        let arrived: Vec<Key> = (0..AT_ONCE as u32).map(|index| (3, index % 3, index / 3)).collect();
        arrived.iter().for_each(|key| load(&mut tiles, *key));
        tiles.begin_frame();
        assert_eq!(painted(&mut tiles, (3, 3, 3)), (false, vec![(1, 0, 0)]));
        assert!(arrived.iter().all(|key| painted(&mut tiles, *key).0));
    }
}
