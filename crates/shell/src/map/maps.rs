//! The map's model: the open game's maps and the one chosen, loading it, and the live run both the map and the
//! minimap follow.

use std::{sync::Arc, time::Duration};

use client::{Registry, library::GameId, media::Media, storage::Paths};
use gpui_kit::{App, AppContext, Context, Entity, Task};
use model::MapOffer;

use super::{
    board::{Board, show},
    live::{LiveMap, LiveRun},
};
use crate::{
    library::Library,
    run::{RunEvent, Runs},
    ui::text::{Texts, content_locale},
};

// A closed map keeps its pins this long, so opening it again soon is instant; later they are read from disk again.
// While a trainer is live in its game, it keeps them until the run ends.
const KEPT_CLOSED: Duration = Duration::from_secs(3 * 60);

/// The stage's camera: center and span.
type Camera = Box<dyn Fn(&App) -> Option<(f64, f64, f64)>>;

pub struct Maps {
    registry: Arc<Registry>,
    library: Entity<Library>,
    runs: Entity<Runs>,
    board: Entity<Board>,
    live_map: Entity<LiveMap>,
    // The game whose maps are listed, and the one a trainer is live in.
    game: Option<GameId>,
    live: Option<GameId>,
    offers: Vec<MapOffer>,
    chosen: usize,
    // The minimap's dock is shown.
    minimap: bool,
    camera: Option<Camera>,
    _load: Task<()>,
    _forget: Task<()>,
}

impl Maps {
    pub fn new(registry: Arc<Registry>, paths: &Paths, library: Entity<Library>, runs: Entity<Runs>, cx: &mut Context<Self>) -> Self {
        cx.subscribe(&runs, |maps, _, event: &RunEvent, cx| {
            if !matches!(event, RunEvent::Pressed(_)) {
                maps.follow(cx);
            }
        })
        .detach();
        cx.observe_global::<Texts>(Self::relocalize).detach();
        let art = Media::Pictures.dir(paths);
        let board = cx.new(|_| Board::new(paths.map_state(), art.clone()));
        let live_board = cx.new(|_| Board::live(paths.map_state(), art));
        let live_map = cx.new(|cx| LiveMap::new(registry.clone(), live_board, board.clone(), Media::Tiles.dir(paths), paths, cx));
        Self {
            registry,
            library,
            runs,
            board,
            live_map,
            game: None,
            live: None,
            offers: Vec::new(),
            chosen: 0,
            minimap: false,
            camera: None,
            _load: Task::ready(()),
            _forget: Task::ready(()),
        }
    }

    pub fn is_open(&self, cx: &App) -> bool {
        self.board.read(cx).is_open()
    }

    pub fn live_map(&self) -> &Entity<LiveMap> {
        &self.live_map
    }

    pub(super) fn board(&self) -> &Entity<Board> {
        &self.board
    }

    pub(super) fn offers(&self) -> &[MapOffer] {
        &self.offers
    }

    pub(super) fn chosen(&self) -> usize {
        self.chosen
    }

    /// Where the camera is read: the board keeps it with the map's state before it lets go of the map.
    pub(super) fn set_camera(&mut self, camera: impl Fn(&App) -> Option<(f64, f64, f64)> + 'static) {
        self.camera = Some(Box::new(camera));
    }

    /// Opens on `offers`, the maps of `game`. The same game keeps its map and the view of it.
    pub(super) fn open(&mut self, game: &GameId, offers: &[MapOffer], cx: &mut Context<Self>) {
        self._forget = Task::ready(());
        let listed = self.game.as_ref() == Some(game) && self.offers == offers;
        if !listed {
            (self.game, self.offers) = (Some(game.clone()), offers.to_vec());
        }
        if !listed || self.board.read(cx).unloaded() {
            self.choose(if listed { self.chosen } else { 0 }, cx);
        }
        self.board.update(cx, |board, cx| board.set_open(true, cx));
        self.follow(cx);
        cx.notify();
    }

    pub(super) fn close(&mut self, cx: &mut Context<Self>) {
        self.keep_view(cx);
        self.board.update(cx, |board, cx| {
            board.save();
            board.set_open(false, cx);
        });
        self.forget_later(cx);
        cx.notify();
    }

    /// The board saves the map it lets go of.
    pub(super) fn choose(&mut self, index: usize, cx: &mut Context<Self>) {
        self.keep_view(cx);
        self.chosen = index;
        cx.notify();
        let Some(offer) = self.offers.get(index).cloned() else { return };
        let locale = content_locale(cx);
        let shared = self.live_map.read(cx).board.read(cx).loaded(&offer.id, &locale);
        self.board.update(cx, |board, cx| board.pending(offer.id.clone(), locale, cx));
        // The minimap's copy, when it shows this map; a load of its own unless the minimap's is done.
        if self.board.update(cx, |board, cx| board.take(shared, cx)) {
            self._load = Task::ready(());
            return;
        }
        self.load(&offer, cx);
    }

    fn load(&mut self, offer: &MapOffer, cx: &mut Context<Self>) {
        let snapshots = self.registry.map(offer, &content_locale(cx));
        let board = self.board.downgrade();
        self._load = cx.spawn(async move |_, cx| {
            let _ = show(&board, snapshots, cx).await;
        });
    }

    /// Another content language: the open map loads again in it, a closed one lets go of its copy; the minimap's
    /// search starts over.
    fn relocalize(&mut self, cx: &mut Context<Self>) {
        if self.is_open(cx) {
            self.choose(self.chosen, cx);
        } else {
            self._load = Task::ready(());
            self.board.update(cx, |board, _| board.forget());
        }
        self.live_map.update(cx, |live, cx| live.relocalize(cx));
    }

    fn keep_view(&mut self, cx: &mut Context<Self>) {
        let Some(camera) = &self.camera else { return };
        let view = camera(cx);
        self.board.update(cx, |board, _| board.saved_mut().view = view);
    }

    /// The minimap follows the live run while its dock is shown.
    pub fn set_minimap(&mut self, wanted: bool, cx: &mut Context<Self>) {
        self.minimap = wanted;
        self.follow_minimap(cx);
    }

    /// The open map follows the live run of its game; the minimap the live run, wherever.
    fn follow(&mut self, cx: &mut Context<Self>) {
        let open = self.is_open(cx);
        let runs = self.runs.read(cx);
        let run = runs.live_run().filter(|(game, _)| open && self.game.as_ref() == Some(*game)).map(|(_, run)| run.clone());
        let live = runs.live().cloned();
        self.board.update(cx, |board, cx| board.set_run(run, cx));
        if self.live != live {
            self.live = live;
            self.forget_later(cx);
        }
        self.follow_minimap(cx);
    }

    /// Only a trainer that can place the player has a run to follow.
    fn follow_minimap(&mut self, cx: &mut Context<Self>) {
        let live = self.runs.read(cx).live_run().filter(|(_, run)| self.minimap && run.tracker.is_some());
        let run = live.and_then(|(game, run)| {
            let offers = self.library.read(cx).game(game)?.source.maps.clone();
            Some(LiveRun { offers, run: run.clone() })
        });
        self.live_map.update(cx, |live, cx| live.set_run(run, cx));
    }

    /// A closed map whose game has no live trainer lets go of its pins after a while.
    fn forget_later(&mut self, cx: &mut Context<Self>) {
        let kept = self.is_open(cx) || self.live.is_some() && self.live == self.game;
        if kept || self.board.read(cx).unloaded() {
            self._forget = Task::ready(());
            return;
        }
        self._forget = cx.spawn(async move |maps, cx| {
            cx.background_executor().timer(KEPT_CLOSED).await;
            maps.update(cx, |maps, cx| {
                // A load on its way would install into the map let go of.
                maps._load = Task::ready(());
                maps.board.update(cx, |board, _| board.forget());
            })
            .ok();
        });
    }
}
