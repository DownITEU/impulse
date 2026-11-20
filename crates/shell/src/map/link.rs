//! LINK A MAP: which source title the open game is, for its maps. Names only order the list; the user picks.

use std::sync::Arc;

use client::library::{Game, GameId, titles::likeness};
use gpui_kit::{
    Context, Entity, FocusHandle, IntoElement, Render, Subscription, Window,
    component::{
        h_flex,
        input::{Input, InputState},
        v_flex,
    },
    div,
    prelude::*,
    px, rgb,
};
use model::Ref;

use super::{
    Maps,
    titles::{Titles, TitlesEvent},
};
use crate::{
    library::Library,
    ui::{
        dialog::dialog,
        key,
        text::{t, text},
        theme::*,
        widgets::{filter_input, mono},
    },
};

/// Rows shown at once: the filter narrows the rest.
const SHOWN: usize = 50;

struct Open {
    game: GameId,
    name: Arc<str>,
    /// Indices into the titles, likest first, and each title's name in lower case for the filter.
    order: Vec<usize>,
    names: Vec<String>,
    filter: Entity<InputState>,
    _typed: Subscription,
}

pub struct TitlePicker {
    titles: Entity<Titles>,
    library: Entity<Library>,
    open: Option<Open>,
    focus: FocusHandle,
}

impl TitlePicker {
    pub fn new(maps: Entity<Maps>, library: Entity<Library>, cx: &mut Context<Self>) -> Self {
        let titles = maps.read(cx).titles().clone();
        cx.subscribe(&titles, |picker, _, _: &TitlesEvent, cx| picker.order(cx)).detach();
        Self { titles, library, open: None, focus: cx.focus_handle() }
    }

    pub fn is_open(&self) -> bool {
        self.open.is_some()
    }

    /// Opens for `game`; the sources are asked for their titles again.
    pub fn open(&mut self, game: &Game, window: &mut Window, cx: &mut Context<Self>) {
        let (filter, typed) = filter_input(key::PICKER_FILTER, window, cx);
        filter.update(cx, |filter, cx| filter.focus(window, cx));
        let (game, name) = (game.id.clone(), game.name.clone());
        self.open = Some(Open { game, name, order: Vec::new(), names: Vec::new(), filter, _typed: typed });
        self.order(cx);
        self.titles.update(cx, |titles, cx| titles.refresh(cx));
    }

    fn order(&mut self, cx: &mut Context<Self>) {
        let Some(open) = &mut self.open else { return };
        let all = self.titles.read(cx).all();
        open.order = likeness(&open.name, all);
        open.names = all.iter().map(|title| title.name.to_lowercase()).collect();
        cx.notify();
    }

    fn choose(&mut self, title: &Ref, cx: &mut Context<Self>) {
        let Some(open) = self.open.take() else { return };
        self.library.update(cx, |library, cx| library.set_link(&open.game, title, cx));
        cx.notify();
    }

    fn close(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        if self.open.take().is_some() {
            cx.notify();
        }
    }
}

impl Render for TitlePicker {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(open) = self.open.as_ref() else { return div().into_any_element() };
        let query = open.filter.read(cx).value().trim().to_lowercase();
        let titles_of = self.titles.read(cx);
        let titles = titles_of.all();
        let rows: Vec<_> = open
            .order
            .iter()
            .filter(|&&at| open.names.get(at).is_some_and(|name| name.contains(&query)))
            .filter_map(|&at| titles.get(at).map(|title| (at, title)))
            .take(SHOWN)
            .map(|(at, title)| {
                let store = title.stores.first().map_or(String::new(), |id| id.store().to_uppercase());
                let source = titles_of.name(title.id.source()).map(|name| text(cx, name).to_uppercase());
                let id = title.id.clone();
                h_flex()
                    .id(("title", at))
                    .h(px(32.))
                    .flex_none()
                    .px(px(10.))
                    .gap(px(12.))
                    .justify_between()
                    .cursor_pointer()
                    .hover(|style| style.bg(rgb(ROW_HOVER)))
                    .on_click(cx.listener(move |view, _, _, cx| view.choose(&id, cx)))
                    .child(mono(title.name.to_uppercase(), 12.).text_color(rgb(TEXT_2)).overflow_hidden().text_ellipsis())
                    .child(
                        h_flex()
                            .flex_none()
                            .gap(px(8.))
                            .child(mono(store, 10.5).text_color(rgb(FAINT)))
                            .children(source.map(|source| mono(source, 10.5).text_color(rgb(MUTED)))),
                    )
            })
            .collect();
        let field = h_flex()
            .h(px(36.))
            .px(px(10.))
            .border_1()
            .border_color(line(0.14))
            .bg(rgb(SUNKEN))
            .child(Input::new(&open.filter).appearance(false).px_0().font_family(MONO).text_size(px(12.5)));
        let list = match rows.is_empty() {
            true => div().py(px(8.)).child(mono(t(cx, key::MAP_LINK_EMPTY), 11.).text_color(rgb(FAINT))).into_any_element(),
            false => div().id("titles").max_h(px(320.)).overflow_y_scroll().flex().flex_col().children(rows).into_any_element(),
        };
        let body = v_flex()
            .p(px(16.))
            .gap(px(12.))
            .child(mono(open.name.clone(), 11.).text_color(rgb(MUTED)).overflow_hidden().text_ellipsis())
            .child(mono(t(cx, key::MAP_LINK_CAPTION), 10.5).text_color(rgb(DIM)))
            .child(field)
            .child(list);
        dialog("link", t(cx, key::MAP_LINK_TITLE), 520., &self.focus, Self::close, body, cx).into_any_element()
    }
}
