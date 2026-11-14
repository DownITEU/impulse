//! What the page shows of a run: its button and stage, the session panel and the note over the cheat list.

use std::time::Duration;

use client::{library::Game, run::Stage};
use gpui_kit::{
    AnyElement, App, Div, Entity, FontWeight, IntoElement,
    component::{h_flex, v_flex},
    div,
    prelude::*,
    px, relative, rgb,
};

use super::{Runs, State};
use crate::{
    trainer::Trainers,
    ui::{
        assets::Icon,
        theme::*,
        widgets::{action, icon, mono, square, tracked},
    },
};

const STOPPING: &str = "STOPPING THE TRAINER";

fn label(stage: Stage) -> &'static str {
    match stage {
        Stage::Preparing => "PREPARING THE TRAINER",
        Stage::Launching => "STARTING THE GAME",
        Stage::WaitingForGame => "WAITING FOR THE GAME",
        Stage::Attaching => "LOADING THE TRAINER",
    }
}

/// LAUNCH + ATTACH, the run's stage, or STOP.
pub fn run_button(runs: &Entity<Runs>, game: &Game, cx: &App) -> impl IntoElement {
    let accent = rgb(accent(cx));
    let run = runs.read(cx);
    let state = run.state(&game.id);
    let starting = match state {
        State::Starting(stage) => Some(stage),
        _ => None,
    };
    let stoppable = starting.is_some() || matches!(state, State::Live);
    let stopping = matches!(state, State::Stopping);
    let launchable = run.can_start(&game.id, cx);
    let (stop, start, id) = (runs.clone(), runs.clone(), game.id.clone());
    h_flex()
        .gap(px(8.))
        .when_some(starting, |bar, stage| bar.child(tracked(label(stage), 12.5, 0.14).text_color(accent)))
        .when(stopping, |bar| bar.child(tracked(STOPPING, 12.5, 0.14).text_color(accent)))
        .when(stoppable, |bar| {
            bar.child(
                action("stop")
                    .px(px(16.))
                    .border_color(accent)
                    .text_color(accent)
                    .on_click(move |_, _, cx| stop.update(cx, |runs, cx| runs.stop(cx)))
                    .child(square(9., accent))
                    .child(tracked("STOP", 12.5, 0.14)),
            )
        })
        .when(launchable, |bar| {
            bar.child(
                h_flex()
                    .id("launch")
                    .occlude()
                    .h(px(38.))
                    .pl(px(16.))
                    .pr(px(10.))
                    .gap(px(12.))
                    .bg(accent)
                    .text_color(rgb(BG))
                    .font_weight(FontWeight::BOLD)
                    .cursor_pointer()
                    .on_click(move |_, _, cx| start.update(cx, |runs, cx| runs.start(id.clone(), cx)))
                    .child(icon(Icon::Play, 13., rgb(BG)))
                    .child(tracked("LAUNCH + ATTACH", 12.5, 0.14))
                    .child(mono("ENTER", 10.).font_weight(FontWeight::BOLD).px(px(6.)).py(px(2.)).border_1().border_color(alpha(BG, 0.45))),
            )
        })
}

/// The live run's clock, or the last run's time, over the active count and `scope`, the hotkeys' switch.
/// `strip`: all of it in one line that wraps.
pub fn session_panel(runs: &Runs, trainers: &Trainers, game: &Game, strip: bool, scope: AnyElement, cx: &App) -> Div {
    let accent = rgb(accent(cx));
    let running = matches!(runs.state(&game.id), State::Live);
    let last = trainers.profile(&game.id).last;
    let (title, time) = match runs.since().filter(|_| running) {
        Some(since) => ("SESSION", since.elapsed()),
        None if last > 0 => ("LAST SESSION", Duration::from_secs(last)),
        None => ("SESSION", Duration::ZERO),
    };
    let sheet = trainers.sheet(&game.id);
    let total = sheet.map_or(game.mods as usize, |sheet| sheet.cheats.len());
    let active = match (running, sheet) {
        (true, Some(sheet)) => format!("{:02}", sheet.active()),
        _ => "-".to_owned(),
    };
    let active = h_flex()
        .gap(px(8.))
        .child(mono(active, 13.).text_color(if running { accent } else { rgb(FAINT) }))
        .child(mono(format!("/ {total}"), 13.).text_color(rgb(FAINT)));
    let clock = |size: f32| {
        mono(clock(time), size).font_weight(FontWeight::BOLD).line_height(relative(1.15)).text_color(rgb(if running {
            TEXT_BRIGHT
        } else {
            FAINT
        }))
    };
    let panel = div().flex().border_1().border_color(line(0.1)).bg(rgb(PANEL)).px(px(16.));
    if strip {
        let pair = |name: &'static str| h_flex().gap(px(12.)).child(caption(name));
        return panel
            .flex_wrap()
            .items_center()
            .gap_x(px(32.))
            .gap_y(px(6.))
            .py(px(10.))
            .child(pair(title).child(clock(24.)))
            .child(div().flex_1())
            .child(pair("ACTIVE").child(active))
            .child(pair("HOTKEYS").child(scope));
    }
    // In a taller box the stats keep to its bottom.
    panel
        .flex_col()
        .justify_between()
        .pt(px(14.))
        .pb(px(6.))
        .child(v_flex().child(caption(title)).child(clock(42.).mt(px(6.)).mb(px(10.))))
        .child(v_flex().child(stat("ACTIVE").child(active)).child(stat("HOTKEYS").child(scope)))
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

/// `01:02:03`.
fn clock(time: Duration) -> String {
    let secs = time.as_secs();
    format!("{:02}:{:02}:{:02}", secs / 3600, secs / 60 % 60, secs % 60)
}

fn caption(text: &'static str) -> Div {
    mono(text, 10.5).text_color(rgb(DIM))
}

fn stat(name: &'static str) -> Div {
    h_flex().h(px(40.)).justify_between().border_t_1().border_color(line(0.07)).child(caption(name))
}
