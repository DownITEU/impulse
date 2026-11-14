//! The hotkey caps of a cheat row: one per slot; a click binds it.

use client::trainer::Sheet;
use gpui_kit::{Context, IntoElement, Window, component::h_flex, prelude::*, px, rgb};

use super::{
    super::{
        Page,
        theme::*,
        trainer::{Printed, cap},
        widgets::keycap,
    },
    PRESS_KEYS, Target,
};

impl Page {
    pub(in super::super) fn caps(&self, index: usize, sheet: &Sheet, printed: &Printed, cx: &Context<Self>) -> impl IntoElement {
        let cheat = &sheet.cheats[index];
        let selected = self.selected(cx);
        let game = selected.as_deref().unwrap_or_default();
        let hotkeys = self.hotkeys.read(cx);
        let accent = rgb(accent(cx));
        h_flex().flex_initial().min_w_0().justify_end().gap(px(4.)).children(printed.rows[index].keys.iter().enumerate().map(
            |(slot, label)| {
                let held = hotkeys.held_slot(game, &cheat.id, slot);
                let text = match held {
                    Some(held) => cap(cheat.kind, slot, &held.map_or_else(|| PRESS_KEYS.to_owned(), |chord| chord.to_string())),
                    None => label.clone(),
                };
                let unbound = held.is_none() && sheet.hotkeys[index][slot].is_none();
                keycap(("cap", index * 2 + slot), text)
                    .cursor_pointer()
                    .hover(|style| style.border_color(line(0.3)))
                    .when(unbound, |cap| cap.text_color(rgb(GHOST)))
                    .when(held.is_some(), |cap| {
                        cap.border_color(accent)
                            .text_color(accent)
                            .on_mouse_down_out(cx.listener(|page, _, _, cx| page.hotkeys.update(cx, |hotkeys, cx| hotkeys.cancel(cx))))
                    })
                    .on_click(cx.listener(move |page, _, window, cx| page.bind_slot(index, slot, window, cx)))
            },
        ))
    }

    /// Binds the open trainer's slot by what the user presses.
    fn bind_slot(&mut self, index: usize, slot: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(game) = self.selected(cx) else { return };
        let Some(cheat) = self.sheet(&game, cx).map(|sheet| sheet.cheats[index].id.clone()) else { return };
        let target = Target::Slot { game: game.clone(), cheat: cheat.clone(), slot };
        let Some(outcome) = self.hotkeys.update(cx, |hotkeys, cx| hotkeys.capture(target, window, cx)) else { return };
        let hotkeys = self.hotkeys.downgrade();
        cx.spawn(async move |_, cx| {
            if let Ok(outcome) = outcome.await {
                hotkeys.update(cx, |hotkeys, cx| hotkeys.rebind(&game, &cheat, slot, outcome, cx)).ok();
            }
        })
        .detach();
    }
}
