//! Where hotkeys fire, as the session panel shows it; a click switches it.

use gpui_kit::{AnyElement, App, Entity, FontWeight, IntoElement, component::h_flex, prelude::*, px, rgb};

use super::Hotkeys;
use crate::ui::{
    theme::*,
    widgets::{square, tracked},
};

/// While hotkeys are off, Settings turns them on.
pub fn hotkeys_scope(hotkeys: &Entity<Hotkeys>, cx: &App) -> AnyElement {
    let (enabled, game_only) = (hotkeys.read(cx).enabled(), hotkeys.read(cx).game_only());
    let scope = match (enabled, game_only) {
        (false, _) => "OFF",
        (true, true) => "IN GAME",
        (true, false) => "GLOBAL",
    };
    let hotkeys = hotkeys.clone();
    h_flex()
        .id("hotkeys-scope")
        .gap(px(8.))
        .when(enabled, |stat| {
            stat.cursor_pointer().on_click(move |_, _, cx| hotkeys.update(cx, |hotkeys, cx| hotkeys.set_game_only(!game_only, cx)))
        })
        .child(square(7., rgb(if enabled { accent(cx) } else { GHOST })))
        .child(tracked(scope, 12., 0.12).font_weight(FontWeight::BOLD).when(!enabled, |text| text.text_color(rgb(GHOST))))
        .into_any_element()
}
