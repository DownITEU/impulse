//! The first-launch notice about the account of the apps Impulse works with. Nothing closes it but its two buttons.

use std::sync::Arc;

use client::Registry;
use gpui_kit::{App, Context, Entity, FocusHandle, IntoElement, Render, Window, prelude::*, rgb};
use i18n::Arg;

use crate::{
    settings::Settings,
    ui::{
        dialog::{message, pledge},
        key,
        text::{t, text, tf},
        theme::*,
        widgets::button,
    },
};

pub struct Disclaimer {
    settings: Entity<Settings>,
    registry: Arc<Registry>,
    focus: FocusHandle,
}

impl Disclaimer {
    pub fn new(settings: Entity<Settings>, registry: Arc<Registry>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        cx.observe(&settings, |_, _, cx| cx.notify()).detach();
        let disclaimer = Self { settings, registry, focus: cx.focus_handle() };
        if disclaimer.is_open(cx) {
            // The dialog's key handling and its Tab trap work only while the focus is in it.
            disclaimer.focus.focus(window, cx);
        }
        disclaimer
    }

    /// Not accepted yet, and some source has an account to speak of.
    pub fn is_open(&self, cx: &App) -> bool {
        !self.settings.read(cx).prefs().disclaimer_seen && !self.registry.account_names().is_empty()
    }
}

/// The names of the sources with an account, for a text that speaks of them: no catalog text names a source.
pub(super) fn account_sources(registry: &Registry, cx: &App) -> String {
    registry.account_names().iter().map(|name| text(cx, name).to_string()).collect::<Vec<_>>().join(", ")
}

impl Render for Disclaimer {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let names = account_sources(&self.registry, cx);
        let accent = rgb(accent(cx));
        let buttons = [
            // Nothing runs before the notice is accepted, and the tray icon goes with the app.
            button(&t(cx, key::DISCLAIMER_EXIT)).id("disclaimer-exit").on_click(cx.listener(|_, _, _, cx| cx.quit())),
            button(&t(cx, key::DISCLAIMER_ACCEPT)).id("disclaimer-accept").border_color(accent).text_color(accent).on_click(cx.listener(
                |view, _, _, cx| {
                    view.settings.update(cx, |settings, cx| settings.edit(|prefs| prefs.disclaimer_seen = true, cx));
                },
            )),
        ];
        let body = message(tf(cx, key::DISCLAIMER_BODY, &[("sources", Arg::Str(&names))]), buttons);
        pledge("disclaimer", t(cx, key::DISCLAIMER_TITLE), 560., &self.focus, body, cx)
    }
}
