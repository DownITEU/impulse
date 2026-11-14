//! The open game's map: a toolbar over the sidebar, the stage and the card. Each is a view of its own
//! that observes the shared board, so what moves on the map repaints the map alone.

mod board;
mod card;
mod keys;
mod live;
mod maps;
mod markers;
mod panel;
mod picture;
mod sidebar;
mod stage;

use client::{library::Game, media::Media, storage::Paths};
use gpui_kit::{
    App, Context, Entity, EventEmitter, FocusHandle, IntoElement, Render, StyleRefinement, Window,
    component::{h_flex, v_flex},
    div,
    prelude::*,
    px, rgb,
};
use model::{Group, Map, MapOffer, Pin};

use crate::{
    assets::Icon,
    theme::*,
    widgets::{icon_button, mono},
};
use board::Board;
use card::Card;
pub use keys::bind_keys;
use keys::{CONTEXT, CloseMap, FindPlace};
pub use live::LiveMap;
pub use maps::Maps;
pub use panel::map_panel;
use sidebar::Sidebar;
use stage::Stage;

const CARD_WIDTH: f32 = 340.;
// Between the map's edges and what floats over it.
const MARGIN: f32 = 12.;
// The stage's buttons at the bottom right. The card ends above their tallest stack: follow, the zoom pair, layers.
const BUTTON: f32 = 34.;
const CONTROLS: f32 = BUTTON * 4. + 16.;

pub enum MapEvent {
    Close,
}

impl EventEmitter<MapEvent> for MapView {}

/// The window's parts of `Maps`.
pub struct MapView {
    maps: Entity<Maps>,
    board: Entity<Board>,
    stage: Entity<Stage>,
    sidebar: Entity<Sidebar>,
    card: Entity<Card>,
    sidebar_open: bool,
    // In a dock, whose frame closes it.
    docked: bool,
    focus: FocusHandle,
}

impl MapView {
    pub fn new(maps: Entity<Maps>, paths: &Paths, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let board = maps.read(cx).board().clone();
        cx.observe(&maps, |_, _, cx| cx.notify()).detach();
        cx.observe(&board, |_, _, cx| cx.notify()).detach();
        let stage = cx.new(|cx| Stage::new(board.clone(), Media::Tiles.dir(paths), cx));
        let sidebar = cx.new(|cx| Sidebar::new(board.clone(), window, cx));
        let card = cx.new(|cx| Card::new(board.clone(), Media::Videos.dir(paths), window, cx));
        // The camera lives with the stage.
        let camera = stage.downgrade();
        maps.update(cx, |maps, _| maps.set_camera(move |cx| camera.upgrade().and_then(|stage| stage.read(cx).view())));
        Self { maps, board, stage, sidebar, card, sidebar_open: true, docked: false, focus: cx.focus_handle() }
    }

    pub fn set_docked(&mut self, docked: bool, cx: &mut Context<Self>) {
        self.docked = docked;
        cx.notify();
    }

    /// Opens on the maps of `game`.
    pub fn show(&mut self, game: &Game, window: &mut Window, cx: &mut Context<Self>) {
        self.stage.update(cx, |stage, cx| stage.take_focus(window, cx));
        self.maps.update(cx, |maps, cx| maps.open(&game.id, &game.source.maps, cx));
    }

    /// Lets go of what only an open map needs.
    pub fn hide(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.maps.read(cx).is_open(cx) {
            return;
        }
        self.maps.update(cx, |maps, cx| maps.close(cx));
        self.stage.update(cx, |stage, _| stage.hide(window));
        self.card.update(cx, |card, _| card.hide(window));
        self.sidebar.update(cx, |sidebar, _| sidebar.hide(window));
    }

    fn toolbar(&self, cx: &Context<Self>) -> impl IntoElement {
        let accent = accent(cx);
        let maps = self.maps.read(cx);
        let chosen = maps.chosen();
        let tab = |(index, offer): (usize, &MapOffer)| {
            let chosen = index == chosen;
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
                    if view.maps.read(cx).chosen() != index {
                        view.maps.update(cx, |maps, cx| maps.choose(index, cx));
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
                    .children(maps.offers().iter().enumerate().map(tab)),
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
            .child(self.toolbar(cx))
            .child(h_flex().flex_1().min_h_0().items_start().children(sidebar).child(map))
    }
}
