//! The open game's map panel on the page: its first map's preview, a click opens the map; without a map, LINK A MAP
//! while a source has titles.

use std::sync::Arc;

use client::library::Game;
use gpui_kit::{
    App, ClickEvent, Div, Entity, IntoElement, ObjectFit, Window,
    component::{h_flex, v_flex},
    div, img,
    prelude::*,
    px, rgb,
};

use super::{MapView, Maps, TitlePicker};
use crate::ui::{
    assets::Icon,
    key,
    text::t,
    theme::*,
    widgets::{brackets, button, grid, icon_button, mono},
};

const HEIGHT: f32 = 204.;

/// Whether the page shows the panel: a map, or a title to link.
pub fn paneled(view: &Entity<MapView>, cx: &App) -> bool {
    let maps = view.read(cx).maps.read(cx);
    maps.mapped() || linkable(maps, cx)
}

/// A source has titles to link the game to, or one has not been fetched yet (the picker asks it).
pub(super) fn linkable(maps: &Maps, cx: &App) -> bool {
    let titles = maps.titles().read(cx);
    !titles.all().is_empty() || titles.pending()
}

/// The caller gives it a width.
pub fn map_panel(view: &Entity<MapView>, game: &Arc<Game>, cx: &App) -> Div {
    let maps = view.read(cx).maps.read(cx);
    let offer = maps.offers().first();
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
                .when(offer.is_some(), |header| header.child(icon_button("map", Icon::Expand, 26., 13.).on_click(opens(view)))),
        )
        .child(match offer {
            Some(offer) => div()
                .id("map-preview")
                .relative()
                .h(px(HEIGHT))
                .overflow_hidden()
                .cursor_pointer()
                .on_click(opens(view))
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
                .flex_col()
                .gap(px(14.))
                .items_center()
                .justify_center()
                .text_center()
                .text_color(rgb(FAINT))
                .child(mono(t(cx, key::MAP_NONE), 11.))
                .when(linkable(maps, cx), |empty| {
                    empty.child(button(&t(cx, key::MAP_LINK)).id("map-link").on_click(links(view.read(cx).linker(), game)))
                })
                .into_any_element(),
        })
        .children(brackets(cx))
}

fn opens(view: &Entity<MapView>) -> impl Fn(&ClickEvent, &mut Window, &mut App) + 'static {
    let view = view.clone();
    move |_, window, cx| view.update(cx, |map, cx| map.show(window, cx))
}

fn links(linker: &Entity<TitlePicker>, game: &Arc<Game>) -> impl Fn(&ClickEvent, &mut Window, &mut App) + 'static {
    let (linker, game) = (linker.clone(), game.clone());
    move |_, window, cx| linker.update(cx, |linker, cx| linker.open(&game, window, cx))
}
