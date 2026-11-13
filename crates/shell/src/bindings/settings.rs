//! The Settings `HOTKEYS` tab.

use gpui_kit::{Context, IntoElement, component::v_flex, div, prelude::*, px};

use super::super::{
    Page,
    settings::row,
    widgets::{choice, segmented, toggle},
};

impl Page {
    pub(in super::super) fn hotkey_settings(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let (enabled, game_only) = (self.bindings.enabled(), self.bindings.game_only());
        let scope = |id, label, value: bool| {
            choice(id, label, game_only == value, self.accent)
                .on_click(cx.listener(move |page, _, window, cx| page.set_game_only(value, window, cx)))
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
                        .on_click(cx.listener(move |page, _, window, cx| page.set_hotkeys_enabled(!enabled, window, cx)))
                        .child(toggle(enabled, self.accent)),
                ),
            )
            .child(row("WORKS IN", "in game: only while the game is in front", 64.).border_b_0().child(
                segmented().h(px(26.)).child(scope("hotkeys-global", "GLOBAL", false)).child(scope("hotkeys-in-game", "IN GAME", true)),
            ))
    }
}
