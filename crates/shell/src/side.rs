use gpui_kit::{
    Context, Div, FontWeight, IntoElement,
    component::{h_flex, v_flex},
    div,
    prelude::*,
    px, relative, rgb,
};

use super::{
    Page,
    assets::Icon,
    library::Game,
    theme::*,
    widgets::{grid, icon, icon_button, mono, square, tracked},
};

const MAP_WIDTH: f32 = 280.;
// The preview keeps the mockup's 300:220 ratio inside the 1 px borders.
const MAP_HEIGHT: f32 = (MAP_WIDTH - 2.) * 220. / 300.;

impl Page {
    pub(super) fn map_panel(&self, game: &Game, running: bool) -> impl IntoElement {
        let accent = rgb(self.accent);
        let bracket = || div().absolute().size(px(10.)).border_color(accent);
        let layer = |glyph: Icon, color| {
            icon(glyph, 0., color)
                .absolute()
                .top_0()
                .left_0()
                .size_full()
        };
        v_flex()
            .relative()
            .w(px(MAP_WIDTH))
            .flex_none()
            .border_1()
            .border_color(line(0.1))
            .bg(rgb(PANEL))
            .child(
                h_flex()
                    .h(px(34.))
                    .pl(px(12.))
                    .pr(px(6.))
                    .justify_between()
                    .border_b_1()
                    .border_color(line(0.07))
                    .child(
                        h_flex()
                            .gap(px(6.))
                            .text_color(rgb(MUTED))
                            .child(mono("MAP", 10.5))
                            .child(mono("/", 10.5).text_color(rgb(GHOST)))
                            .child(mono(game.map.clone().unwrap_or("-".into()), 10.5)),
                    )
                    .child(icon_button("map", Icon::Expand, 26., 13.)),
            )
            .child(match &game.map {
                Some(_) => div()
                    .relative()
                    .h(px(MAP_HEIGHT))
                    .child(grid(20., line(0.035)))
                    .child(layer(Icon::MapTerrain, rgb(TEXT)))
                    .child(layer(Icon::MapPins, accent))
                    .when(running, |map| {
                        map.child(
                            icon(Icon::Player, 34., accent)
                                .absolute()
                                .left(relative(0.32))
                                .top(relative(0.69))
                                .ml(px(-17.))
                                .mt(px(-17.)),
                        )
                        .child(
                            mono("X 1204 · Y 388", 10.5)
                                .absolute()
                                .left(px(10.))
                                .bottom(px(10.))
                                .px(px(7.))
                                .py(px(3.))
                                .border_1()
                                .border_color(alpha(self.accent, 0.4))
                                .bg(alpha(BG, 0.85))
                                .text_color(accent),
                        )
                    }),
                None => div()
                    .h(px(MAP_HEIGHT))
                    .p(px(16.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_center()
                    .text_color(rgb(FAINT))
                    .child(mono("NO INTERACTIVE MAP FOR THIS GAME", 11.)),
            })
            .child(
                bracket()
                    .left(px(-1.))
                    .top(px(-1.))
                    .border_l_2()
                    .border_t_2(),
            )
            .child(
                bracket()
                    .right(px(-1.))
                    .top(px(-1.))
                    .border_r_2()
                    .border_t_2(),
            )
            .child(
                bracket()
                    .left(px(-1.))
                    .bottom(px(-1.))
                    .border_l_2()
                    .border_b_2(),
            )
            .child(
                bracket()
                    .right(px(-1.))
                    .bottom(px(-1.))
                    .border_r_2()
                    .border_b_2(),
            )
    }

    pub(super) fn session_panel(&self, game: &Game, running: bool, cx: &mut Context<Self>) -> impl IntoElement {
        let accent = rgb(self.accent);
        let loaded = self.trainer(Some(&game.id));
        let total = loaded.map_or(game.mods as usize, |loaded| loaded.cheats.len());
        let active = match (running, loaded) {
            (true, Some(loaded)) => format!("{:02}", loaded.active()),
            _ => "-".to_owned(),
        };
        v_flex()
            .border_1()
            .border_color(line(0.1))
            .bg(rgb(PANEL))
            .px(px(16.))
            .pt(px(14.))
            .pb(px(6.))
            .child(label("SESSION"))
            .child(
                // No clock until a real trainer session exists.
                mono("00:00:00", 42.)
                    .font_weight(FontWeight::BOLD)
                    .line_height(relative(1.15))
                    .mt(px(6.))
                    .mb(px(10.))
                    .text_color(rgb(if running { TEXT_BRIGHT } else { FAINT })),
            )
            .child(
                stat("ACTIVE").child(
                    h_flex()
                        .gap(px(8.))
                        .child(mono(active, 13.).text_color(if running {
                            accent
                        } else {
                            rgb(FAINT)
                        }))
                        .child(mono(format!("/ {total}"), 13.).text_color(rgb(FAINT))),
                ),
            )
            .child(stat("HOTKEYS").child(self.hotkeys_scope(cx)))
    }

    /// Switches where hotkeys fire; while they are off, Settings turns them on.
    fn hotkeys_scope(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let (enabled, game_only) = (self.bindings.enabled(), self.bindings.game_only());
        let scope = match (enabled, game_only) {
            (false, _) => "OFF",
            (true, true) => "IN GAME",
            (true, false) => "GLOBAL",
        };
        h_flex()
            .id("hotkeys-scope")
            .gap(px(8.))
            .when(enabled, |stat| {
                stat.cursor_pointer()
                    .on_click(cx.listener(move |page, _, window, cx| page.set_game_only(!game_only, window, cx)))
            })
            .child(square(7., rgb(if enabled { self.accent } else { GHOST })))
            .child(
                tracked(scope, 12., 0.12)
                    .font_weight(FontWeight::BOLD)
                    .when(!enabled, |text| text.text_color(rgb(GHOST))),
            )
    }
}

fn label(text: &'static str) -> Div {
    mono(text, 10.5).text_color(rgb(DIM))
}

fn stat(name: &'static str) -> Div {
    h_flex()
        .h(px(40.))
        .justify_between()
        .border_t_1()
        .border_color(line(0.07))
        .child(label(name))
}
