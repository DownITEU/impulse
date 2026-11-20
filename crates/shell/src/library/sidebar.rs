//! The game list at the left: favorites first, a filter, `+ ADD`; and the pane's notice before there is a game.

use std::time::Duration;

use client::{
    Load,
    library::{Game, GameId, Support},
};
use gpui_kit::{
    Animation, AnimationExt, App, Context, Entity, FontWeight, IntoElement, ObjectFit, Render, SharedString, Window,
    component::{h_flex, input::InputState, scroll::ScrollableElement, v_flex},
    div, img,
    prelude::*,
    px, rgb,
};

use super::{GamePicker, Library};
use crate::{
    settings::{Lead, Settings},
    ui::{
        assets::Icon,
        key,
        text::{failed, t},
        theme::*,
        widgets::{filter, filter_input, icon, mono, notice, square},
    },
};

pub const WIDTH: f32 = 248.;

/// What other slices mark on a game's row.
pub struct Marks {
    pub favorite: bool,
    pub live: bool,
}

/// Reads a game's marks.
type MarksOf = Box<dyn Fn(&GameId, &App) -> Marks>;

pub struct Sidebar {
    library: Entity<Library>,
    settings: Entity<Settings>,
    picker: Entity<GamePicker>,
    filter: Entity<InputState>,
    marks: MarksOf,
}

impl Sidebar {
    pub fn new(
        library: Entity<Library>,
        settings: Entity<Settings>,
        picker: Entity<GamePicker>,
        marks: impl Fn(&GameId, &App) -> Marks + 'static,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        cx.observe(&library, |_, _, cx| cx.notify()).detach();
        cx.observe(&settings, |_, _, cx| cx.notify()).detach();
        let (filter, typed) = filter_input(key::LIBRARY_FILTER, window, cx);
        typed.detach();
        Self { library, settings, picker, filter, marks: Box::new(marks) }
    }

    fn game_row(&self, index: usize, number: usize, game: &Game, cx: &mut Context<Self>) -> impl IntoElement {
        let accent = rgb(accent(cx));
        let current = Some(&game.id) == self.library.read(cx).open_id();
        let no_trainer = game.support == Support::NoTrainer;
        let name_color = match (current, no_trainer) {
            (true, _) => accent,
            (false, true) => rgb(FAINT),
            (false, false) => rgb(TEXT_2),
        };
        let lead = match self.settings.read(cx).prefs().lead {
            Lead::Number => mono(format!("{number:02}"), 11.)
                .w(px(22.))
                .flex_none()
                .text_color(if current { accent } else { rgb(FAINT) })
                .into_any_element(),
            Lead::Thumbnail => div()
                .size(px(22.))
                .flex_none()
                .bg(rgb(AVATAR))
                .when_some(game.thumbnail.clone(), |lead, url| {
                    lead.child(img(url).size_full().object_fit(ObjectFit::Cover).grayscale(no_trainer))
                })
                .into_any_element(),
        };
        h_flex()
            .id(("game", index))
            .h(px(36.))
            .flex_none()
            .pl(px(16.))
            .pr(px(14.))
            .gap(px(10.))
            .cursor_pointer()
            .map(|row| match current {
                true => row.bg(rgb(ROW_SELECTED)),
                false => row.hover(|style| style.bg(rgb(ROW_HOVER))),
            })
            .on_click(cx.listener({
                let id = game.id.clone();
                move |view, _, _, cx| view.library.update(cx, |library, cx| library.open(id.clone(), cx))
            }))
            .child(div().w(px(8.)).flex_none().when(current, |cell| cell.child(square(6., accent))))
            .child(lead)
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_size(px(13.))
                    .text_color(name_color)
                    .child(SharedString::from(&game.name)),
            )
            .when((self.marks)(&game.id, cx).live, |row| row.child(live_label(cx)))
            .when(game.support == Support::UpdatePending, |row| row.child(icon(Icon::Warn, 14., rgb(WARN))))
    }
}

impl Render for Sidebar {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let query = self.filter.read(cx).value().to_lowercase();
        let games = self.library.read(cx).games().to_vec();
        // Favorites first; the numbers follow that order and stay through the filter.
        let (starred, rest): (Vec<_>, Vec<_>) = games.iter().enumerate().partition(|(_, game)| (self.marks)(&game.id, cx).favorite);
        let sectioned = !starred.is_empty();
        let mut number = 0;
        let mut rows = Vec::new();
        for (title, section) in [(t(cx, key::LIBRARY_FAVORITES), starred), (t(cx, key::LIBRARY_GAMES), rest)] {
            let shown: Vec<_> = section
                .into_iter()
                .map(|found| {
                    number += 1;
                    (number, found)
                })
                .filter(|(_, (_, game))| game.search.contains(&query))
                .collect();
            if sectioned && !shown.is_empty() {
                rows.push(section_title(title, shown.len()).into_any_element());
            }
            rows.extend(shown.into_iter().map(|(number, (index, game))| self.game_row(index, number, game, cx).into_any_element()));
        }
        if rows.is_empty() && !games.is_empty() {
            rows.push(mono(t(cx, key::LIBRARY_NO_MATCH), 11.).px(px(16.)).py(px(10.)).text_color(rgb(FAINT)).into_any_element());
        }
        v_flex()
            .w(px(WIDTH))
            .flex_none()
            .h_full()
            .border_r_1()
            .border_color(line(0.09))
            .bg(alpha(BG, 0.82))
            .child(
                h_flex().h(px(44.)).flex_none().pl(px(16.)).pr(px(10.)).justify_between().border_b_1().border_color(line(0.07)).child(
                    h_flex()
                        .gap(px(7.))
                        .child(mono(t(cx, key::LIBRARY_GAMES), 11.).text_color(rgb(DIM)))
                        .child(mono("/", 11.).text_color(rgb(GHOST)))
                        .child(mono(games.len().to_string(), 11.).text_color(rgb(TEXT_2))),
                ), // Hidden until reordering works (needs `icon_button`): .child(icon_button("sort", Icon::Sort, 30., 15.))
            )
            .child(
                filter(&self.filter, cx)
                    .h(px(34.))
                    .flex_none()
                    .mx(px(12.))
                    .mt(px(10.))
                    .mb(px(6.))
                    .px(px(10.))
                    .border_1()
                    .border_color(line(0.1)),
            )
            .child(v_flex().id("games").flex_1().min_h_0().pt(px(4.)).pb(px(12.)).children(rows).overflow_y_scrollbar())
            .child(
                h_flex()
                    .id("add")
                    .h(px(44.))
                    .flex_none()
                    .px(px(16.))
                    .border_t_1()
                    .border_color(line(0.07))
                    .cursor_pointer()
                    .text_color(rgb(DIM))
                    .hover(|style| style.text_color(rgb(TEXT)))
                    .on_click(cx.listener(|view, _, window, cx| view.picker.update(cx, |picker, cx| picker.add(window, cx))))
                    .child(mono(t(cx, key::LIBRARY_ADD), 11.)),
            )
    }
}

fn live_label(cx: &App) -> impl IntoElement {
    let label = mono(t(cx, key::LIBRARY_LIVE), 10.).font_weight(FontWeight::BOLD).text_color(rgb(accent(cx)));
    label
        .with_animation("live", Animation::new(Duration::from_millis(1200)).repeat(), |label, t| {
            label.opacity(if t < 0.5 { 1. } else { 0.35 })
        })
        .into_any_element()
}

fn section_title(title: SharedString, count: usize) -> impl IntoElement {
    h_flex()
        .h(px(26.))
        .flex_none()
        .px(px(16.))
        .gap(px(7.))
        .child(mono(title, 10.).text_color(rgb(DIM)))
        .child(mono(format!("{count:02}"), 10.).text_color(rgb(GHOST)))
}

/// The pane before there is a game to open.
pub fn vacant(library: &Library, cx: &App) -> impl IntoElement {
    match library.list() {
        Load::Pending => notice(t(cx, key::LIBRARY_SCANNING_TAG), t(cx, key::LIBRARY_SCANNING), t(cx, key::LIBRARY_SCANNING_BODY)),
        Load::Failed(failure) => {
            notice(t(cx, key::CATALOG_FAILED_TAG), t(cx, key::CATALOG_FAILED), failed(cx, key::CATALOG_FAILED_BODY, failure))
        }
        Load::Ready(_) => notice(t(cx, key::LIBRARY_EMPTY_TAG), t(cx, key::LIBRARY_EMPTY), t(cx, key::LIBRARY_EMPTY_BODY)),
    }
}
