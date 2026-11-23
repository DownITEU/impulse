//! What stands over a slot's cap while it is bound: the keys that end the binding, or the chord another hotkey holds.

use client::{
    bindings::{Clash, Resolution},
    library::GameId,
};
use gpui_kit::{
    AnyElement, App, Div, Entity, KeyBinding, MouseButton, Stateful, actions,
    component::{h_flex, v_flex},
    deferred,
    prelude::*,
    px, relative, rgb,
};
use i18n::{Arg, Key};

use super::{Hotkeys, clash::Conflict};
use crate::ui::{
    key,
    text::{t, tf},
    theme::*,
    widgets::{button, mono},
};

actions!(hotkeys, [DismissConflict]);

const CONTEXT: &str = "Conflict";
/// A definite width: text that wraps inside an absolute box with no width is measured against its cap and breaks per letter.
const QUESTION_WIDTH: f32 = 300.;

pub fn bind_keys(cx: &mut App) {
    cx.bind_keys([KeyBinding::new("escape", DismissConflict, Some(CONTEXT))]);
}

/// Over the cap of a slot: the question about the chord just pressed, or while `binding` the keys that end it.
pub fn popover(hotkeys: &Entity<Hotkeys>, game: &GameId, cheat: &str, slot: usize, binding: bool, cx: &App) -> Option<AnyElement> {
    let frame = v_flex().id(("popover", slot)).occlude().absolute().bottom(relative(1.)).right_0().mb(px(6.)).px(px(10.)).py(px(8.));
    let frame = frame.gap(px(8.)).border_1().border_color(line(0.2)).bg(rgb(PANEL));
    let popover = match hotkeys.read(cx).conflict_at(game, cheat, slot) {
        Some(asked) => question(frame, hotkeys, asked, cx).into_any_element(),
        None if binding => {
            frame.child(mono(t(cx, key::HOTKEYS_BIND_HINT), TYPE_CAPTION).text_color(rgb(MUTED)).whitespace_nowrap()).into_any_element()
        }
        None => return None,
    };
    Some(deferred(popover).into_any_element())
}

/// The chord and who holds it over what may be done: the overlay's key cannot be taken, a slot of the cheat itself cannot be kept.
fn question(frame: Stateful<Div>, hotkeys: &Entity<Hotkeys>, asked: &Conflict, cx: &App) -> Stateful<Div> {
    let keys = Arg::Str(&asked.keys);
    let (text, keep, rebind) = match &asked.clash {
        Clash::Reserved => (tf(cx, key::HOTKEYS_CLASH_OVERLAY, &[("keys", keys)]), false, false),
        Clash::Slots { keep, .. } => (tf(cx, key::HOTKEYS_CLASH, &[("keys", keys), ("name", Arg::Str(&asked.names))]), *keep, true),
    };
    let dismiss = hotkeys.clone();
    let answers = h_flex()
        .gap(px(8.))
        .justify_end()
        .when(keep, |row| row.child(answer("keep", key::HOTKEYS_KEEP, Resolution::Keep, hotkeys, cx)))
        .when(rebind, |row| row.child(answer("rebind", key::HOTKEYS_REBIND, Resolution::Take, hotkeys, cx)));
    // Grows to the left of its cap: a long list of holders wraps instead of leaving the window.
    frame
        .w(px(QUESTION_WIDTH))
        .key_context(CONTEXT)
        .track_focus(hotkeys.read(cx).focus())
        .on_action(move |_: &DismissConflict, window, cx| dismiss.update(cx, |hotkeys, cx| hotkeys.dismiss(window, cx)))
        .child(mono(text, TYPE_LABEL).text_color(rgb(TEXT_2)))
        .when(rebind, |frame| frame.child(answers))
}

/// Mouse down, not click: the cap's press-outside handler runs first and a click would find the question gone.
fn answer(id: &'static str, label: Key, resolution: Resolution, hotkeys: &Entity<Hotkeys>, cx: &App) -> Stateful<Div> {
    let hotkeys = hotkeys.clone();
    button(&t(cx, label)).id(id).on_mouse_down(MouseButton::Left, move |_, window, cx| {
        // The question's box would take the focus on this press, right after the answer gave it back.
        window.prevent_default();
        hotkeys.update(cx, |hotkeys, cx| hotkeys.answer(resolution, window, cx));
    })
}
