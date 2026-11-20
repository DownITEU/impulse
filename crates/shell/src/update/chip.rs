//! The status bar's mark that an update is on its way or waiting.

use gpui_kit::{App, Entity, IntoElement, component::h_flex, prelude::*, px, rgb};
use i18n::Arg;

use super::{Mode, Status, Updates};
use crate::ui::{
    key,
    text::{t, tf},
    theme::*,
    widgets::{mono, square},
};

/// `None` while there is nothing to say; a click opens the offer's notes.
pub fn chip(updates: &Entity<Updates>, cx: &App) -> Option<impl IntoElement> {
    if !updates.read(cx).announces() {
        return None;
    }
    let text = match updates.read(cx).status() {
        Status::Available(version) => tf(cx, key::UPDATE_CHIP_AVAILABLE, &[("version", Arg::Str(&version.to_string()))]),
        Status::Downloading(_) => t(cx, key::UPDATE_CHIP_DOWNLOADING),
        Status::Ready(_) => t(cx, key::UPDATE_CHIP_READY),
        _ => return None,
    };
    let accent = rgb(accent(cx));
    let updates = updates.clone();
    Some(
        h_flex()
            .id("update-chip")
            .h_full()
            .gap(px(8.))
            .cursor_pointer()
            .child(square(7., accent))
            .child(mono(text, 11.).text_color(accent))
            .on_click(move |_, _, cx| updates.update(cx, |updates, cx| updates.present(Mode::Available, cx))),
    )
}
