//! The Settings `HOTKEYS` tab.

use gpui_kit::{Context, Entity, IntoElement, Render, Window, component::v_flex, div, prelude::*, px};

use super::Hotkeys;
use crate::{
    settings::row,
    widgets::{choice, segmented, toggle},
};

pub struct HotkeySettings {
    hotkeys: Entity<Hotkeys>,
}

impl HotkeySettings {
    pub fn new(hotkeys: Entity<Hotkeys>, cx: &mut Context<Self>) -> Self {
        cx.observe(&hotkeys, |_, _, cx| cx.notify()).detach();
        Self { hotkeys }
    }
}

impl Render for HotkeySettings {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let hotkeys = self.hotkeys.read(cx);
        let (enabled, game_only) = (hotkeys.enabled(), hotkeys.game_only());
        let scope = |id, label, value: bool| {
            choice(id, label, game_only == value, cx)
                .on_click(cx.listener(move |view, _, _, cx| view.hotkeys.update(cx, |hotkeys, cx| hotkeys.set_game_only(value, cx))))
        };
        v_flex()
            .flex_1()
            .px(px(18.))
            .py(px(6.))
            .child(
                row("HOTKEYS", "trainer hotkeys during a run", 64.).child(
                    div()
                        .id("hotkeys-enabled")
                        .cursor_pointer()
                        .on_click(
                            cx.listener(move |view, _, _, cx| view.hotkeys.update(cx, |hotkeys, cx| hotkeys.set_enabled(!enabled, cx))),
                        )
                        .child(toggle(enabled, cx)),
                ),
            )
            .child(row("WORKS IN", "in game: only while the game is in front", 64.).border_b_0().child(
                segmented().h(px(26.)).child(scope("hotkeys-global", "GLOBAL", false)).child(scope("hotkeys-in-game", "IN GAME", true)),
            ))
    }
}
