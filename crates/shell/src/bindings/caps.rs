//! The hotkey caps of a cheat row: one per slot; a click binds it.

use std::sync::Arc;

use client::trainer::Sheet;
use gpui_kit::{App, Entity, IntoElement, SharedString, Window, component::h_flex, prelude::*, px, rgb};
use model::Kind;

use super::{Hotkeys, PRESS_KEYS, Target};
use crate::ui::{theme::*, widgets::keycap};

/// A slot's cap: a pair's arrow (`Kind::slots` order), then `body`.
pub fn cap(kind: Kind, slot: usize, body: &str) -> SharedString {
    let arrow = match (kind, slot) {
        (Kind::Selection, 0) => "← ",
        (Kind::Selection, _) => "→ ",
        (Kind::Number | Kind::Slider, 0) => "↓ ",
        (Kind::Number | Kind::Slider, _) => "↑ ",
        _ => "",
    };
    format!("{arrow}{body}").into()
}

/// `keys`: the slots' labels as printed.
pub fn caps(hotkeys: &Entity<Hotkeys>, game: &Arc<str>, sheet: &Sheet, index: usize, keys: &[SharedString], cx: &App) -> impl IntoElement {
    let cheat = &sheet.cheats[index];
    let bound = hotkeys.read(cx);
    let accent = rgb(accent(cx));
    h_flex().flex_initial().min_w_0().justify_end().gap(px(4.)).children(keys.iter().enumerate().map(|(slot, label)| {
        let held = bound.held_slot(game, &cheat.id, slot);
        let text = match held {
            Some(held) => cap(cheat.kind, slot, &held.map_or_else(|| PRESS_KEYS.to_owned(), |chord| chord.to_string())),
            None => label.clone(),
        };
        let unbound = held.is_none() && sheet.hotkeys[index][slot].is_none();
        let (cancel, bind) = (hotkeys.clone(), hotkeys.clone());
        let (game, id) = (game.clone(), cheat.id.clone());
        keycap(("cap", index * 2 + slot), text)
            .cursor_pointer()
            .hover(|style| style.border_color(line(0.3)))
            .when(unbound, |cap| cap.text_color(rgb(GHOST)))
            .when(held.is_some(), |cap| {
                cap.border_color(accent)
                    .text_color(accent)
                    .on_mouse_down_out(move |_, _, cx| cancel.update(cx, |hotkeys, cx| hotkeys.cancel(cx)))
            })
            .on_click(move |_, window, cx| bind_slot(&bind, game.clone(), id.clone(), slot, window, cx))
    }))
}

/// Binds a cheat's slot by what the user presses.
fn bind_slot(hotkeys: &Entity<Hotkeys>, game: Arc<str>, cheat: String, slot: usize, window: &mut Window, cx: &mut App) {
    let target = Target::Slot { game: game.clone(), cheat: cheat.clone(), slot };
    let Some(outcome) = hotkeys.update(cx, |hotkeys, cx| hotkeys.capture(target, window, cx)) else { return };
    let hotkeys = hotkeys.downgrade();
    cx.spawn(async move |cx| {
        if let Ok(outcome) = outcome.await {
            hotkeys.update(cx, |hotkeys, cx| hotkeys.rebind(&game, &cheat, slot, outcome, cx)).ok();
        }
    })
    .detach();
}
