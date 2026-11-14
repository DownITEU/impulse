//! The overlay's layout over the game: the game's title at the top left, its run's button at the top right,
//! the docks' toggles between them, and the docks with the page's own parts in them.

use gpui_kit::{
    AnyElement, App, Bounds, Context, Entity, IntoElement, KeyBinding, Pixels, Size, Window, actions,
    component::{h_flex, scroll::ScrollableElement},
    div, point,
    prelude::*,
    px, size,
};

use super::{
    Page,
    assets::Icon,
    docks::{self, Spec},
    layer::Layer,
    theme::*,
    widgets::{icon_button, notice},
};

actions!(hud, [LeaveOverlay]);

const CONTEXT: &str = "Hud";
const MODS: &str = "mods";
const MAP: &str = "map";
const SESSION: &str = "session";
const MINIMAP: &str = "minimap";
// The top bar's height, where the docks start; the screen edge's margin; the first columns' widths.
const TOP: f32 = 88.;
const EDGE: f32 = 24.;
const GAP: f32 = 16.;
const LIST: f32 = 560.;
const SIDE: f32 = 320.;

/// The toolbar's order.
pub(super) static DOCKS: [Spec; 4] = [
    Spec {
        id: MODS,
        title: "MODS",
        icon: Icon::Mods,
        first: list_first,
        smallest: Some(size(px(420.), px(240.))),
        shown: true,
        body_moves: false,
    },
    Spec {
        id: MAP,
        title: "MAP",
        icon: Icon::Map,
        first: map_first,
        smallest: Some(size(px(360.), px(280.))),
        shown: true,
        body_moves: false,
    },
    Spec { id: SESSION, title: "SESSION", icon: Icon::Clock, first: side_first, smallest: None, shown: true, body_moves: false },
    // Also over the game while Impulse is behind it.
    Spec {
        id: MINIMAP,
        title: "MINIMAP",
        icon: Icon::Player,
        first: minimap_first,
        smallest: Some(size(px(200.), px(200.))),
        shown: false,
        body_moves: true,
    },
];

pub(super) static MINIMAP_DOCK: &Spec = &DOCKS[3];

fn list_first(screen: Size<Pixels>) -> Bounds<Pixels> {
    Bounds::new(point(px(EDGE), px(TOP)), size(px(LIST), screen.height - px(TOP + EDGE)))
}

fn map_first(screen: Size<Pixels>) -> Bounds<Pixels> {
    let left = px(EDGE + LIST + GAP);
    Bounds::new(point(left, px(TOP)), size(screen.width - left - px(SIDE + GAP + EDGE), screen.height - px(TOP + EDGE)))
}

fn side_first(screen: Size<Pixels>) -> Bounds<Pixels> {
    Bounds::new(point(screen.width - px(SIDE + EDGE), px(TOP)), size(px(SIDE), px(250.)))
}

fn minimap_first(screen: Size<Pixels>) -> Bounds<Pixels> {
    Bounds::new(point(screen.width - px(SIDE + EDGE), screen.height - px(SIDE + EDGE)), size(px(SIDE), px(SIDE)))
}

pub fn bind_keys(cx: &mut App) {
    cx.bind_keys([KeyBinding::new("escape", LeaveOverlay, Some(CONTEXT))]);
}

impl Page {
    pub(super) fn hud(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let game = self.game(cx);
        let game = game.as_deref();
        let mapped = game.is_some_and(|game| !game.source.maps.is_empty());
        let running = self.is_running(cx);
        let shown = self.docks.read(cx).shown();
        let docked = shown
            .into_iter()
            .filter_map(|spec| {
                let content = match (spec.id, game) {
                    (MODS, Some(game)) => {
                        div().id(MODS).size_full().p(px(16.)).overflow_y_scrollbar().child(self.cheats(game, cx)).into_any_element()
                    }
                    (MODS, None) => div().p(px(16.)).child(self.vacant(cx)).into_any_element(),
                    (MAP, Some(_)) if mapped => self.map.clone().into_any_element(),
                    (MINIMAP, Some(_)) if mapped && self.maps.read(cx).live_map().read(cx).ready(cx) => {
                        self.minimap.clone().into_any_element()
                    }
                    (MINIMAP, Some(_)) if mapped => div()
                        .p(px(16.))
                        .child(notice(
                            "[ MINIMAP ]",
                            "NO LIVE MAP",
                            "It follows the player while a trainer that can place them runs in a game with a map.",
                        ))
                        .into_any_element(),
                    (SESSION, Some(game)) => div().p(px(16.)).child(self.session_panel(game, running, false, cx)).into_any_element(),
                    _ => return None,
                };
                Some((spec, content))
            })
            .collect();
        let frames = docks::frame(&self.docks, docked, window, cx);
        let layer = self.layer.clone();
        let bar = h_flex()
            .absolute()
            .top_0()
            .left_0()
            .w_full()
            .h(px(TOP - GAP))
            .px(px(EDGE))
            .gap(px(GAP))
            .child(h_flex().flex_1().min_w_0().when_some(game, |left, game| {
                left.child(
                    self.title(game, 24., cx).max_w_full().px(px(14.)).py(px(8.)).bg(alpha(BG, 0.85)).border_1().border_color(line(0.1)),
                )
            }))
            .child(
                h_flex()
                    .gap(px(4.))
                    .child(docks::toolbar(&self.docks, cx, |spec| !matches!(spec.id, MAP | MINIMAP) || mapped))
                    .child(div().w(px(1.)).h(px(24.)).mx(px(6.)).bg(line(0.16)))
                    .child(
                        icon_button("hud-close", Icon::Close, 40., 14.)
                            .border_1()
                            .border_color(line(0.12))
                            .bg(alpha(BG, 0.85))
                            .on_click(move |_, _, cx| layer.read(cx).leave()),
                    ),
            )
            .child(h_flex().flex_1().justify_end().when_some(game, |right, game| right.child(self.run_button(game, cx))));
        div()
            .id("hud")
            .key_context(CONTEXT)
            .track_focus(&self.hud_focus)
            .on_action(cx.listener(Self::leave_overlay))
            .relative()
            .size_full()
            .bg(alpha(0x000000, 0.4))
            .child(bar)
            .children(frames)
            .child(docks::input(&self.docks))
            .children(self.collection_editor(cx))
            .into_any_element()
    }

    /// The window may have switched between the overlay and itself, or the hotkey changed.
    pub(super) fn overlay_changed(&mut self, layer: Entity<Layer>, window: &mut Window, cx: &mut Context<Self>) {
        let over = layer.read(cx).is_over();
        if over != self.over {
            self.switched(over, window, cx);
        }
        cx.notify();
    }

    fn switched(&mut self, over: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.over = over;
        self.map.update(cx, |map, cx| map.set_docked(over, cx));
        self.sync_minimap_dock(window, cx);
        let selected = self.selected(cx);
        let live = self.runs.read(cx).live().filter(|&live| over && selected.as_ref() != Some(live)).cloned();
        if over {
            // What belongs to the window closes, and the docks show the running game.
            self.menu = None;
            self.editor = None;
            self.settings_view.update(cx, |view, cx| view.close(cx));
            self.log_open = false;
            self.hotkeys.update(cx, |hotkeys, cx| hotkeys.cancel(cx));
        }
        match live {
            // Its `Opened` syncs the map dock.
            Some(live) => self.select(live, cx),
            None => self.sync_map_dock(window, cx),
        }
        // An open map keeps the focus: its Escape closes its own things first, then leaves.
        if over && !self.maps.read(cx).is_open(cx) {
            self.hud_focus.focus(window, cx);
        }
    }

    /// The minimap follows the live run while its dock is shown.
    pub(super) fn dock_minimap(&self, cx: &mut Context<Self>) {
        let wanted = self.docks.read(cx).is_shown(MINIMAP_DOCK);
        self.maps.update(cx, |maps, cx| maps.set_minimap(wanted, cx));
    }

    /// Only a live game with a map has a minimap to toggle.
    pub(super) fn minimap_key(&mut self, cx: &mut Context<Self>) {
        let live = self.runs.read(cx).live().and_then(|live| self.library.read(cx).game(live));
        if live.is_some_and(|game| !game.source.maps.is_empty()) {
            self.docks.update(cx, |docks, cx| docks.toggle(MINIMAP_DOCK, cx));
        }
    }

    /// The dock's minimap lets go of its tiles once it is not drawn: out of the overlay (the HUD's own draws),
    /// its dock closed, or no live map.
    pub(super) fn sync_minimap_dock(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let drawn = self.over && self.docks.read(cx).is_shown(MINIMAP_DOCK) && self.maps.read(cx).live_map().read(cx).ready(cx);
        if !drawn {
            self.minimap.update(cx, |minimap, cx| minimap.hide(window, cx));
        }
    }

    /// The map is open while its dock shows over a game that has one.
    pub(super) fn sync_map_dock(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let shown = DOCKS.iter().find(|spec| spec.id == MAP).is_some_and(|spec| self.docks.read(cx).is_shown(spec));
        let docked = self.over && shown && self.game(cx).is_some_and(|game| !game.source.maps.is_empty());
        if docked != self.maps.read(cx).is_open(cx) {
            match docked {
                true => self.open_map(window, cx),
                false => {
                    self.map.update(cx, |map, cx| map.hide(window, cx));
                    // The focus must not stay in the gone map, or Escape would reach nothing.
                    if self.over {
                        self.hud_focus.focus(window, cx);
                    }
                }
            }
        }
    }

    fn leave_overlay(&mut self, _: &LeaveOverlay, window: &mut Window, cx: &mut Context<Self>) {
        // An open dialog or menu closes first.
        if self.editor.is_some() {
            return self.close_editor(window, cx);
        }
        if self.menu.take().is_some() {
            return cx.notify();
        }
        self.layer.read(cx).leave();
    }
}
