//! The open game's pane: the hero, the map and session panels and the cheat list.

use gpui_kit::{
    AnyElement, Context, IntoElement, Window,
    component::{h_flex, scroll::ScrollableElement, v_flex},
    div,
    prelude::*,
    px,
};

use super::{Load, Page, SIDEBAR_WIDTH, prefs::Panels, widgets::notice};

// Below this pane width the side panels drop under the cheat list.
const WIDE_PANE: f32 = 900.;
const SIDE_WIDTH: f32 = 280.;
const SESSION_WIDTH: f32 = 300.;

impl Page {
    pub(super) fn pane(&self, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        let Some(game) = self.game() else {
            return div().flex_1().p(px(24.)).child(self.vacant()).into_any_element();
        };
        let cheats = self.cheats(game, cx).into_any_element();
        let (running, mapped) = (self.is_running(), !game.source.maps.is_empty());
        let body = div().flex().gap(px(20.)).px(px(24.)).pt(px(20.)).pb(px(28.));
        let body = match self.panels {
            // Without a map the session takes the row alone, as a strip.
            Panels::Top if !mapped => body.flex_col().child(self.session_panel(game, running, true, cx)).child(cheats),
            Panels::Top => {
                let session = self.session_panel(game, running, false, cx).w(px(SESSION_WIDTH)).flex_none();
                let map = self.map_panel(game, cx).flex_1().min_w_0();
                body.flex_col().child(h_flex().items_stretch().gap(px(16.)).child(map).child(session)).child(cheats)
            }
            Panels::Side => {
                let wide = window.viewport_size().width >= px(SIDEBAR_WIDTH + WIDE_PANE);
                let side = match wide {
                    true => v_flex().flex_none(),
                    false => h_flex().items_start(),
                };
                let session = self.session_panel(game, running, false, cx).when(!wide, |panel| panel.flex_1());
                body.when(!wide, |body| body.flex_col())
                    .when(wide, |body| body.items_start())
                    .child(cheats)
                    .child(side.gap(px(16.)).child(self.map_panel(game, cx).w(px(SIDE_WIDTH)).flex_none()).child(session))
            }
        };
        v_flex().id("pane").flex_1().min_w_0().h_full().child(self.hero(game, cx)).child(body).overflow_y_scrollbar().into_any_element()
    }

    // The pane before there is a game to open.
    pub(super) fn vacant(&self) -> impl IntoElement {
        match &self.library {
            Load::Pending => notice("[ SCANNING ]", "LOOKING FOR INSTALLED GAMES", "Reading the game catalog and your launchers."),
            Load::Failed(why) => notice(
                "[ NO CATALOG ]",
                "THE GAME CATALOG IS UNAVAILABLE",
                &format!("It could not be downloaded: {why}. Impulse tries again on the next start."),
            ),
            Load::Ready(_) => notice(
                "[ NO GAMES ]",
                "NO INSTALLED GAMES FOUND",
                "Impulse looks in Steam, Epic, GOG, Ubisoft, Xbox, Rockstar and EA for games it has trainers for.",
            ),
        }
    }
}
