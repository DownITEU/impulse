use std::time::Duration;

use client::library::Game;
use gpui_kit::{
    Context, Div, FontWeight, IntoElement, ObjectFit, Window,
    component::{h_flex, v_flex},
    div, img,
    prelude::*,
    px, relative, rgb,
};

use super::{
    Page,
    assets::Icon,
    theme::*,
    widgets::{brackets, grid, icon_button, mono, square, tracked},
};

const MAP_HEIGHT: f32 = 204.;

impl Page {
    pub(super) fn open_map(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(game) = self.game(cx) else { return };
        let (id, offers) = (game.id.clone(), game.source.maps.clone());
        self.map.update(cx, |map, cx| map.show(&id, &offers, window, cx));
    }

    pub(super) fn close_map(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // Over a game the map is a dock, and Escape's last step leaves the overlay.
        if self.over {
            return self.layer.read(cx).leave();
        }
        self.map.update(cx, |map, cx| map.hide(window, cx));
    }

    /// The caller gives it a width.
    pub(super) fn map_panel(&self, game: &Game, cx: &mut Context<Self>) -> Div {
        let offer = game.source.maps.first();
        let open = || cx.listener(|page, _, window, cx| page.open_map(window, cx));
        v_flex()
            .relative()
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
                            .child(mono(offer.map_or("-".into(), |offer| offer.name.to_uppercase()), 10.5)),
                    )
                    .when(offer.is_some(), |header| header.child(icon_button("map", Icon::Expand, 26., 13.).on_click(open()))),
            )
            .child(match offer {
                Some(offer) => div()
                    .id("map-preview")
                    .relative()
                    .h(px(MAP_HEIGHT))
                    .overflow_hidden()
                    .cursor_pointer()
                    .on_click(open())
                    .child(grid(20., line(0.035)))
                    // Out of the flow: an image in it takes its own height at the box's width.
                    .when_some(offer.thumbnail.clone(), |preview, url| {
                        preview.child(img(url).absolute().top_0().left_0().size_full().object_fit(ObjectFit::Cover))
                    })
                    .into_any_element(),
                None => div()
                    .h(px(MAP_HEIGHT))
                    .p(px(16.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_center()
                    .text_color(rgb(FAINT))
                    .child(mono("NO INTERACTIVE MAP FOR THIS GAME", 11.))
                    .into_any_element(),
            })
            .children(brackets(cx))
    }

    /// The live run's clock, or the last run's time, over the active count and the hotkeys' scope.
    /// `strip`: all of it in one line that wraps.
    pub(super) fn session_panel(&self, game: &Game, running: bool, strip: bool, cx: &mut Context<Self>) -> Div {
        let accent = rgb(accent(cx));
        let last = self.trainers.read(cx).profile(&game.id).last;
        let (title, time) = match self.runs.read(cx).since().filter(|_| running) {
            Some(since) => ("SESSION", since.elapsed()),
            None if last > 0 => ("LAST SESSION", Duration::from_secs(last)),
            None => ("SESSION", Duration::ZERO),
        };
        let sheet = self.sheet(&game.id, cx);
        let total = sheet.map_or(game.mods as usize, |sheet| sheet.cheats.len());
        let active = match (running, sheet) {
            (true, Some(sheet)) => format!("{:02}", sheet.active()),
            _ => "-".to_owned(),
        };
        let active = h_flex()
            .gap(px(8.))
            .child(mono(active, 13.).text_color(if running { accent } else { rgb(FAINT) }))
            .child(mono(format!("/ {total}"), 13.).text_color(rgb(FAINT)));
        let clock = |size: f32| {
            mono(clock(time), size).font_weight(FontWeight::BOLD).line_height(relative(1.15)).text_color(rgb(if running {
                TEXT_BRIGHT
            } else {
                FAINT
            }))
        };
        let panel = div().flex().border_1().border_color(line(0.1)).bg(rgb(PANEL)).px(px(16.));
        if strip {
            let pair = |name: &'static str| h_flex().gap(px(12.)).child(label(name));
            return panel
                .flex_wrap()
                .items_center()
                .gap_x(px(32.))
                .gap_y(px(6.))
                .py(px(10.))
                .child(pair(title).child(clock(24.)))
                .child(div().flex_1())
                .child(pair("ACTIVE").child(active))
                .child(pair("HOTKEYS").child(self.hotkeys_scope(cx)));
        }
        // In a taller box the stats keep to its bottom.
        panel
            .flex_col()
            .justify_between()
            .pt(px(14.))
            .pb(px(6.))
            .child(v_flex().child(label(title)).child(clock(42.).mt(px(6.)).mb(px(10.))))
            .child(v_flex().child(stat("ACTIVE").child(active)).child(stat("HOTKEYS").child(self.hotkeys_scope(cx))))
    }

    /// Switches where hotkeys fire; while they are off, Settings turns them on.
    fn hotkeys_scope(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let hotkeys = self.hotkeys.read(cx);
        let (enabled, game_only) = (hotkeys.enabled(), hotkeys.game_only());
        let scope = match (enabled, game_only) {
            (false, _) => "OFF",
            (true, true) => "IN GAME",
            (true, false) => "GLOBAL",
        };
        h_flex()
            .id("hotkeys-scope")
            .gap(px(8.))
            .when(enabled, |stat| {
                stat.cursor_pointer().on_click(
                    cx.listener(move |page, _, _, cx| page.hotkeys.update(cx, |hotkeys, cx| hotkeys.set_game_only(!game_only, cx))),
                )
            })
            .child(square(7., rgb(if enabled { accent(cx) } else { GHOST })))
            .child(tracked(scope, 12., 0.12).font_weight(FontWeight::BOLD).when(!enabled, |text| text.text_color(rgb(GHOST))))
    }
}

/// `01:02:03`.
fn clock(time: Duration) -> String {
    let secs = time.as_secs();
    format!("{:02}:{:02}:{:02}", secs / 3600, secs / 60 % 60, secs % 60)
}

fn label(text: &'static str) -> Div {
    mono(text, 10.5).text_color(rgb(DIM))
}

fn stat(name: &'static str) -> Div {
    h_flex().h(px(40.)).justify_between().border_t_1().border_color(line(0.07)).child(label(name))
}
