//! The hotkey caps of a cheat row: one per slot; a click binds it.

use gpui_kit::{Context, IntoElement, component::h_flex, prelude::*, px, rgb};

use super::{
    super::{
        Page,
        loaded::{Loaded, cap},
        theme::*,
        widgets::keycap,
    },
    PRESS_KEYS,
};

impl Page {
    pub(in super::super) fn caps(&self, index: usize, loaded: &Loaded, cx: &mut Context<Self>) -> impl IntoElement {
        let cheat = &loaded.sheet.cheats[index];
        let selected = self.selected(cx);
        let game = selected.as_deref().unwrap_or_default();
        let accent = rgb(accent(cx));
        h_flex().flex_initial().min_w_0().justify_end().gap(px(4.)).children(loaded.rows[index].keys.iter().enumerate().map(
            |(slot, label)| {
                let held = self.bindings.held(game, &cheat.id, slot);
                let text = match held {
                    Some(held) => cap(cheat.kind, slot, &held.map_or_else(|| PRESS_KEYS.to_owned(), |chord| chord.to_string())),
                    None => label.clone(),
                };
                let unbound = held.is_none() && loaded.sheet.hotkeys[index][slot].is_none();
                keycap(("cap", index * 2 + slot), text)
                    .cursor_pointer()
                    .hover(|style| style.border_color(line(0.3)))
                    .when(unbound, |cap| cap.text_color(rgb(GHOST)))
                    .when(held.is_some(), |cap| {
                        cap.border_color(accent).text_color(accent).on_mouse_down_out(cx.listener(|page, _, _, cx| page.cancel_capture(cx)))
                    })
                    .on_click(cx.listener(move |page, _, window, cx| page.capture(index, slot, window, cx)))
            },
        ))
    }
}
