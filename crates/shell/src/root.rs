//! The window's content: the page, replayed as drawn while the open map over it moves. Over a game the page
//! is the overlay's HUD, with the map in a dock of its own.

use std::{path::PathBuf, sync::Arc};

use client::{Registry, storage::Paths};
use gpui_kit::{Context, Entity, IntoElement, Render, StyleRefinement, Window, div, prelude::*, px, rgb};

use crate::{
    map::{MapEvent, MapView},
    page::Page,
    remote::Serve,
    ui::{dialog::backdrop, theme::*},
};

const MAP_INSET: f32 = 40.;

pub struct Root {
    page: Entity<Page>,
    map: Entity<MapView>,
}

impl Root {
    /// `serve`: the remote panel's server, in a build with it.
    pub fn new(
        registry: Arc<Registry>,
        plugins: Arc<client::plugins::Plugins>,
        data_dir: PathBuf,
        serve: Option<Serve>,
        elevated: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let page = cx.new(|cx| Page::new(registry, plugins, Paths::new(data_dir), serve, elevated, window, cx));
        let map = page.read(cx).map().clone();
        cx.observe(&map, |_, _, cx| cx.notify()).detach();
        // LINK A MAP opens from the page or the map's toolbar: it is drawn over both.
        let linker = map.read(cx).linker().clone();
        cx.observe(&linker, |_, _, cx| cx.notify()).detach();
        Self { page, map }
    }

    /// The map over the dimmed page; a click beside it closes it.
    fn modal(&self) -> impl IntoElement {
        let map = self.map.clone();
        backdrop("map-backdrop").p(px(MAP_INSET)).on_click(move |_, _, cx| map.update(cx, |_, cx| cx.emit(MapEvent::Close))).child(
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
        let modal = self.page.read(cx).map_modal(cx);
        div()
            .relative()
            .size_full()
            .font_family(ui_font(cx))
            .text_size(px(TYPE_LEAD))
            .text_color(rgb(TEXT))
            .child(self.page.clone().cached(StyleRefinement::default().size_full()))
            .when(modal, |root| root.child(self.modal()))
            .children(Some(self.map.read(cx).linker().clone()).filter(|linker| linker.read(cx).is_open()))
    }
}
