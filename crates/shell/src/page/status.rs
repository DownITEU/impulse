//! The status bar: the live run, the log's problems and the hotkeys' state.

use gpui_kit::{App, Div, Entity, IntoElement, component::h_flex, div, prelude::*, px, rgb};
use i18n::{Arg, Key};

use crate::{
    about::Link,
    bindings::{Hotkeys, Target},
    library::Library,
    log_view::LogView,
    run::Runs,
    trainer::Trainers,
    ui::{
        assets::Icon,
        key,
        text::{t, tf},
        theme::*,
        widgets::{mono, square, tinted_icon_button, tip},
    },
    update::{Mode, Updates, chip},
};

pub fn status_bar(
    log: &Entity<LogView>,
    updates: &Entity<Updates>,
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
        .child(links(cx))
        .map(|bar| match running {
            Some(game) => {
                let (active, total) = trainers.sheet(&game.id).map_or((0, 0), |sheet| (sheet.active(), sheet.cheats.len()));
                bar.child(h_flex().gap(px(8.)).child(square(7., accent)).child(mono(t(cx, key::PAGE_LIVE), 11.).text_color(accent)))
                    .child(mono(game.name.clone(), 11.))
                    .child(mono(tf(cx, key::PAGE_ACTIVE, &[("active", Arg::Num(active as f64)), ("total", Arg::Num(total as f64))]), 11.))
            }
            None => bar.child(mono(t(cx, key::PAGE_IDLE), 11.)),
        })
        .child(div().flex_1())
        .children(chip(updates, cx))
        .child(
            h_flex()
                .id("log")
                .h_full()
                .gap(px(10.))
                .cursor_pointer()
                .hover(|style| style.text_color(rgb(TEXT)))
                .when(open, |item| item.text_color(rgb(TEXT)))
                .on_click(move |_, window, cx| log.update(cx, |log, cx| log.toggle(window, cx)))
                .child(mono(t(cx, key::PAGE_LOG), 11.))
                .when(errors > 0, |item| item.child(problem(errors, DANGER)))
                .when(warnings > 0, |item| item.child(problem(warnings, WARN))),
        )
        .child(hotkeys_status(hotkeys, cx))
        .child(version(updates))
}

/// The build's version; a click opens the changelog.
fn version(updates: &Entity<Updates>) -> impl IntoElement {
    let updates = updates.clone();
    div()
        .id("version")
        .cursor_pointer()
        .text_color(rgb(GHOST))
        .hover(|style| style.text_color(rgb(DIM)))
        .on_click(move |_, _, cx| updates.update(cx, |updates, cx| updates.present(Mode::Changelog, cx)))
        .child(mono(concat!("IMPULSE ", env!("CARGO_PKG_VERSION")), 11.))
}

/// The project's places, one click from anywhere; support rests in the accent.
fn links(cx: &App) -> impl IntoElement {
    let quick = |link: Link, glyph: Icon, title: Key, color: u32| {
        let text = t(cx, title);
        tinted_icon_button(title.0, glyph, 22., 13., color)
            .tooltip(move |window, cx| tip(text.clone(), window, cx))
            .on_click(move |_, _, cx| link.open(cx))
    };
    h_flex()
        .gap(px(2.))
        .child(quick(Link::Repo, Icon::Github, key::ABOUT_GITHUB, DIM))
        .child(quick(Link::Discord, Icon::Discord, key::ABOUT_DISCORD, DIM))
        .child(quick(Link::Bug, Icon::Bug, key::ABOUT_BUG, DIM))
        .child(quick(Link::Support, Icon::Coffee, key::ABOUT_SUPPORT, accent(cx)))
        .child(div().w(px(1.)).h(px(12.)).ml(px(8.)).bg(line(0.12)))
}

/// The hotkeys' state; while binding, the keys that end it.
fn hotkeys_status(hotkeys: &Hotkeys, cx: &App) -> Div {
    // A cheat's slot has its keys in a popover over its cap.
    if let Some(Target::Key { required, .. }) = hotkeys.capturing() {
        let keys = t(cx, if *required { key::PAGE_BINDING } else { key::PAGE_BINDING_UNBIND });
        return mono(keys, 11.).text_color(rgb(accent(cx)));
    }
    match hotkeys.is_armed() {
        true => mono(t(cx, key::PAGE_HOTKEYS_ON), 11.),
        false => mono(t(cx, key::PAGE_HOTKEYS_OFF), 11.).text_color(rgb(GHOST)),
    }
}
