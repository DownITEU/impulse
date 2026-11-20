//! The ABOUT tab's update rows: what is known, CHECK NOW and CHANGELOG, the channel and the auto-check.

use gpui_kit::{
    App, Context, Entity, IntoElement, Render, SharedString, Window,
    component::{h_flex, v_flex},
    prelude::*,
    px, rgb,
};
use i18n::Arg;
use updater::{Channel, Version};

use super::{Mode, Status, Updates};
use crate::{
    about::{stacked_row, tab_row},
    settings::Settings,
    ui::{
        assets::Icon,
        key,
        text::{t, tf},
        theme::*,
        widgets::{button, choice, segmented, switch},
    },
};

pub struct UpdatePanel {
    updates: Entity<Updates>,
    settings: Entity<Settings>,
}

impl UpdatePanel {
    pub fn new(updates: Entity<Updates>, settings: Entity<Settings>, cx: &mut Context<Self>) -> Self {
        cx.observe(&updates, |_, _, cx| cx.notify()).detach();
        cx.observe(&settings, |_, _, cx| cx.notify()).detach();
        Self { updates, settings }
    }

    fn set_channel(&self, channel: Channel, cx: &mut Context<Self>) {
        self.settings.update(cx, |settings, cx| settings.edit(|prefs| prefs.update_channel = channel, cx));
    }
}

impl Render for UpdatePanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let updates = self.updates.read(cx);
        let (status, current, can_restart) = (updates.status(), updates.version().clone(), updates.can_restart(cx));
        let prefs = self.settings.read(cx).prefs();
        let (channel, auto) = (prefs.update_channel, !prefs.no_update_check);
        let accent = rgb(accent(cx));
        let check = button(&t(cx, key::UPDATE_CHECK))
            .id("update-check")
            .when(status == Status::Checking, |check| check.opacity(0.4))
            .on_click(cx.listener(|view, _, _, cx| view.updates.update(cx, |updates, cx| updates.check(cx))));
        let changelog = button(&t(cx, key::UPDATE_OPEN))
            .id("update-changelog")
            .on_click(cx.listener(|view, _, _, cx| view.updates.update(cx, |updates, cx| updates.present(Mode::Changelog, cx))));
        let restart = button(&t(cx, key::UPDATE_RESTART))
            .id("update-restart")
            .border_color(accent)
            .text_color(accent)
            .on_click(cx.listener(|view, _, _, cx| view.updates.update(cx, |updates, cx| updates.restart(cx))));
        let ready = matches!(status, Status::Ready(_));
        let buttons = h_flex().gap(px(8.)).when(ready && can_restart, |buttons| buttons.child(restart)).child(check);
        let version = stacked_row(cx, Icon::Download, ready, t(cx, key::UPDATE_TITLE), state_text(&status, &current, cx), buttons);
        let changelog = tab_row(cx, Icon::Info, false, t(cx, key::UPDATE_CHANGELOG), t(cx, key::UPDATE_CHANGELOG_HINT)).child(changelog);
        let stable = choice("update-stable", t(cx, key::UPDATE_CHANNEL_STABLE), channel == Channel::Stable, cx)
            .on_click(cx.listener(|view, _, _, cx| view.set_channel(Channel::Stable, cx)));
        let pre = choice("update-pre", t(cx, key::UPDATE_CHANNEL_PRE), channel == Channel::Pre, cx)
            .on_click(cx.listener(|view, _, _, cx| view.set_channel(Channel::Pre, cx)));
        let channels = stacked_row(
            cx,
            Icon::Layers,
            false,
            t(cx, key::UPDATE_CHANNEL),
            t(cx, key::UPDATE_CHANNEL_HINT),
            segmented().h(px(26.)).w_auto().self_start().child(stable).child(pre),
        );
        let automatic = tab_row(cx, Icon::Clock, false, t(cx, key::UPDATE_AUTO), t(cx, key::UPDATE_AUTO_HINT)).child(
            switch("update-auto", auto, cx).on_click(cx.listener(move |view, _, _, cx| {
                view.settings.update(cx, |settings, cx| settings.edit(|prefs| prefs.no_update_check = auto, cx));
            })),
        );
        v_flex().child(version).child(changelog).child(channels).child(automatic)
    }
}

fn state_text(status: &Status, current: &Version, cx: &App) -> SharedString {
    let with = |key, version: &Version| tf(cx, key, &[("version", Arg::Str(&version.to_string()))]);
    match status {
        Status::Idle => with(key::UPDATE_STATE_IDLE, current),
        Status::Current => with(key::UPDATE_STATE_CURRENT, current),
        Status::Skipped(version) => with(key::UPDATE_STATE_SKIPPED, version),
        Status::Checking => t(cx, key::UPDATE_STATE_CHECKING),
        Status::Available(version) => with(key::UPDATE_STATE_AVAILABLE, version),
        Status::Downloading(version) => with(key::UPDATE_STATE_DOWNLOADING, version),
        Status::Ready(version) => with(key::UPDATE_STATE_READY, version),
        Status::CheckFailed => t(cx, key::UPDATE_CHECK_FAILED),
        Status::InstallFailed => t(cx, key::UPDATE_INSTALL_FAILED),
    }
}
