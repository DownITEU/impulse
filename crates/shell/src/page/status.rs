//! The status bar: the live run, the log's problems and the hotkeys' state.

use gpui_kit::{App, Div, Entity, IntoElement, component::h_flex, div, prelude::*, px, rgb};

use crate::{
    bindings::{Hotkeys, Target},
    library::Library,
    log_view::LogView,
    run::Runs,
    trainer::Trainers,
    ui::{
        theme::*,
        widgets::{mono, square},
    },
};

pub fn status_bar(
    log: &Entity<LogView>,
    runs: &Runs,
    library: &Library,
    trainers: &Trainers,
    hotkeys: &Hotkeys,
    cx: &App,
) -> impl IntoElement {
    let (errors, warnings) = log.read(cx).problems();
    let open = log.read(cx).is_open();
    let log = log.clone();
    let problem = |count: usize, color: u32| {
        h_flex().gap(px(6.)).child(square(7., rgb(color))).child(mono(count.to_string(), 11.).text_color(rgb(color)))
    };
    let accent = rgb(accent(cx));
    let running = runs.live().and_then(|live| library.game(live));
    h_flex()
        .h(px(30.))
        .flex_none()
        .px(px(20.))
        .gap(px(22.))
        .border_t_1()
        .border_color(line(0.09))
        .bg(rgb(STATUSBAR))
        .text_color(rgb(DIM))
        .map(|bar| match running {
            Some(game) => {
                let (active, total) = trainers.sheet(&game.id).map_or((0, 0), |sheet| (sheet.active(), sheet.cheats.len()));
                bar.child(h_flex().gap(px(8.)).child(square(7., accent)).child(mono("LIVE", 11.).text_color(accent)))
                    .child(mono(game.name.clone(), 11.))
                    .child(mono(format!("{active}/{total} ACTIVE"), 11.))
            }
            None => bar.child(mono("IDLE", 11.)),
        })
        .child(div().flex_1())
        .child(
            h_flex()
                .id("log")
                .h_full()
                .gap(px(10.))
                .cursor_pointer()
                .hover(|style| style.text_color(rgb(TEXT)))
                .when(open, |item| item.text_color(rgb(TEXT)))
                .on_click(move |_, window, cx| log.update(cx, |log, cx| log.toggle(window, cx)))
                .child(mono("LOG", 11.))
                .when(errors > 0, |item| item.child(problem(errors, DANGER)))
                .when(warnings > 0, |item| item.child(problem(warnings, WARN))),
        )
        .child(hotkeys_status(hotkeys, cx))
        .child(mono("IMPULSE 0.1", 11.).text_color(rgb(GHOST)))
}

/// The hotkeys' state; while binding, the keys that end it.
fn hotkeys_status(hotkeys: &Hotkeys, cx: &App) -> Div {
    if let Some(target) = hotkeys.capturing() {
        let keys = match target {
            Target::Key { required: true, .. } => "PRESS KEYS · ESC CANCEL · BKSP DEFAULT",
            _ => "PRESS KEYS · ESC CANCEL · BKSP DEFAULT · DEL UNBIND",
        };
        return mono(keys, 11.).text_color(rgb(accent(cx)));
    }
    match hotkeys.is_armed() {
        true => mono("HOTKEYS ON", 11.),
        false => mono("HOTKEYS OFF", 11.).text_color(rgb(GHOST)),
    }
}
