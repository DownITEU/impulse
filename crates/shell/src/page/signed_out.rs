//! The notice that no source could sign in: its app is not installed, not signed in, or not reachable.
//! Unlike the disclaimer it can be closed; the header keeps saying so.

use std::sync::Arc;

use client::Registry;
use gpui_kit::{App, Context, Entity, FocusHandle, IntoElement, Render, Window, component::v_flex, div, prelude::*, px, rgb};
use i18n::Arg;

use super::disclaimer::{Disclaimer, account_sources};
use crate::{
    account::Account,
    ui::{
        dialog::{dialog, message},
        key,
        text::{failed, t, tf},
        theme::{DIM, WARN},
        widgets::button,
    },
};

pub struct SignedOut {
    account: Entity<Account>,
    disclaimer: Entity<Disclaimer>,
    registry: Arc<Registry>,
    // Closed by the user, until the next start.
    dismissed: bool,
    shown: bool,
    focus: FocusHandle,
}

impl SignedOut {
    pub fn new(
        account: Entity<Account>,
        disclaimer: Entity<Disclaimer>,
        registry: Arc<Registry>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        cx.observe_in(&account, window, |view, _, window, cx| view.sync(window, cx)).detach();
        // The disclaimer comes first: this one shows when it is accepted.
        cx.observe_in(&disclaimer, window, |view, _, window, cx| view.sync(window, cx)).detach();
        Self { account, disclaimer, registry, dismissed: false, shown: false, focus: cx.focus_handle() }
    }

    pub fn is_open(&self, cx: &App) -> bool {
        !self.dismissed && self.account.read(cx).signed_out() && !self.disclaimer.read(cx).is_open(cx)
    }

    /// The box takes the focus as it appears: Escape closes it only while the focus is in it.
    fn sync(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let open = self.is_open(cx);
        if open && !self.shown {
            self.focus.focus(window, cx);
        }
        self.shown = open;
        cx.notify();
    }

    fn close(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        self.dismissed = true;
        cx.notify();
    }

    fn retry(&mut self, cx: &mut Context<Self>) {
        self.account.update(cx, |account, cx| account.reload(cx));
    }
}

impl Render for SignedOut {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let names = account_sources(&self.registry, cx);
        let buttons = [
            button(&t(cx, key::SIGNED_OUT_CLOSE))
                .id("signed-out-close")
                .on_click(cx.listener(|view, _, window, cx| view.close(window, cx))),
            button(&t(cx, key::SIGNED_OUT_RETRY)).id("signed-out-retry").on_click(cx.listener(|view, _, _, cx| view.retry(cx))),
        ];
        // A retry that fails again changes nothing else: the line shows it was tried.
        let account = self.account.read(cx);
        let status = match (account.trying(), account.failure()) {
            (true, _) => Some((t(cx, key::SIGNED_OUT_TRYING), DIM)),
            (false, Some(failure)) => Some((failed(cx, key::SIGNED_OUT_WHY, failure), WARN)),
            (false, None) => None,
        };
        let text = v_flex()
            .gap(px(12.))
            .child(tf(cx, key::SIGNED_OUT_BODY, &[("sources", Arg::Str(&names))]))
            .children(status.map(|(line, color)| div().text_color(rgb(color)).child(line)));
        let body = message(text, buttons);
        dialog("signed-out", t(cx, key::SIGNED_OUT_TITLE), 560., &self.focus, Self::close, body, cx)
    }
}
