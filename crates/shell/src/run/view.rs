//! What the page prints of a run: its stage while it starts and stops, and the note over the cheat list.

use client::run::Stage;
use gpui_kit::{App, Div, prelude::*, rgb};

use super::{Runs, State};
use crate::{theme::*, widgets::mono};

pub const STOPPING: &str = "STOPPING THE TRAINER";

pub fn label(stage: Stage) -> &'static str {
    match stage {
        Stage::Preparing => "PREPARING THE TRAINER",
        Stage::Launching => "STARTING THE GAME",
        Stage::WaitingForGame => "WAITING FOR THE GAME",
        Stage::Attaching => "LOADING THE TRAINER",
    }
}

/// The line above the cheat list while the trainer is not live.
pub fn run_note(runs: &Runs, game: &str, cx: &App) -> Div {
    match runs.state(game) {
        State::Failed(error) => mono(error.to_owned(), 11.).text_color(rgb(WARN)),
        State::Starting(stage) => mono(label(stage), 11.).text_color(rgb(accent(cx))),
        State::Stopping => mono(STOPPING, 11.).text_color(rgb(accent(cx))),
        State::Idle | State::Live => mono("TRAINER IDLE · CONTROLS UNLOCK AFTER LAUNCH", 11.).text_color(rgb(DIM)),
    }
}
