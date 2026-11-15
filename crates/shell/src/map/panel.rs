//! The open game's map panel on the page: its first map's preview; a click opens the map.

use std::sync::Arc;

use client::library::Game;
use gpui_kit::{
    App, ClickEvent, Div, Entity, IntoElement, ObjectFit, Window,
    component::{h_flex, v_flex},
    div, img,
    prelude::*,
    px, rgb,
};

use super::MapView;
use crate::ui::{
    assets::Icon,
    key,
    text::t,
    theme::*,
    widgets::{brackets, grid, icon_button, mono},
};

const HEIGHT: f32 = 204.;

/// The caller gives it a width.
pub fn map_panel(view: &Entity<MapView>, game: &Arc<Game>, cx: &App) -> Div {
    let offer = game.source.maps.first();
    v_flex()
        .relative()
        .border_1()
        .border_color(line(0.1))
        .bg(rgb(PANEL))
        .child(
            h_flex()
                .h(px(34.))
                .pl(px(12.))
                .pr(px(6.))
                .justify_between()
                .border_b_1()
                .border_color(line(0.07))
                .child(
                    h_flex()
                        .gap(px(6.))
                        .text_color(rgb(MUTED))
                        .child(mono(t(cx, key::MAP_TITLE), 10.5))
                        .child(mono("/", 10.5).text_color(rgb(GHOST)))
                        .child(mono(offer.map_or("-".into(), |offer| offer.name.to_uppercase()), 10.5)),
                )
                .when(offer.is_some(), |header| header.child(icon_button("map", Icon::Expand, 26., 13.).on_click(opens(view, game)))),
        )
        .child(match offer {
            Some(offer) => div()
                .id("map-preview")
                .relative()
                .h(px(HEIGHT))
                .overflow_hidden()
                .cursor_pointer()
                .on_click(opens(view, game))
                .child(grid(20., line(0.035)))
                // Out of the flow: an image in it takes its own height at the box's width.
                .when_some(offer.thumbnail.clone(), |preview, url| {
                    preview.child(img(url).absolute().top_0().left_0().size_full().object_fit(ObjectFit::Cover))
                })
                .into_any_element(),
            None => div()
                .h(px(HEIGHT))
                .p(px(16.))
                .flex()
                .items_center()
                .justify_center()
                .text_center()
                .text_color(rgb(FAINT))
                .child(mono(t(cx, key::MAP_NONE), 11.))
                .into_any_element(),
        })
        .children(brackets(cx))
}

fn opens(view: &Entity<MapView>, game: &Arc<Game>) -> impl Fn(&ClickEvent, &mut Window, &mut App) + 'static {
    let (view, game) = (view.clone(), game.clone());
    move |_, window, cx| view.update(cx, |map, cx| map.show(&game, window, cx))
}
