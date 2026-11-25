//! LAUNCH's questions: before a run on a build the trainer was not confirmed on (some mods may not work), and after one
//! that found no game when its trainer attaches only to a game the user starts.

use client::library::{GameId, Support};
use gpui_kit::{Context, Entity, EventEmitter, FocusHandle, IntoElement, Render, Window, prelude::*};
use model::FailureKind;

use super::{RunEvent, Runs, State};
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

enum Question {
    Unconfirmed(GameId),
    StartOutside(GameId),
}

pub struct Ask {
    runs: Entity<Runs>,
    settings: Entity<Settings>,
    asking: Option<Question>,
    focus: FocusHandle,
    // Had the focus before the question: it gets it back.
    before: Option<FocusHandle>,
}

impl EventEmitter<Closed> for Ask {}

impl Ask {
    pub fn new(runs: Entity<Runs>, settings: Entity<Settings>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        cx.subscribe_in(&runs, window, |ask, runs, event: &RunEvent, window, cx| match event {
            // A newer run makes it moot.
            RunEvent::Starting if matches!(ask.asking, Some(Question::StartOutside(_))) => ask.answer(false, window, cx),
            // An open question keeps its place: the failure shows under the hero all the same.
            RunEvent::Failed if ask.asking.is_none() => {
                let runs = runs.read(cx);
                let outside =
                    |game: &&GameId| matches!(runs.state(game), State::Failed(error) if error.kind == FailureKind::ExternalLaunch);
                if let Some(game) = runs.game().filter(outside).cloned() {
                    ask.open(Question::StartOutside(game), window, cx);
                }
            }
            _ => {}
        })
        .detach();
        Self { runs, settings, asking: None, focus: cx.focus_handle(), before: None }
    }

    pub fn is_open(&self) -> bool {
        self.asking.is_some()
    }

    /// LAUNCH: the run starts, after the question when the game's build is newer than the trainer's.
    pub fn launch(&mut self, game: GameId, support: Support, window: &mut Window, cx: &mut Context<Self>) {
        if support != Support::UpdatePending || self.settings.read(cx).prefs().skip_unconfirmed_warning {
            return self.runs.update(cx, |runs, cx| runs.start(game, cx));
        }
        self.open(Question::Unconfirmed(game), window, cx);
    }

    fn open(&mut self, question: Question, window: &mut Window, cx: &mut Context<Self>) {
        self.asking = Some(question);
        self.before = window.focused(cx);
        self.focus.focus(window, cx);
        cx.notify();
    }

    fn answer(&mut self, yes: bool, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(before) = self.before.take().filter(|_| self.focus.is_focused(window)) {
            before.focus(window, cx);
        }
        match self.asking.take().filter(|_| yes) {
            Some(Question::Unconfirmed(game)) => self.runs.update(cx, |runs, cx| runs.start(game, cx)),
            Some(Question::StartOutside(game)) => self.runs.update(cx, |runs, cx| runs.start_game(game, cx)),
            None => {}
        }
        cx.emit(Closed);
        cx.notify();
    }

    fn close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.answer(false, window, cx);
    }
}

impl Render for Ask {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let cancel = button(&t(cx, key::RUN_ASK_CANCEL))
            .id("ask-cancel")
            .on_click(cx.listener(|view, _, window, cx| view.answer(false, window, cx)));
        let yes =
            |caption| button(&t(cx, caption)).id("ask-yes").on_click(cx.listener(|view, _, window, cx| view.answer(true, window, cx)));
        if let Some(Question::StartOutside(_)) = self.asking {
            let body = message(t(cx, key::RUN_OUTSIDE_BODY), [cancel, yes(key::RUN_OUTSIDE_START)]);
            return dialog("start-outside", t(cx, key::RUN_OUTSIDE_TITLE), 480., &self.focus, Self::close, body, cx).into_any_element();
        }
        let always = button(&t(cx, key::RUN_UNCONFIRMED_SKIP)).id("unconfirmed-skip").on_click(cx.listener(|view, _, window, cx| {
            view.settings.update(cx, |settings, cx| settings.edit(|prefs| prefs.skip_unconfirmed_warning = true, cx));
            view.answer(true, window, cx);
        }));
        let body = message(t(cx, key::RUN_UNCONFIRMED_BODY), [always, cancel, yes(key::RUN_UNCONFIRMED_PLAY)]);
        dialog("unconfirmed-build", t(cx, key::RUN_UNCONFIRMED_TITLE), 480., &self.focus, Self::close, body, cx).into_any_element()
    }
}
