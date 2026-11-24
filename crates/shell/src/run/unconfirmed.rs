//! The question before a run on a build the trainer was not confirmed on: some mods may not work.

use client::library::{GameId, Support};
use gpui_kit::{Context, Entity, EventEmitter, FocusHandle, IntoElement, Render, Window, prelude::*};

use super::Runs;
use crate::{
    settings::Settings,
    ui::{
        dialog::{dialog, message},
        key,
        text::t,
        widgets::button,
    },
};

/// The question closed: whoever drew it takes the focus back.
pub struct Closed;

pub struct Unconfirmed {
    runs: Entity<Runs>,
    settings: Entity<Settings>,
    asking: Option<GameId>,
    focus: FocusHandle,
}

impl EventEmitter<Closed> for Unconfirmed {}

impl Unconfirmed {
    pub fn new(runs: Entity<Runs>, settings: Entity<Settings>, cx: &mut Context<Self>) -> Self {
        Self { runs, settings, asking: None, focus: cx.focus_handle() }
    }

    pub fn is_open(&self) -> bool {
        self.asking.is_some()
    }

    /// LAUNCH: the run starts, after the question when the game's build is newer than the trainer's.
    pub fn launch(&mut self, game: GameId, support: Support, window: &mut Window, cx: &mut Context<Self>) {
        if support != Support::UpdatePending || self.settings.read(cx).prefs().skip_unconfirmed_warning {
            return self.runs.update(cx, |runs, cx| runs.start(game, cx));
        }
        self.asking = Some(game);
        self.focus.focus(window, cx);
        cx.notify();
    }

    fn answer(&mut self, play: bool, cx: &mut Context<Self>) {
        if let Some(game) = self.asking.take().filter(|_| play) {
            self.runs.update(cx, |runs, cx| runs.start(game, cx));
        }
        cx.emit(Closed);
        cx.notify();
    }

    fn close(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        self.answer(false, cx);
    }
}

impl Render for Unconfirmed {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let buttons = [
            button(&t(cx, key::RUN_UNCONFIRMED_SKIP)).id("unconfirmed-skip").on_click(cx.listener(|view, _, _, cx| {
                view.settings.update(cx, |settings, cx| settings.edit(|prefs| prefs.skip_unconfirmed_warning = true, cx));
                view.answer(true, cx);
            })),
            button(&t(cx, key::RUN_UNCONFIRMED_CANCEL))
                .id("unconfirmed-cancel")
                .on_click(cx.listener(|view, _, _, cx| view.answer(false, cx))),
            button(&t(cx, key::RUN_UNCONFIRMED_PLAY)).id("unconfirmed-play").on_click(cx.listener(|view, _, _, cx| view.answer(true, cx))),
        ];
        let body = message(t(cx, key::RUN_UNCONFIRMED_BODY), buttons);
        dialog("unconfirmed-build", t(cx, key::RUN_UNCONFIRMED_TITLE), 480., &self.focus, Self::close, body, cx)
    }
}
