//! The open game's pane: the hero, the map and session panels and the cheat list.

use client::library::Game;
use gpui_kit::{
    AnyElement, App, Context, Div, IntoElement, Window,
    component::{h_flex, scroll::ScrollableElement, v_flex},
    div,
    prelude::*,
    px,
};

use super::{
    Page,
    bindings::hotkeys_scope,
    library::{SIDEBAR_WIDTH, vacant},
    map::map_panel,
    run::session_panel,
    settings::Panels,
};

// Below this pane width the side panels drop under the cheat list.
const WIDE_PANE: f32 = 900.;
const SIDE_WIDTH: f32 = 280.;
const SESSION_WIDTH: f32 = 300.;

impl Page {
    pub(super) fn pane(&self, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        let Some(game) = self.game(cx) else {
            return div().flex_1().p(px(24.)).child(vacant(self.library.read(cx))).into_any_element();
        };
        let cheats = self.cheat_list.clone().into_any_element();
        let mapped = !game.source.maps.is_empty();
        let body = div().flex().gap(px(20.)).px(px(24.)).pt(px(20.)).pb(px(28.));
        let body = match self.settings.read(cx).prefs().panels {
            // Without a map the session takes the row alone, as a strip.
            Panels::Top if !mapped => body.flex_col().child(self.session(&game, true, cx)).child(cheats),
            Panels::Top => {
                let session = self.session(&game, false, cx).w(px(SESSION_WIDTH)).flex_none();
                let map = map_panel(&self.map, &game, cx).flex_1().min_w_0();
                body.flex_col().child(h_flex().items_stretch().gap(px(16.)).child(map).child(session)).child(cheats)
            }
            Panels::Side => {
                let wide = window.viewport_size().width >= px(SIDEBAR_WIDTH + WIDE_PANE);
                let side = match wide {
                    true => v_flex().flex_none(),
                    false => h_flex().items_start(),
                };
                let session = self.session(&game, false, cx).when(!wide, |panel| panel.flex_1());
                body.when(!wide, |body| body.flex_col())
                    .when(wide, |body| body.items_start())
                    .child(cheats)
                    .child(side.gap(px(16.)).child(map_panel(&self.map, &game, cx).w(px(SIDE_WIDTH)).flex_none()).child(session))
            }
        };
        v_flex().id("pane").flex_1().min_w_0().h_full().child(self.hero.clone()).child(body).overflow_y_scrollbar().into_any_element()
    }

    pub(super) fn session(&self, game: &Game, strip: bool, cx: &App) -> Div {
        session_panel(self.runs.read(cx), self.trainers.read(cx), game, strip, hotkeys_scope(&self.hotkeys, cx), cx)
    }
}
