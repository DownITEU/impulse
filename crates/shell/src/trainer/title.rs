//! A game's name over its mods, version, store and play time.

use client::library::{Game, Support};
use gpui_kit::{App, Div, FontWeight, IntoElement, SharedString, component::h_flex, div, prelude::*, px, relative, rgb};
use i18n::Arg;

use super::Trainers;
use crate::ui::{
    assets::Icon,
    key,
    text::{digits, t, tf},
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
        Support::NoTrainer => t(cx, key::TRAINER_NONE),
        // The catalog count, until the trainer itself is there.
        _ => {
            let count = trainers.sheet(&game.id).map_or(game.mods as usize, |sheet| sheet.cheats.len());
            tf(cx, key::TRAINER_MODS, &[("count", Arg::Num(count as f64))])
        }
    };
    let version = match game.install.build {
        Some(version) => tf(cx, key::TRAINER_VERSION, &[("version", Arg::Num(version.into()))]),
        None => t(cx, key::TRAINER_VERSION_UNKNOWN),
    };
    let profile = trainers.game_profile(&game.id);
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
                .when(game.support == Support::UpdatePending, |version| {
                    version.child(mono(t(cx, key::TRAINER_UPDATE_PENDING), 12.).text_color(rgb(WARN)))
                }),
        )
        .child(slash())
        .child(mono(game.store.clone().map_or_else(|| t(cx, key::PAGE_CUSTOM), Into::into), 12.))
        .when(profile.sessions > 0, |meta| {
            let sessions = tf(cx, key::TRAINER_SESSIONS, &[("count", Arg::Num(profile.sessions as f64))]);
            meta.child(slash()).child(mono(played(profile.played, cx), 12.)).child(slash()).child(mono(sessions, 12.))
        })
}

/// `42 MIN PLAYED`, then `1.2 H PLAYED`.
fn played(secs: u64, cx: &App) -> SharedString {
    match secs {
        0..3600 => tf(cx, key::TRAINER_PLAYED_MIN, &[("count", Arg::Num((secs / 60) as f64))]),
        _ => {
            let hours = secs as f64 / 3600.;
            tf(cx, key::TRAINER_PLAYED_HOURS, &[("hours", Arg::Str(&digits(cx, &format!("{hours:.1}")))), ("count", Arg::Num(hours))])
        }
    }
}
