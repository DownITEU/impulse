//! The open game's map: a toolbar over the sidebar, the stage and the card. Each is a view of its own
//! that observes the shared board, so what moves on the map repaints the map alone.

mod board;
mod card;
mod heading;
mod keys;
mod live;
mod markers;
mod picture;
mod sidebar;
mod stage;
mod state;
mod tracker;

use std::{path::Path, rc::Rc, sync::Arc, time::Duration};

use futures::StreamExt;
use gpui_kit::{
    App, Context, Entity, EventEmitter, FocusHandle, IntoElement, Render, StyleRefinement, Task, Window,
    component::{h_flex, v_flex},
    div,
    prelude::*,
    px, rgb,
};
use model::{Group, Map, MapOffer, Pin, Provider, Snapshot};

use crate::{
    Load, art,
    assets::Icon,
    theme::*,
    widgets::{icon_button, mono},
};
use board::Board;
pub use board::Run;
use card::Card;
pub use keys::bind_keys;
use keys::{CONTEXT, CloseMap, FindPlace};
pub use live::{LiveMap, LiveRun};
use sidebar::Sidebar;
use stage::Stage;
/// The user's progress on each map.
pub use state::dir as progress_dir;
pub use tracker::Tracker;

const CARD_WIDTH: f32 = 340.;
// Between the map's edges and what floats over it.
const MARGIN: f32 = 12.;
// The stage's buttons at the bottom right. The card ends above their tallest stack: follow, the zoom pair, layers.
const BUTTON: f32 = 34.;
const CONTROLS: f32 = BUTTON * 4. + 16.;
// A closed map keeps its pins this long, so opening it again soon is instant; later they are read from disk again.
// While a trainer is live in its game, it keeps them until the run ends.
const KEPT_CLOSED: Duration = Duration::from_secs(3 * 60);

pub enum MapEvent {
    Close,
}

impl EventEmitter<MapEvent> for MapView {}

pub struct MapView {
    provider: Arc<dyn Provider>,
    board: Entity<Board>,
    stage: Entity<Stage>,
    sidebar: Entity<Sidebar>,
    card: Entity<Card>,
    // The game whose maps are listed, and the one a trainer is live in.
    game: Option<Arc<str>>,
    live: Option<Arc<str>>,
    offers: Vec<MapOffer>,
    chosen: usize,
    sidebar_open: bool,
    // In a dock, whose frame closes it.
    docked: bool,
    live_map: Entity<LiveMap>,
    focus: FocusHandle,
    _load: Task<()>,
    _forget: Task<()>,
}

impl MapView {
    pub fn new(provider: Arc<dyn Provider>, data_dir: &Path, accent: u32, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let board = cx.new(|_| Board::new(state::dir(data_dir), accent));
        cx.observe(&board, |_, _, cx| cx.notify()).detach();
        let live_board = cx.new(|_| Board::live(state::dir(data_dir), accent));
        let tile_dir = art::Media::Tiles.dir(data_dir);
        let live_map = cx.new(|cx| LiveMap::new(provider.clone(), live_board, board.clone(), tile_dir.clone(), data_dir, cx));
        let stage = cx.new(|cx| Stage::new(board.clone(), tile_dir, cx));
        let sidebar = cx.new(|cx| Sidebar::new(board.clone(), window, cx));
        let card = cx.new(|cx| Card::new(board.clone(), art::Media::Videos.dir(data_dir), window, cx));
        Self {
            provider,
            board,
            stage,
            sidebar,
            card,
            game: None,
            live: None,
            offers: Vec::new(),
            chosen: 0,
            sidebar_open: true,
            docked: false,
            live_map,
            focus: cx.focus_handle(),
            _load: Task::ready(()),
            _forget: Task::ready(()),
        }
    }

    pub fn is_open(&self, cx: &App) -> bool {
        self.board.read(cx).is_open()
    }

    pub fn set_accent(&mut self, accent: u32, cx: &mut Context<Self>) {
        self.board.update(cx, |board, cx| board.set_accent(accent, cx));
        self.live_map.update(cx, |live, cx| live.set_accent(accent, cx));
    }

    pub fn live_map(&self) -> &Entity<LiveMap> {
        &self.live_map
    }

    /// The live run the minimap follows, while it is wanted.
    pub fn set_minimap(&mut self, run: Option<LiveRun>, cx: &mut Context<Self>) {
        self.live_map.update(cx, |live, cx| live.set_run(run, cx));
    }

    pub fn set_docked(&mut self, docked: bool, cx: &mut Context<Self>) {
        self.docked = docked;
        cx.notify();
    }

    pub fn set_run(&mut self, run: Option<Run>, cx: &mut Context<Self>) {
        self.board.update(cx, |board, cx| board.set_run(run, cx));
    }

    /// The game a trainer is live in, open map or not.
    pub fn set_live(&mut self, game: Option<Arc<str>>, cx: &mut Context<Self>) {
        if self.live != game {
            self.live = game;
            self.forget_later(cx);
        }
    }

    /// Opens on `offers`, the maps of `game`. The same game keeps its map and the view of it.
    pub fn show(&mut self, game: &Arc<str>, offers: &[MapOffer], window: &mut Window, cx: &mut Context<Self>) {
        self._forget = Task::ready(());
        self.stage.update(cx, |stage, cx| stage.take_focus(window, cx));
        let listed = self.game.as_ref() == Some(game) && self.offers == offers;
        if !listed {
            (self.game, self.offers) = (Some(game.clone()), offers.to_vec());
        }
        if !listed || self.board.read(cx).unloaded() {
            self.choose(if listed { self.chosen } else { 0 }, cx);
        }
        self.board.update(cx, |board, cx| board.set_open(true, cx));
        cx.notify();
    }

    /// Lets go of what only an open map needs.
    pub fn hide(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.is_open(cx) {
            return;
        }
        self.keep_view(cx);
        self.board.update(cx, |board, cx| {
            board.save();
            board.set_open(false, cx);
        });
        self.stage.update(cx, |stage, _| stage.hide(window));
        self.card.update(cx, |card, _| card.hide(window));
        self.sidebar.update(cx, |sidebar, _| sidebar.hide(window));
        self.forget_later(cx);
        cx.notify();
    }

    /// A closed map whose game has no live trainer lets go of its pins after a while.
    fn forget_later(&mut self, cx: &mut Context<Self>) {
        let kept = self.is_open(cx) || self.live.is_some() && self.live == self.game;
        if kept || self.board.read(cx).unloaded() {
            self._forget = Task::ready(());
            return;
        }
        self._forget = cx.spawn(async move |view, cx| {
            cx.background_executor().timer(KEPT_CLOSED).await;
            view.update(cx, |view, cx| {
                // A load on its way would install into the map let go of.
                view._load = Task::ready(());
                view.board.update(cx, |board, _| board.forget());
            })
            .ok();
        });
    }

    // The camera lives with the stage; the board keeps it with the rest of the map's state.
    fn keep_view(&mut self, cx: &mut Context<Self>) {
        let view = self.stage.read(cx).view();
        self.board.update(cx, |board, _| board.saved_mut().view = view);
    }

    fn choose(&mut self, index: usize, cx: &mut Context<Self>) {
        // The board saves the map it lets go of.
        self.keep_view(cx);
        self.chosen = index;
        let Some(offer) = self.offers.get(index) else { return };
        let shared = self.live_map.read(cx).board.read(cx).loaded(&offer.id);
        self.board.update(cx, |board, cx| board.pending(offer.id.clone(), cx));
        // The minimap's copy, when it shows this map.
        if let Some(map) = shared {
            self._load = Task::ready(());
            return self.board.update(cx, |board, cx| board.install(map, cx));
        }
        let mut snapshots = self.provider.map(offer);
        let board = self.board.downgrade();
        self._load = cx.spawn(async move |_, cx| {
            while let Some(snapshot) = snapshots.next().await {
                let shown = board.update(cx, |board, cx| match snapshot {
                    // The minimap shared its copy meanwhile.
                    Snapshot::Cached(Some(_)) if matches!(board.map, Load::Ready(_)) => {}
                    Snapshot::Cached(Some(map)) | Snapshot::Fresh(Ok(map)) => board.install(Rc::new(map), cx),
                    // The network is next.
                    Snapshot::Cached(None) => {}
                    Snapshot::Fresh(Err(e)) => board.fail(format!("{e:#}"), cx),
                });
                if shown.is_err() {
                    break;
                }
            }
        });
    }

    fn toolbar(&self, accent: u32, cx: &Context<Self>) -> impl IntoElement {
        let tab = |(index, offer): (usize, &MapOffer)| {
            let chosen = index == self.chosen;
            div()
                .id(("map-tab", index))
                .h(px(26.))
                .px(px(10.))
                .flex()
                .flex_none()
                .items_center()
                .cursor_pointer()
                .border_1()
                .border_color(if chosen { rgb(accent) } else { line(0.16) })
                .text_color(rgb(if chosen { TEXT_BRIGHT } else { DIM }))
                .child(mono(offer.name.to_uppercase(), 11.))
                .on_click(cx.listener(move |view, _, _, cx| {
                    if view.chosen != index {
                        view.choose(index, cx);
                    }
                }))
        };
        h_flex()
            .h(px(44.))
            .flex_none()
            .px(px(6.))
            .gap(px(10.))
            .border_b_1()
            .border_color(line(0.09))
            .bg(rgb(PANEL))
            .child(icon_button("map-sidebar", Icon::Sidebar, 32., 14.).on_click(cx.listener(|view, _, window, cx| {
                view.sidebar_open ^= true;
                // A focused search leaving the tree would take the map's keys with it.
                if !view.sidebar_open && view.sidebar.read(cx).has_focus(window, cx) {
                    view.stage.update(cx, |stage, cx| stage.take_focus(window, cx));
                }
                cx.notify();
            })))
            .child(mono("MAP", 10.5).text_color(rgb(DIM)))
            .child(
                h_flex()
                    .id("map-tabs")
                    .flex_1()
                    .min_w_0()
                    .gap(px(6.))
                    .overflow_x_scroll()
                    .children(self.offers.iter().enumerate().map(tab)),
            )
            .when(!self.docked, |bar| {
                bar.child(icon_button("map-close", Icon::Close, 32., 12.).on_click(cx.listener(|_, _, _, cx| cx.emit(MapEvent::Close))))
            })
    }

    /// Escape: the innermost open thing closes first, the map last.
    fn back(&mut self, _: &CloseMap, _: &mut Window, cx: &mut Context<Self>) {
        let closed = self.card.update(cx, |card, cx| card.close_top(cx))
            || self.stage.update(cx, |stage, cx| stage.close_top(cx))
            || self.board.update(cx, |board, cx| board.deselect(cx));
        if !closed {
            cx.emit(MapEvent::Close);
        }
    }

    fn find_place(&mut self, _: &FindPlace, window: &mut Window, cx: &mut Context<Self>) {
        if !self.sidebar_open {
            self.sidebar_open = true;
            cx.notify();
        }
        self.sidebar.update(cx, |sidebar, cx| sidebar.focus_search(window, cx));
    }
}

/// The live map following the player, small: in the HUD over the game and in the overlay's dock. Each
/// window keeps its own tiles and images.
pub struct Minimap {
    stage: Entity<Stage>,
}

impl Minimap {
    pub fn new(live: Entity<LiveMap>, cx: &mut Context<Self>) -> Self {
        Self { stage: cx.new(|cx| Stage::mini(live, cx)) }
    }

    /// Lets go of its tiles and images; `window` is the one it was drawn in.
    pub fn hide(&self, window: &mut Window, cx: &mut App) {
        self.stage.update(cx, |stage, _| stage.hide(window));
    }
}

impl Render for Minimap {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        self.stage.clone()
    }
}

fn group<'a>(map: &'a Map, pin: &Pin) -> &'a Group {
    &map.categories[pin.group.0].groups[pin.group.1]
}

/// "CATEGORY / GROUP" under a pin's name.
fn kind(map: &Map, pin: &Pin) -> String {
    format!("{} / {}", map.categories[pin.group.0].name, group(map, pin).name).to_uppercase()
}

impl Render for MapView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let accent = self.board.read(cx).accent;
        let sidebar =
            self.sidebar_open.then(|| self.sidebar.clone().cached(StyleRefinement::default().w(px(sidebar::WIDTH)).h_full().flex_none()));
        let map = div()
            .relative()
            .flex_1()
            .min_w_0()
            .h_full()
            .child(self.stage.clone().cached(StyleRefinement::default().size_full()))
            .child(self.card.clone().cached(StyleRefinement::default().absolute().top_0().left_0().size_full()));
        v_flex()
            .size_full()
            .bg(rgb(BG))
            .track_focus(&self.focus)
            .key_context(CONTEXT)
            .on_action(cx.listener(Self::back))
            .on_action(cx.listener(Self::find_place))
            .child(self.toolbar(accent, cx))
            .child(h_flex().flex_1().min_h_0().items_start().children(sidebar).child(map))
    }
}
