//! The live game's map for the minimap, whatever page is open: picked, loaded and followed on a board of its
//! own, which shares the big map's copy and the user's state of it when both show the same map.

use std::{
    path::{Path, PathBuf},
    rc::Rc,
    sync::Arc,
    time::Duration,
};

use anyhow::Result;
use futures::StreamExt;
use gpui_kit::{App, AsyncApp, Context, Entity, EntityId, Subscription, Task, WeakEntity};
use model::{Map, MapOffer, Provider, Snapshot};
use serde::{Deserialize, Serialize};

use super::{
    board::{Board, BoardEvent, Run},
    tracker::{Tracked, Tracker},
};
use crate::{Load, store};

const FILE: &str = "minimap.json";
// Tile levels above the map's deepest: where the minimap starts and how far the wheel takes it.
const ZOOM: f64 = 1.;
const ZOOMS: (f64, f64) = (-1., 6.);
// A turn of the wheel is kept once it rests.
const SETTLE: Duration = Duration::from_millis(500);

/// A live run whose trainer can place the player, with its game's maps.
pub struct LiveRun {
    pub offers: Vec<MapOffer>,
    pub run: Run,
}

#[derive(Serialize, Deserialize)]
struct Kept {
    zoom: f64,
}

pub struct LiveMap {
    provider: Arc<dyn Provider>,
    pub(super) board: Entity<Board>,
    big: Entity<Board>,
    pub(super) tile_dir: PathBuf,
    file: PathBuf,
    offers: Vec<MapOffer>,
    // The run's tracker.
    following: Option<EntityId>,
    // The trainer tells where the player is now: not in menus and loading screens.
    placed: bool,
    zoom: f64,
    _tracked: Option<Subscription>,
    _load: Task<()>,
    _save: Task<()>,
}

impl LiveMap {
    pub(super) fn new(
        provider: Arc<dyn Provider>,
        board: Entity<Board>,
        big: Entity<Board>,
        tile_dir: PathBuf,
        data_dir: &Path,
        cx: &mut Context<Self>,
    ) -> Self {
        // Every change to the big board's state of a map makes a new copy, and comes with a notify.
        cx.observe(&big, |live, _, cx| live.adopt(cx)).detach();
        // A copy either board installs goes to the other when it shows the same map, so both hold one.
        cx.subscribe(&big, |live, big, event: &BoardEvent, cx| {
            if matches!(event, BoardEvent::Installed { .. }) {
                share(&big, &live.board, cx);
            }
        })
        .detach();
        cx.subscribe(&board, |live, board, event: &BoardEvent, cx| {
            if matches!(event, BoardEvent::Installed { .. }) {
                share(&board, &live.big, cx);
                // The HUD may show it now, with the big board's state of the same map.
                live.adopt(cx);
                cx.notify();
            }
        })
        .detach();
        let file = data_dir.join(FILE);
        let zoom = store::load::<Kept>(&file).map_or(ZOOM, |kept| kept.zoom);
        Self {
            provider,
            board,
            big,
            tile_dir,
            file,
            offers: Vec::new(),
            following: None,
            placed: false,
            zoom,
            _tracked: None,
            _load: Task::ready(()),
            _save: Task::ready(()),
        }
    }

    /// The map is up and can place the player.
    pub fn ready(&self, cx: &App) -> bool {
        matches!(&self.board.read(cx).map, Load::Ready(map) if map.frame.is_some())
    }

    /// Known only while some map follows the player.
    pub fn placed(&self) -> bool {
        self.placed
    }

    pub fn accent(&self, cx: &App) -> u32 {
        self.board.read(cx).accent
    }

    /// While shown over the game, the minimap follows the player closely.
    pub fn set_shown(&self, shown: bool, cx: &mut App) {
        self.board.update(cx, |board, cx| board.set_open(shown, cx));
    }

    pub(super) fn set_run(&mut self, run: Option<LiveRun>, cx: &mut Context<Self>) {
        let Some(LiveRun { offers, run }) = run else {
            (self.offers, self.following, self.placed, self._tracked, self._load) = (Vec::new(), None, false, None, Task::ready(()));
            self.board.update(cx, |board, cx| {
                board.forget();
                board.set_run(None, cx);
            });
            return cx.notify();
        };
        // Asked again for the run it follows: nothing changes.
        let tracker = run.tracker.as_ref().map(Entity::entity_id);
        if tracker == self.following && offers == self.offers {
            return;
        }
        if tracker != self.following {
            self.track(run.tracker.as_ref(), cx);
        }
        self.following = tracker;
        self.board.update(cx, |board, cx| board.set_run(Some(run), cx));
        if offers != self.offers {
            self.offers = offers;
            self.load(cx);
        }
    }

    /// Tells only when the player comes or goes, not each fix.
    fn track(&mut self, tracker: Option<&Entity<Tracker>>, cx: &mut Context<Self>) {
        self.placed = tracker.is_some_and(|tracker| tracker.read(cx).last().is_some());
        self._tracked = tracker.map(|tracker| {
            cx.subscribe(tracker, |live, _, tracked: &Tracked, cx| {
                let placed = matches!(tracked, Tracked::Fix(_));
                if placed != live.placed {
                    live.placed = placed;
                    cx.notify();
                }
            })
        });
        cx.notify();
    }

    /// The big map's choice first when it is one of the game's maps, then the rest in order, until one can
    /// place the player.
    fn load(&mut self, cx: &mut Context<Self>) {
        let mut offers = self.offers.clone();
        if let Some(at) = offers.iter().position(|offer| offer.id == self.big.read(cx).id()) {
            offers[..=at].rotate_right(1);
        }
        let (provider, board, big) = (self.provider.clone(), self.board.downgrade(), self.big.downgrade());
        self._load = cx.spawn(async move |live, cx| {
            let _ = load(live, board, big, provider, offers, cx).await;
        });
    }

    /// Takes the big board's state of the same map; only that board writes it.
    fn adopt(&mut self, cx: &mut Context<Self>) {
        let (big, board) = (self.big.read(cx), self.board.read(cx));
        let same = big.loaded(board.id()).is_some() && matches!(board.map, Load::Ready(_));
        if !same || Rc::ptr_eq(&big.saved, &board.saved) {
            return;
        }
        let saved = big.saved.clone();
        self.board.update(cx, |board, cx| board.adopt(saved, cx));
    }

    /// The camera's span for `map` at the kept zoom; `tile` is a tile's side in view pixels.
    pub(super) fn span(&self, map: &Map, tile: f64) -> f64 {
        tile * 2f64.powf(f64::from(map.tiles.max_zoom) - self.zoom)
    }

    /// Closer for more `levels`, further for fewer.
    pub(super) fn zoom_by(&mut self, levels: f64, cx: &mut Context<Self>) {
        self.zoom = (self.zoom - levels).clamp(ZOOMS.0, ZOOMS.1);
        let (file, kept) = (self.file.clone(), Kept { zoom: self.zoom });
        self._save = cx.spawn(async move |_, cx| {
            cx.background_executor().timer(SETTLE).await;
            store::save(&file, &kept);
        });
        cx.notify();
    }

    pub(super) fn set_accent(&self, accent: u32, cx: &mut Context<Self>) {
        self.board.update(cx, |board, cx| board.set_accent(accent, cx));
        cx.notify();
    }
}

/// `to` shows the map `from` just installed: it takes the same copy.
fn share(from: &Entity<Board>, to: &Entity<Board>, cx: &mut App) {
    let (map, wanted) = {
        let (from, to) = (from.read(cx), to.read(cx));
        let Some(map) = from.loaded(from.id()) else { return };
        let held = to.loaded(from.id()).is_some_and(|held| Rc::ptr_eq(&held, &map));
        (map, to.id() == from.id() && !held)
    };
    if wanted {
        to.update(cx, |board, cx| board.install(map, cx));
    }
}

/// Loads `offers` in order until one can place the player; none can: the last is let go of.
async fn load(
    live: WeakEntity<LiveMap>,
    board: WeakEntity<Board>,
    big: WeakEntity<Board>,
    provider: Arc<dyn Provider>,
    offers: Vec<MapOffer>,
    cx: &mut AsyncApp,
) -> Result<()> {
    for offer in offers {
        let shared = big.read_with(cx, |big, _| big.loaded(&offer.id))?;
        board.update(cx, |board, cx| board.pending(offer.id.clone(), cx))?;
        if let Some(map) = shared {
            board.update(cx, |board, cx| board.install(map, cx))?;
        } else {
            let mut snapshots = provider.map(&offer);
            while let Some(snapshot) = snapshots.next().await {
                board.update(cx, |board, cx| match snapshot {
                    // The big board shared its copy meanwhile.
                    Snapshot::Cached(Some(_)) if matches!(board.map, Load::Ready(_)) => {}
                    Snapshot::Cached(Some(map)) | Snapshot::Fresh(Ok(map)) => board.install(Rc::new(map), cx),
                    Snapshot::Cached(None) => {}
                    Snapshot::Fresh(Err(e)) => board.fail(format!("{e:#}"), cx),
                })?;
            }
        }
        if live.read_with(cx, |live, cx| live.ready(cx))? {
            return Ok(());
        }
    }
    board.update(cx, |board, _| board.forget())?;
    live.update(cx, |_, cx| cx.notify())
}
