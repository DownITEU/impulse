//! The map's tiles as images: a bounded number kept, the least recently drawn dropped.

use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use gpui_kit::RenderImage;
use rustc_hash::FxHashMap as HashMap;

use client::media::Fetched;

/// Level, column, row.
pub type Key = (u8, u32, u32);

// Tiles kept per tile the last frame drew; the rest are the levels and places just left.
const KEPT: usize = 2;
// A zoomed out view draws a few tiles, and a burst of loads must not push its stand-ins out.
const FLOOR: usize = 64;
// The minimap never zooms far: its view draws a handful.
const SMALL: usize = 16;
// A tile from the CDN takes about 100 ms: a view of some 50 fills in half a second.
const AT_ONCE: usize = 12;
/// How many levels up a missing tile looks for a stand-in.
pub const STAND_INS: u8 = 4;
// And down: a zoom out just left those.
const BELOW: u8 = 2;
/// Levels above the view's whose few tiles load first and stay while they cover it: a zoom out or a pan shows them
/// at once. Within `STAND_INS`.
pub const BACKDROP: u8 = 2;
// A failed tile waits a second, doubling per failure up to about a minute; only a draw asks again.
const BACKOFF: u32 = 6;

enum Slot {
    /// With the failures before it.
    Loading(u32),
    /// With the frame it was last drawn in.
    Ready(Arc<RenderImage>, u64),
    /// Not on the server, or not a picture: not asked for again while this map shows.
    Absent,
    Failed {
        due: Instant,
        tries: u32,
    },
}

/// What a missing tile asks for.
enum Ask {
    /// Itself, with the failures before: not known yet, or its wait after a failure is over.
    Load(u32),
    /// Its parent: the server lacks it, or it waits.
    Parent,
    /// Nothing: it is on its way or loaded.
    Stop,
}

pub struct Tiles {
    slots: HashMap<Key, Slot>,
    loading: usize,
    frame: u64,
    // Tiles drawn in this frame, each once.
    shown: usize,
    floor: usize,
}

impl Default for Tiles {
    fn default() -> Self {
        Self::new(FLOOR)
    }
}

impl Tiles {
    fn new(floor: usize) -> Self {
        Self { slots: HashMap::default(), loading: 0, frame: 0, shown: 0, floor }
    }

    pub fn small() -> Self {
        Self::new(SMALL)
    }

    /// Starts a frame; the caller drops the returned images from the atlas.
    pub fn begin_frame(&mut self) -> Vec<Arc<RenderImage>> {
        let (last, kept) = (self.frame, (self.shown * KEPT).max(self.floor));
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
    pub fn draw(&mut self, (level, column, row): Key, mut paint: impl FnMut(Key, Arc<RenderImage>)) {
        if let Some(image) = self.touch((level, column, row)) {
            paint((level, column, row), image.clone());
            return;
        }
        let above = (1..=level.min(STAND_INS))
            .map(|up| (level - up, column >> up, row >> up))
            .find_map(|key| self.touch(key).map(|image| (key, image.clone())));
        if let Some((key, image)) = above {
            paint(key, image);
        }
        self.below((level, column, row), BELOW, &mut paint);
    }

    fn below(&mut self, (level, column, row): Key, depth: u8, paint: &mut impl FnMut(Key, Arc<RenderImage>)) {
        for (x, y) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
            let child = (level + 1, column * 2 + x, row * 2 + y);
            match self.touch(child) {
                Some(image) => paint(child, image.clone()),
                None if depth > 1 => self.below(child, depth - 1, paint),
                None => {}
            }
        }
    }

    fn touch(&mut self, key: Key) -> Option<&Arc<RenderImage>> {
        let Some(Slot::Ready(image, drawn)) = self.slots.get_mut(&key) else { return None };
        self.shown += usize::from(*drawn != self.frame);
        *drawn = self.frame;
        Some(image)
    }

    /// A tile the view needs, or for one the server lacks or that waits after a failure its nearest ancestor down to
    /// level `lowest`: kept as a drawn one when loaded, else returned to load.
    pub fn need(&mut self, (level, column, row): Key, lowest: u8, now: Instant) -> Option<Key> {
        let (key, ask) = (0..=level.saturating_sub(lowest).min(STAND_INS))
            .map(|up| (level - up, column >> up, row >> up))
            .map(|key| (key, self.ask(&key, now)))
            .find(|(_, ask)| !matches!(ask, Ask::Parent))?;
        if matches!(ask, Ask::Load(_)) {
            return Some(key);
        }
        self.touch(key);
        None
    }

    /// Every load is taken: nothing more starts until one is back.
    pub fn busy(&self) -> bool {
        self.loading >= AT_ONCE
    }

    /// Whether the caller loads it now: it is not known yet, or its wait is over, and a load is free.
    pub fn claim(&mut self, key: Key, now: Instant) -> bool {
        let Ask::Load(tries) = self.ask(&key, now) else { return false };
        if self.busy() {
            return false;
        }
        self.slots.insert(key, Slot::Loading(tries));
        self.loading += 1;
        true
    }

    pub fn loaded(&mut self, key: Key, fetched: Fetched<Arc<RenderImage>>, now: Instant) {
        self.loading -= 1;
        let tries = match self.slots.get(&key) {
            Some(Slot::Loading(tries)) => *tries,
            _ => 0,
        };
        let slot = match fetched {
            Fetched::Ready(image) => Slot::Ready(image, self.frame),
            Fetched::Absent => Slot::Absent,
            Fetched::Retry => Slot::Failed { due: now + Duration::from_secs(1 << tries.min(BACKOFF)), tries: tries + 1 },
        };
        self.slots.insert(key, slot);
    }

    fn ask(&self, key: &Key, now: Instant) -> Ask {
        match self.slots.get(key) {
            None => Ask::Load(0),
            Some(Slot::Failed { due, tries }) if *due <= now => Ask::Load(*tries),
            Some(Slot::Absent | Slot::Failed { .. }) => Ask::Parent,
            Some(Slot::Loading(_) | Slot::Ready(..)) => Ask::Stop,
        }
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

    fn image() -> Fetched<Arc<RenderImage>> {
        Fetched::Ready(Arc::new(RenderImage::new(Vec::new())))
    }

    fn load(tiles: &mut Tiles, key: Key) {
        let now = Instant::now();
        assert!(tiles.claim(key, now));
        tiles.loaded(key, image(), now);
    }

    fn painted(tiles: &mut Tiles, key: Key) -> Vec<Key> {
        let mut keys = Vec::new();
        tiles.draw(key, |key, _| keys.push(key));
        keys
    }

    fn exact(tiles: &mut Tiles, key: Key) -> bool {
        painted(tiles, key) == [key]
    }

    #[test]
    fn stand_ins_come_from_above_and_below() {
        let (mut tiles, now) = (Tiles::default(), Instant::now());
        assert!(tiles.claim((2, 1, 0), now) && !tiles.claim((2, 1, 0), now));
        tiles.loaded((2, 1, 0), image(), now);
        assert_eq!(painted(&mut tiles, (4, 6, 3)), [(2, 1, 0)]);
        assert!(painted(&mut tiles, (4, 9, 3)).is_empty());
        assert_eq!(tiles.need((4, 6, 3), 0, now), Some((4, 6, 3)));
        assert!(tiles.claim((4, 6, 3), now));
        assert_eq!(tiles.need((4, 6, 3), 0, now), None, "on its way");
        tiles.loaded((4, 6, 3), Fetched::Absent, now);
        assert!(!tiles.claim((4, 6, 3), now), "an absent tile is not asked for again");
        assert_eq!(tiles.need((4, 6, 3), 0, now), Some((3, 3, 1)), "its parent is asked for instead");
        assert_eq!(tiles.need((4, 6, 3), 4, now), None, "but not below the lowest level");
        // A zoom out: the tiles just left lie over the ancestor, two levels down at most.
        load(&mut tiles, (4, 7, 2));
        load(&mut tiles, (5, 12, 4));
        load(&mut tiles, (6, 24, 8));
        assert_eq!(painted(&mut tiles, (3, 3, 1)), [(2, 1, 0), (5, 12, 4), (4, 7, 2)]);
        assert_eq!(painted(&mut tiles, (4, 7, 2)), [(4, 7, 2)]);
    }

    #[test]
    fn a_failed_tile_is_asked_again_after_a_growing_wait() {
        let (mut tiles, now) = (Tiles::default(), Instant::now());
        let key = (3, 2, 2);
        assert!(tiles.claim(key, now));
        tiles.loaded(key, Fetched::Retry, now);
        assert_eq!(tiles.need(key, 0, now), Some((2, 1, 1)), "its parent stands in meanwhile");
        let later = now + Duration::from_secs(1);
        assert_eq!(tiles.need(key, 0, later), Some(key));
        assert!(tiles.claim(key, later));
        tiles.loaded(key, Fetched::Retry, later);
        assert!(!tiles.claim(key, later + Duration::from_secs(1)), "the second wait is longer");
        assert!(tiles.claim(key, later + Duration::from_secs(2)));
        tiles.loaded(key, image(), later);
        assert!(exact(&mut tiles, key));
    }

    #[test]
    fn loads_are_bounded_and_old_tiles_leave() {
        let (mut tiles, now) = (Tiles::default(), Instant::now());
        let claimed = (0..20).filter(|column| tiles.claim((5, *column, 0), now)).count();
        assert_eq!(claimed, AT_ONCE);
        assert_eq!(tiles.clear().len(), 0);
        (0..FLOOR as u32 + 10).for_each(|column| load(&mut tiles, (9, column, 0)));
        assert!(tiles.begin_frame().is_empty(), "just loaded, not drawn yet");
        // Drawn twice, counted once: three shown, the floor kept.
        [0, 1, 2, 0].into_iter().for_each(|column| assert!(exact(&mut tiles, (9, column, 0))));
        assert_eq!(tiles.begin_frame().len(), 10);
        assert!(exact(&mut tiles, (9, 0, 0)) && !exact(&mut tiles, (9, 3, 0)));
        assert_eq!(tiles.clear().len(), FLOOR);
    }

    #[test]
    fn a_tile_the_view_needs_stays_though_not_drawn() {
        let (mut tiles, now) = (Tiles::default(), Instant::now());
        (0..FLOOR as u32).for_each(|column| load(&mut tiles, (9, column, 0)));
        // An absent tile's parent stands in for it: kept as well.
        load(&mut tiles, (5, 0, 0));
        assert!(tiles.claim((6, 0, 0), now));
        tiles.loaded((6, 0, 0), Fetched::Absent, now);
        tiles.begin_frame();
        assert_eq!(tiles.need((9, 0, 0), 0, now), None, "loaded");
        assert_eq!(tiles.need((6, 0, 0), 0, now), None, "its parent is loaded");
        assert_eq!(tiles.begin_frame().len(), 1);
        assert!(exact(&mut tiles, (9, 0, 0)) && exact(&mut tiles, (5, 0, 0)));
    }

    #[test]
    fn a_burst_of_loads_keeps_what_the_view_draws() {
        let mut tiles = Tiles::default();
        (0..FLOOR as u32).for_each(|column| load(&mut tiles, (9, column, 0)));
        load(&mut tiles, (1, 0, 0));
        tiles.begin_frame();
        // Zoomed out: one ancestor stands in for the view, and its tiles arrive at once.
        assert_eq!(painted(&mut tiles, (3, 3, 3)), [(1, 0, 0)]);
        let arrived: Vec<Key> = (0..AT_ONCE as u32).map(|index| (3, index % 3, index / 3)).collect();
        arrived.iter().for_each(|key| load(&mut tiles, *key));
        tiles.begin_frame();
        assert_eq!(painted(&mut tiles, (3, 3, 3)), [(1, 0, 0)]);
        assert!(arrived.iter().all(|key| exact(&mut tiles, *key)));
    }
}
