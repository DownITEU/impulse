//! The trainer's notes over its cheats: what to read before playing, folded like a section.

use client::trainer::{NOTES, Sheet};
use gpui_kit::{
    Context, FontWeight, IntoElement, SharedString,
    component::{h_flex, text::TextView, v_flex},
    div,
    prelude::*,
    px, rgb,
};

use super::CheatList;
use crate::ui::{
    assets::Icon,
    key,
    text::t,
    theme::*,
    widgets::{icon, icon_button, tracked},
};

impl CheatList {
    pub(super) fn notes(&self, notes: &SharedString, sheet: &Sheet, cx: &Context<Self>) -> impl IntoElement {
        let folded = sheet.folded.contains(NOTES);
        let chevron = if folded { Icon::ChevronRight } else { Icon::ChevronDown };
        let text = TextView::markdown("notes", notes.clone()).on_link_click(|url, _, _, cx| {
            // Links into other apps are not followed from here.
            if url.starts_with("https://") || url.starts_with("http://") {
                cx.open_url(url);
            }
        });
        v_flex()
            .border_1()
            .border_color(line(0.09))
            .bg(alpha(BG, 0.78))
            .child(
                h_flex()
                    .h(px(36.))
                    .px(px(14.))
                    .gap(px(10.))
                    .child(icon(Icon::Info, 13., rgb(accent(cx))))
                    .child(tracked(&t(cx, key::CHEATS_NOTES), 13., 0.16).font_weight(FontWeight::BOLD))
                    .child(div().flex_1())
                    .child(
                        icon_button("fold-notes", chevron, 26., 13.).on_click(cx.listener(|list, _, _, cx| list.toggle_fold(NOTES, cx))),
                    ),
            )
            .when(!folded, |card| card.child(div().px(px(14.)).pb(px(12.)).text_size(px(13.)).text_color(rgb(TEXT_2)).child(text)))
    }
}
