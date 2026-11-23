//! The hotkey caps of a cheat row: one per slot; a click binds it.

use client::{library::GameId, trainer::Sheet};
use gpui_kit::{App, Entity, IntoElement, SharedString, Window, component::h_flex, prelude::*, px, rgb};
use model::Kind;

use super::{Hotkeys, Target, popover::popover};
use crate::ui::{
    key,
    text::t,
    theme::*,
    widgets::{keycap, tip},
};

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

/// `keys`: the slots' labels as printed; `shared`: which of them another cheat's hotkey is the same as.
pub fn caps(
    hotkeys: &Entity<Hotkeys>,
    game: &GameId,
    sheet: &Sheet,
    index: usize,
    keys: &[SharedString],
    shared: &[bool],
    cx: &App,
) -> impl IntoElement {
    let cheat = &sheet.cheats[index];
    let bound = hotkeys.read(cx);
    let accent = rgb(accent(cx));
    h_flex().flex_initial().min_w_0().justify_end().gap(px(4.)).children(keys.iter().enumerate().map(|(slot, label)| {
        let held = bound.held_slot(game, &cheat.id, slot);
        let asked = bound.conflict_at(game, &cheat.id, slot).is_some();
        let text = match held {
            Some(held) => {
                cap(cheat.kind, slot, &held.map_or_else(|| t(cx, key::HOTKEYS_PRESS_KEYS).to_string(), |chord| chord.to_string()))
            }
            None => label.clone(),
        };
        let unbound = held.is_none() && sheet.hotkeys[index][slot].is_none();
        let shared = shared.get(slot).copied().unwrap_or(false);
        let (cancel, dismiss, bind) = (hotkeys.clone(), hotkeys.clone(), hotkeys.clone());
        let (owner, id) = (game.clone(), cheat.id.clone());
        let warn = alpha(WARN, 0.85);
        // The popover sits beside the cap, not in it: the cap clips what it holds.
        h_flex()
            .relative()
            .min_w_0()
            .child(
                keycap(("cap", index * 2 + slot), text)
                    .cursor_pointer()
                    .hover(move |style| style.border_color(if shared { warn } else { line(0.3) }))
                    .when(unbound, |cap| cap.text_color(rgb(GHOST)))
                    .when(shared, |cap| {
                        let text = t(cx, key::HOTKEYS_SHARED_TIP);
                        cap.border_color(warn).tooltip(move |window, cx| tip(text.clone(), window, cx))
                    })
                    .when(held.is_some(), |cap| {
                        cap.border_color(accent)
                            .text_color(accent)
                            .on_mouse_down_out(move |_, _, cx| cancel.update(cx, |hotkeys, cx| hotkeys.cancel(cx)))
                    })
                    .when(asked, |cap| {
                        // After the press is handled: an answer's button lies outside the cap and must find its question.
                        cap.border_color(accent).on_mouse_down_out(move |_, window, cx| {
                            let dismiss = dismiss.clone();
                            window.defer(cx, move |window, cx| dismiss.update(cx, |hotkeys, cx| hotkeys.dismiss(window, cx)));
                        })
                    })
                    .on_click(move |_, window, cx| bind_slot(&bind, owner.clone(), id.clone(), slot, window, cx)),
            )
            .children(popover(hotkeys, game, &cheat.id, slot, held.is_some(), cx))
    }))
}

fn bind_slot(hotkeys: &Entity<Hotkeys>, game: GameId, cheat: String, slot: usize, window: &mut Window, cx: &mut App) {
    let target = Target::Slot { game: game.clone(), cheat: cheat.clone(), slot };
    let Some(outcome) = hotkeys.update(cx, |hotkeys, cx| hotkeys.capture(target, window, cx)) else { return };
    let (hotkeys, window) = (hotkeys.downgrade(), window.window_handle());
    cx.spawn(async move |cx| {
        let Ok(outcome) = outcome.await else { return };
        // The outcome comes after the press ended: the window is asked again, where the question takes the focus.
        window.update(cx, |_, window, cx| hotkeys.update(cx, |hotkeys, cx| hotkeys.rebind(&game, &cheat, slot, outcome, window, cx))).ok();
    })
    .detach();
}
