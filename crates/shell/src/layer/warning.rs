//! The exclusive fullscreen warning: a dialog as the overlay goes over such a game, until the user turns it off, and the
//! chip that stays in the overlay's top bar.

use std::mem;

use gpui_kit::{
    App, Context, Entity, EventEmitter, FocusHandle, IntoElement, Render, Window, component::h_flex, prelude::*, px, relative, rgb,
};

use crate::{
    settings::Settings,
    ui::{
        assets::Icon,
        dialog::{dialog, message},
        key,
        text::t,
        theme::*,
        widgets::{button, icon, mono},
    },
};

/// The dialog closed: the overlay takes the focus back.
pub(super) struct Closed;

pub(super) struct Warning {
    settings: Entity<Settings>,
    open: bool,
    focus: FocusHandle,
}

impl EventEmitter<Closed> for Warning {}

impl Warning {
    pub fn new(settings: Entity<Settings>, cx: &mut Context<Self>) -> Self {
        Self { settings, open: false, focus: cx.focus_handle() }
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    /// The overlay went over a game in exclusive fullscreen: asked on each entry unless the user turned it off.
    pub fn entered(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.settings.read(cx).prefs().skip_exclusive_warning {
            return;
        }
        self.focus.focus(window, cx);
        self.open = true;
        cx.notify();
    }

    /// A map that opened late took the focus: the question is modal, so it takes it back.
    pub fn refocus(&self, window: &mut Window, cx: &mut App) {
        if self.open {
            self.focus.focus(window, cx);
        }
    }

    /// Closes the question; whether it was open.
    pub fn hide(&mut self, cx: &mut Context<Self>) -> bool {
        let was = mem::take(&mut self.open);
        if was {
            cx.notify();
        }
        was
    }

    fn close(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        self.hide(cx);
        cx.emit(Closed);
    }
}

impl Render for Warning {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let buttons = [
            button(&t(cx, key::OVERLAY_EXCLUSIVE_SKIP)).id("exclusive-skip").on_click(cx.listener(|view, _, window, cx| {
                view.settings.update(cx, |settings, cx| settings.edit(|prefs| prefs.skip_exclusive_warning = true, cx));
                view.close(window, cx);
            })),
            button(&t(cx, key::OVERLAY_EXCLUSIVE_OK))
                .id("exclusive-ok")
                .on_click(cx.listener(|view, _, window, cx| view.close(window, cx))),
        ];
        let body = message(t(cx, key::OVERLAY_EXCLUSIVE_BODY), buttons);
        dialog("exclusive-warning", t(cx, key::OVERLAY_EXCLUSIVE_TITLE), 480., &self.focus, Self::close, body, cx)
    }
}

/// Under the game's title while the layer may not show over the game. Low, so title and chip end above the docks.
pub(super) fn chip(cx: &App) -> impl IntoElement {
    h_flex()
        .min_w_0()
        .max_w_full()
        .gap(px(8.))
        .px(px(10.))
        .py(px(4.))
        .border_1()
        .border_color(alpha(WARN, 0.45))
        .bg(alpha(BG, 0.85))
        .child(icon(Icon::Warn, 12., rgb(WARN)).flex_none())
        .child(mono(t(cx, key::OVERLAY_EXCLUSIVE_CHIP), 10.5).line_height(relative(1.)).truncate().text_color(rgb(WARN)))
}
