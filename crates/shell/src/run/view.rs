//! What the page shows of a run: its button and stage, the session panel and the note over the cheat list.

use std::time::Duration;

use client::{
    library::{Game, GameId},
    run::Stage,
};
use gpui_kit::{
    AnyElement, App, Div, Entity, FontWeight, IntoElement, SharedString,
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
        key,
        text::{t, why},
        theme::*,
        widgets::{action, icon, mono, primary_action, square, tracked},
    },
};

fn label(stage: Stage, cx: &App) -> SharedString {
    t(
        cx,
        match stage {
            Stage::Preparing => key::RUN_PREPARING,
            Stage::Launching => key::RUN_LAUNCHING,
            Stage::WaitingForGame => key::RUN_WAITING,
            Stage::Attaching => key::RUN_ATTACHING,
        },
    )
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
    let launchable = run.can_start(game, cx);
    let stopping_text = t(cx, key::RUN_STOPPING);
    let stop_text = t(cx, key::RUN_STOP);
    let launch_text = t(cx, key::RUN_LAUNCH);
    let enter_text = t(cx, key::RUN_ENTER);
    let (stop, start, id) = (runs.clone(), runs.clone(), game.id.clone());
    h_flex()
        .gap(px(8.))
        .when_some(starting, |bar, stage| bar.child(tracked(&label(stage, cx), TYPE_FIELD, 0.14).text_color(accent)))
        .when(stopping, |bar| bar.child(tracked(&stopping_text, TYPE_FIELD, 0.14).text_color(accent)))
        .when(stoppable, |bar| {
            bar.child(
                action("stop")
                    .px(px(16.))
                    .border_color(accent)
                    .text_color(accent)
                    .on_click(move |_, _, cx| stop.update(cx, |runs, cx| runs.stop(cx)))
                    .child(square(9., accent))
                    .child(tracked(&stop_text, TYPE_FIELD, 0.14)),
            )
        })
        .when(launchable, |bar| {
            bar.child(
                primary_action("launch", cx)
                    .pl(px(16.))
                    .pr(px(10.))
                    .gap(px(12.))
                    .on_click(move |_, _, cx| start.update(cx, |runs, cx| runs.start(id.clone(), cx)))
                    .child(icon(Icon::Play, 13., rgb(BG)))
                    .child(tracked(&launch_text, TYPE_FIELD, 0.14))
                    .child(
                        mono(enter_text, TYPE_CAPTION)
                            .font_weight(FontWeight::BOLD)
                            .px(px(6.))
                            .py(px(2.))
                            .border_1()
                            .border_color(alpha(BG, 0.45)),
                    ),
            )
        })
}

/// The live run's clock, or the last run's time, over the active count and `scope`, the hotkeys' switch.
/// `strip`: all of it in one line that wraps.
pub fn session_panel(runs: &Runs, trainers: &Trainers, game: &Game, strip: bool, scope: AnyElement, cx: &App) -> Div {
    let accent = rgb(accent(cx));
    let running = matches!(runs.state(&game.id), State::Live);
    let last = trainers.game_profile(&game.id).last;
    let (title, time) = match runs.since().filter(|_| running) {
        Some(since) => (t(cx, key::RUN_SESSION), since.elapsed()),
        None if last > 0 => (t(cx, key::RUN_LAST_SESSION), Duration::from_secs(last)),
        None => (t(cx, key::RUN_SESSION), Duration::ZERO),
    };
    let sheet = trainers.sheet(&game.id);
    let total = sheet.map_or(game.mods as usize, |sheet| sheet.cheats.len());
    let active = match (running, sheet) {
        (true, Some(sheet)) => format!("{:02}", sheet.active()),
        _ => "-".to_owned(),
    };
    let active = h_flex()
        .gap(px(8.))
        .child(mono(active, TYPE_BODY).text_color(if running { accent } else { rgb(FAINT) }))
        .child(mono(format!("/ {total}"), TYPE_BODY).text_color(rgb(FAINT)));
    let clock = |size: f32| {
        mono(clock(time), size).font_weight(FontWeight::BOLD).line_height(relative(1.15)).text_color(rgb(if running {
            TEXT_BRIGHT
        } else {
            FAINT
        }))
    };
    let panel = div().flex().border_1().border_color(line(0.1)).bg(rgb(PANEL)).px(px(16.));
    if strip {
        let pair = |name: SharedString| h_flex().gap(px(12.)).child(caption(name));
        return panel
            .flex_wrap()
            .items_center()
            .gap_x(px(32.))
            .gap_y(px(6.))
            .py(px(10.))
            .child(pair(title).child(clock(TYPE_DISPLAY)))
            .child(div().flex_1())
            .child(pair(t(cx, key::RUN_ACTIVE)).child(active))
            .child(pair(t(cx, key::RUN_HOTKEYS)).child(scope));
    }
    // In a taller box the stats keep to its bottom.
    panel
        .flex_col()
        .justify_between()
        .pt(px(14.))
        .pb(px(6.))
        .child(v_flex().child(caption(title)).child(clock(TYPE_CLOCK).mt(px(6.)).mb(px(10.))))
        .child(v_flex().child(stat(t(cx, key::RUN_ACTIVE)).child(active)).child(stat(t(cx, key::RUN_HOTKEYS)).child(scope)))
}

/// The line above the cheat list while the trainer is not live.
pub fn run_note(runs: &Runs, game: &GameId, cx: &App) -> Div {
    match runs.state(game) {
        State::Failed(error) => mono(why(cx, error), TYPE_LABEL).text_color(rgb(WARN)),
        State::Starting(stage) => mono(label(stage, cx), TYPE_LABEL).text_color(rgb(accent(cx))),
        State::Stopping => mono(t(cx, key::RUN_STOPPING), TYPE_LABEL).text_color(rgb(accent(cx))),
        State::Idle | State::Live => mono(t(cx, key::RUN_IDLE), TYPE_LABEL).text_color(rgb(DIM)),
    }
}

/// `01:02:03`.
fn clock(time: Duration) -> String {
    let secs = time.as_secs();
    format!("{:02}:{:02}:{:02}", secs / 3600, secs / 60 % 60, secs % 60)
}

fn caption(text: SharedString) -> Div {
    mono(text, TYPE_CAPTION).text_color(rgb(DIM))
}

fn stat(name: SharedString) -> Div {
    h_flex().h(px(40.)).justify_between().border_t_1().border_color(line(0.07)).child(caption(name))
}
