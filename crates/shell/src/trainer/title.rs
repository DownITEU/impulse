//! A game's name over its mods, version, store and play time.

use client::library::{Game, Support};
use gpui_kit::{App, Div, FontWeight, IntoElement, component::h_flex, div, prelude::*, px, relative, rgb};

use super::Trainers;
use crate::{
    assets::Icon,
    theme::*,
    widgets::{icon, mono, tracked},
};

pub fn title(trainers: &Trainers, game: &Game, size: f32, cx: &App) -> Div {
    let has_trainer = game.support != Support::NoTrainer;
    div()
        .flex()
        .flex_col()
        .gap(px(size * 0.18))
        .child(tracked(&game.name, size, 0.04).font_weight(FontWeight::BOLD).line_height(relative(0.95)).text_color(rgb(if has_trainer {
            TEXT_BRIGHT
        } else {
            MUTED
        })))
        .child(meta(trainers, game, cx))
}

fn meta(trainers: &Trainers, game: &Game, cx: &App) -> impl IntoElement {
    let slash = || mono("/", 12.).text_color(rgb(GHOST));
    let mods = match game.support {
        Support::NoTrainer => "NO TRAINER".to_owned(),
        // The catalog count, until the trainer itself is there.
        _ => {
            let count = trainers.sheet(&game.id).map_or(game.mods as usize, |sheet| sheet.cheats.len());
            format!("{count} MODS")
        }
    };
    let version = match game.install.build {
        Some(version) => format!("V{version}"),
        None => "VERSION UNKNOWN".to_owned(),
    };
    let profile = trainers.profile(&game.id);
    h_flex()
        .flex_wrap()
        .gap_x(px(10.))
        .gap_y(px(4.))
        .text_color(rgb(MUTED))
        .child(mono(mods, 12.))
        .child(slash())
        .child(
            h_flex()
                .gap(px(6.))
                .child(mono(version, 12.))
                .when(game.support == Support::Ready, |version| version.child(icon(Icon::Check, 13., rgb(accent(cx)))))
                .when(game.support == Support::UpdatePending, |version| version.child(mono("UPDATE PENDING", 12.).text_color(rgb(WARN)))),
        )
        .child(slash())
        .child(mono(game.store.clone(), 12.))
        .when(profile.sessions > 0, |meta| {
            let sessions = if profile.sessions == 1 { "1 SESSION".to_owned() } else { format!("{} SESSIONS", profile.sessions) };
            meta.child(slash()).child(mono(played(profile.played), 12.)).child(slash()).child(mono(sessions, 12.))
        })
}

/// `42 MIN PLAYED`, then `1.2 H PLAYED`.
fn played(secs: u64) -> String {
    match secs {
        0..3600 => format!("{} MIN PLAYED", secs / 60),
        _ => format!("{:.1} H PLAYED", secs as f64 / 3600.),
    }
}
