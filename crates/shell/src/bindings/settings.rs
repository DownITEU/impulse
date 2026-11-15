//! The Settings `HOTKEYS` tab.

use gpui_kit::{Context, Entity, IntoElement, Render, SharedString, Window, component::v_flex, div, prelude::*, px};

use super::Hotkeys;
use crate::{
    settings::row,
    ui::{
        key,
        text::t,
        widgets::{choice, segmented, toggle},
    },
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
        let scope = |id, label: SharedString, value: bool| {
            choice(id, label, game_only == value, cx)
                .on_click(cx.listener(move |view, _, _, cx| view.hotkeys.update(cx, |hotkeys, cx| hotkeys.set_game_only(value, cx))))
        };
        v_flex()
            .flex_1()
            .px(px(18.))
            .py(px(6.))
            .child(
                row(t(cx, key::HOTKEYS_ENABLED), t(cx, key::HOTKEYS_ENABLED_HINT), 64.).child(
                    div()
                        .id("hotkeys-enabled")
                        .cursor_pointer()
                        .on_click(
                            cx.listener(move |view, _, _, cx| view.hotkeys.update(cx, |hotkeys, cx| hotkeys.set_enabled(!enabled, cx))),
                        )
                        .child(toggle(enabled, cx)),
                ),
            )
            .child(row(t(cx, key::HOTKEYS_WORKS_IN), t(cx, key::HOTKEYS_WORKS_IN_HINT), 64.).border_b_0().child(
                segmented().h(px(26.)).child(scope("hotkeys-global", t(cx, key::HOTKEYS_GLOBAL), false)).child(scope(
                    "hotkeys-in-game",
                    t(cx, key::HOTKEYS_IN_GAME),
                    true,
                )),
            ))
    }
}
