//! The window's content: the page, replayed as drawn while the open map over it moves. Over a game the page
//! is the overlay's HUD, with the map in a dock of its own.

use std::{path::PathBuf, sync::Arc};

use client::storage::Paths;
use gpui_kit::{Context, Entity, IntoElement, Render, StyleRefinement, Window, div, prelude::*, px, rgb};
use model::Provider;

use super::{
    Page,
    map::{MapEvent, MapView},
    theme::*,
};

// The open map's distance from the window's edges.
const MAP_INSET: f32 = 40.;

pub struct Root {
    page: Entity<Page>,
    map: Entity<MapView>,
}

impl Root {
    pub fn new(provider: Arc<dyn Provider>, data_dir: PathBuf, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let page = cx.new(|cx| Page::new(provider, Paths::new(data_dir), window, cx));
        let map = page.read(cx).map.clone();
        cx.observe(&map, |_, _, cx| cx.notify()).detach();
        Self { page, map }
    }

    /// The map over the dimmed page; a click beside it closes it.
    fn modal(&self) -> impl IntoElement {
        let map = self.map.clone();
        div()
            .id("map-backdrop")
            .occlude()
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .p(px(MAP_INSET))
            .bg(alpha(SUNKEN, 0.78))
            .on_click(move |_, _, cx| map.update(cx, |_, cx| cx.emit(MapEvent::Close)))
            .child(
                div()
                    .id("map-modal")
                    .size_full()
                    .border_1()
                    .border_color(line(0.14))
                    .on_click(|_, _, cx| cx.stop_propagation())
                    .child(self.map.clone()),
            )
    }
}

impl Render for Root {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let page = self.page.read(cx);
        let modal = !page.over && page.maps.read(cx).is_open(cx);
        div()
            .relative()
            .size_full()
            .font_family(UI)
            .text_size(px(14.))
            .text_color(rgb(TEXT))
            .child(self.page.clone().cached(StyleRefinement::default().size_full()))
            .when(modal, |root| root.child(self.modal()))
    }
}
