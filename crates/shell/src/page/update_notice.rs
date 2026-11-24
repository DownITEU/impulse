//! The update dialog: the notes of an offer, of the build just installed, or the whole changelog. Like the
//! sign-in notice it waits for the notices that come first and takes the focus as it appears.

use gpui_kit::{
    App, Context, Div, Entity, EventEmitter, FocusHandle, IntoElement, Render, SharedString, Stateful, Window,
    component::{h_flex, scroll::ScrollableElement, text::TextView, v_flex},
    prelude::*,
    px, rgb,
};
use i18n::Arg;

use super::{disclaimer::Disclaimer, signed_out::SignedOut};
use crate::{
    ui::{
        dialog::dialog,
        key,
        text::{t, tf},
        theme::*,
        widgets::{button, mono},
    },
    update::{Mode, Present, Status, Updates},
};

/// The dialog came up, asked or not: whatever it would cover steps aside.
pub struct Opened;

pub struct UpdateNotice {
    updates: Entity<Updates>,
    disclaimer: Entity<Disclaimer>,
    signed_out: Entity<SignedOut>,
    /// What shows, or waits behind a notice that comes first.
    mode: Option<Mode>,
    shown: bool,
    focus: FocusHandle,
}

impl EventEmitter<Opened> for UpdateNotice {}

impl UpdateNotice {
    pub fn new(
        updates: Entity<Updates>,
        disclaimer: Entity<Disclaimer>,
        signed_out: Entity<SignedOut>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        cx.subscribe_in(&updates, window, |notice, _, Present(mode), window, cx| notice.show(*mode, window, cx)).detach();
        cx.observe_in(&updates, window, |notice, _, window, cx| notice.sync(window, cx)).detach();
        // The notices before it: this one shows when they are gone.
        cx.observe_in(&disclaimer, window, |notice, _, window, cx| notice.sync(window, cx)).detach();
        cx.observe_in(&signed_out, window, |notice, _, window, cx| notice.sync(window, cx)).detach();
        let mut notice = Self { updates, disclaimer, signed_out, mode: None, shown: false, focus: cx.focus_handle() };
        notice.sync(window, cx);
        notice
    }

    pub fn is_open(&self, cx: &App) -> bool {
        self.mode.is_some() && !self.blocked(cx)
    }

    fn blocked(&self, cx: &App) -> bool {
        self.disclaimer.read(cx).is_open(cx) || self.signed_out.read(cx).is_open(cx)
    }

    /// Opens what is due unasked, once nothing comes before it.
    fn sync(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.mode.is_none()
            && !self.blocked(cx)
            && let Some(mode) = self.updates.read(cx).due()
        {
            return self.show(mode, window, cx);
        }
        self.take_focus(window, cx);
    }

    fn show(&mut self, mode: Mode, window: &mut Window, cx: &mut Context<Self>) {
        self.mode = Some(mode);
        self.updates.update(cx, |updates, _| updates.acknowledge(mode));
        self.take_focus(window, cx);
    }

    /// Escape closes the box only while the focus is in it.
    fn take_focus(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let open = self.is_open(cx);
        if open && !self.shown {
            self.focus.focus(window, cx);
            cx.emit(Opened);
        }
        self.shown = open;
        cx.notify();
    }

    /// What was due behind it comes next.
    fn close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.mode = None;
        self.shown = false;
        self.sync(window, cx);
    }
}

impl Render for UpdateNotice {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mode = self.mode.unwrap_or(Mode::Changelog);
        let updates = self.updates.read(cx);
        let (status, notes, current) = (updates.status(), updates.notes(mode), updates.version().to_string());
        let accent = rgb(accent(cx));
        let title: SharedString = match mode {
            Mode::Available => t(cx, key::UPDATE_TITLE_AVAILABLE),
            Mode::New => tf(cx, key::UPDATE_TITLE_NEW, &[("version", Arg::Str(&current))]),
            Mode::Changelog => t(cx, key::UPDATE_CHANGELOG),
        };
        let route = updates.latest().filter(|_| mode == Mode::Available).map(|latest| {
            tf(cx, key::UPDATE_FROM_TO, &[("current", Arg::Str(&current)), ("latest", Arg::Str(&latest.version.to_string()))])
        });
        let notes = match notes.is_empty() {
            true => mono(t(cx, key::UPDATE_NO_NOTES), TYPE_LABEL).text_color(rgb(FAINT)).into_any_element(),
            false => TextView::markdown(("update-notes", mode as usize), notes)
                .on_link_click(|url, _, _, cx| {
                    // A feed's notes may hold links into other apps: not followed from here.
                    if url.starts_with("https://") || url.starts_with("http://") {
                        cx.open_url(url);
                    }
                })
                .into_any_element(),
        };
        let busy = matches!(status, Status::Ready(_)) && !updates.can_restart(cx);
        let body = v_flex()
            .flex_1()
            .min_h_0()
            .when_some(route, |body, route| body.child(mono(route, TYPE_LABEL).text_color(accent).px(px(20.)).pt(px(14.))))
            .child(
                v_flex()
                    .id("update-notes")
                    .flex_1()
                    .min_h_0()
                    .max_h(px(420.))
                    .px(px(20.))
                    .py(px(14.))
                    .text_size(px(TYPE_BODY))
                    .text_color(rgb(TEXT_2))
                    .overflow_y_scrollbar()
                    .child(notes),
            )
            .when(busy, |body| body.child(mono(t(cx, key::UPDATE_BUSY), TYPE_LABEL).text_color(rgb(WARN)).px(px(20.)).pb(px(10.))))
            .child(h_flex().justify_end().gap(px(10.)).p(px(20.)).pt(px(6.)).children(self.buttons(mode, &status, cx)));
        dialog("update", title, 640., &self.focus, Self::close, body, cx)
    }
}

impl UpdateNotice {
    /// The later or close button, and while an offer is on the way or fetched, its progress or the restart.
    fn buttons(&self, mode: Mode, status: &Status, cx: &mut Context<Self>) -> Vec<Stateful<Div>> {
        let close =
            |label| button(&t(cx, label)).id("update-close").on_click(cx.listener(|notice, _, window, cx| notice.close(window, cx)));
        if mode != Mode::Available {
            return vec![close(key::UPDATE_CLOSE)];
        }
        let accent = rgb(accent(cx));
        let skip = button(&t(cx, key::UPDATE_SKIP)).id("update-skip").on_click(cx.listener(|notice, _, window, cx| {
            notice.updates.update(cx, |updates, cx| updates.skip(cx));
            notice.close(window, cx);
        }));
        let mut buttons = vec![skip, close(key::UPDATE_LATER)];
        match status {
            Status::Downloading(_) => buttons.push(button(&t(cx, key::UPDATE_CHIP_DOWNLOADING)).id("update-wait").opacity(0.5)),
            Status::Ready(_) if self.updates.read(cx).can_restart(cx) => buttons.push(
                button(&t(cx, key::UPDATE_RESTART))
                    .id("update-restart")
                    .border_color(accent)
                    .text_color(accent)
                    .on_click(cx.listener(|notice, _, _, cx| notice.updates.update(cx, |updates, cx| updates.restart(cx)))),
            ),
            _ => {}
        }
        buttons
    }
}
