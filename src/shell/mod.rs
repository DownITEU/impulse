//! The combined page (`docs/design.md`). It renders mock data until the catalog,
//! library and trainer are wired in.

mod art;
mod assets;
mod boot;
mod cheats;
mod controls;
mod header;
mod hero;
mod library;
mod mock;
mod settings;
mod side;
mod sidebar;
mod theme;
mod widgets;

use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use gpui_kit::{
    App, Context, Entity, IntoElement, Render, Window,
    component::{
        h_flex,
        input::{InputEvent, InputState},
        scroll::ScrollableElement,
        v_flex,
    },
    div,
    prelude::*,
    px, rgb,
};

pub use assets::Assets;
pub use boot::run as boot;
use mock::GAMES;
use theme::*;
use widgets::{grid, mono, square};

const SIDEBAR_WIDTH: f32 = 248.;
// Below this pane width the map and session panels drop under the cheat list.
const WIDE_PANE: f32 = 900.;
const AMOUNT_DIGITS: usize = 6;

pub fn init(cx: &mut App) {
    theme::init(cx);
    cx.set_http_client(Arc::new(art::CdnClient));
}

#[derive(Clone, Copy, PartialEq)]
enum Lead {
    Number,
    Thumbnail,
}

pub struct Page {
    accent: u32,
    lead: Lead,
    reduce_motion: bool,
    settings: bool,
    selected: usize,
    running: Option<usize>,
    favorites: HashSet<usize>,
    enabled: HashSet<&'static str>,
    pinned: HashSet<&'static str>,
    // Folded sections, per game.
    folded: HashSet<(usize, &'static str)>,
    crit: i32,
    levels: HashMap<&'static str, u8>,
    choices: HashMap<&'static str, &'static str>,
    overlay: &'static str,
    // The open dropdown: "overlay" or a mod id.
    menu: Option<&'static str>,
    amount: Entity<InputState>,
    game_filter: Entity<InputState>,
    mod_filter: Entity<InputState>,
}

impl Page {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let mut filter = |placeholder: &'static str| {
            let state = cx.new(|cx| InputState::new(window, cx).placeholder(placeholder));
            cx.subscribe(&state, |_, _, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    cx.notify();
                }
            })
            .detach();
            state
        };
        Self {
            game_filter: filter("filter"),
            mod_filter: filter("filter mods"),
            accent: ACCENTS[0].1,
            lead: Lead::Number,
            reduce_motion: false,
            settings: false,
            selected: 0,
            running: Some(0),
            favorites: HashSet::from([0]),
            enabled: HashSet::from(["hp", "st", "dur", "craft"]),
            pinned: HashSet::from(["hp", "st"]),
            folded: HashSet::from([(0, "COMBAT"), (0, "TELEPORT")]),
            crit: 0,
            levels: HashMap::new(),
            choices: HashMap::new(),
            overlay: hero::OVERLAYS[0],
            menu: None,
            amount: cx.new(|cx| {
                InputState::new(window, cx)
                    .validate(|text, _| {
                        text.len() <= AMOUNT_DIGITS && text.bytes().all(|b| b.is_ascii_digit())
                    })
                    .default_value("999")
            }),
        }
    }

    fn pane(&self, window: &Window, cx: &mut Context<Self>) -> impl IntoElement {
        let game = &GAMES[self.selected];
        let running = self.running == Some(self.selected);
        let wide = window.viewport_size().width >= px(SIDEBAR_WIDTH + WIDE_PANE);
        let side = match wide {
            true => v_flex().flex_none(),
            false => h_flex().items_start(),
        };
        v_flex()
            .id("pane")
            .flex_1()
            .min_w_0()
            .h_full()
            .child(self.hero(game, cx))
            .child(
                div()
                    .flex()
                    .when(!wide, |body| body.flex_col())
                    .when(wide, |body| body.items_start())
                    .gap(px(20.))
                    .px(px(24.))
                    .pt(px(20.))
                    .pb(px(28.))
                    .child(self.cheats(game, cx))
                    .child(
                        side.gap(px(16.))
                            .child(self.map_panel(game, running))
                            .child(
                                div()
                                    .when(!wide, |panel| panel.flex_1())
                                    .child(self.session_panel(game, running)),
                            ),
                    ),
            )
            .overflow_y_scrollbar()
    }

    fn status_bar(&self) -> impl IntoElement {
        let accent = rgb(self.accent);
        let total = self.running.map_or(0, |index| GAMES[index].mods);
        h_flex()
            .h(px(30.))
            .flex_none()
            .px(px(20.))
            .gap(px(22.))
            .border_t_1()
            .border_color(line(0.09))
            .bg(rgb(STATUSBAR))
            .text_color(rgb(DIM))
            .map(|bar| match self.running {
                Some(_) => bar
                    .child(
                        h_flex()
                            .gap(px(8.))
                            .child(square(7., accent))
                            .child(mono("ATTACHED", 11.).text_color(accent)),
                    )
                    .child(mono("enshrouded.exe", 11.))
                    .child(mono(format!("{}/{total} ACTIVE", self.active()), 11.)),
                None => bar.child(mono("IDLE", 11.)),
            })
            .child(div().flex_1())
            .child(mono("HOTKEYS ON", 11.))
            .child(mono("IMPULSE 0.1", 11.).text_color(rgb(GHOST)))
    }
}

impl Render for Page {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .relative()
            .size_full()
            .bg(rgb(BG))
            .text_color(rgb(TEXT))
            .font_family(UI)
            .text_size(px(14.))
            .child(grid(32., line(0.025)))
            .child(self.header(window, cx))
            .child(
                h_flex()
                    .flex_1()
                    .min_h_0()
                    .items_start()
                    .child(self.sidebar(cx))
                    .child(self.pane(window, cx)),
            )
            .child(self.status_bar())
            .when(self.settings, |page| page.child(self.settings_modal(cx)))
    }
}
