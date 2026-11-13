use std::time::Duration;

use gpui_kit::{
    Animation, AnimationExt, App, Context, FontWeight, IntoElement, ObjectFit,
    component::{h_flex, scroll::ScrollableElement, v_flex},
    div, img,
    prelude::*,
    px, rgb,
};

use super::{
    Page, SIDEBAR_WIDTH,
    assets::Icon,
    library::{Game, Support},
    settings::Lead,
    theme::*,
    widgets::{filter, icon, icon_button, mono, square},
};

impl Page {
    pub(super) fn sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let query = self.game_filter.read(cx).value().to_lowercase();
        let games = self.games();
        // Favorites first; the numbers follow that order and stay through the filter.
        let (starred, rest): (Vec<_>, Vec<_>) = games.iter().enumerate().partition(|(_, game)| self.profiles.of(&game.id).favorite);
        let sectioned = !starred.is_empty();
        let mut number = 0;
        let mut rows = Vec::new();
        for (title, section) in [("FAVORITES", starred), ("GAMES", rest)] {
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
            rows.push(mono("NO GAMES MATCH", 11.).px(px(16.)).py(px(10.)).text_color(rgb(FAINT)).into_any_element());
        }
        v_flex()
            .w(px(SIDEBAR_WIDTH))
            .flex_none()
            .h_full()
            .border_r_1()
            .border_color(line(0.09))
            .bg(alpha(BG, 0.82))
            .child(
                h_flex()
                    .h(px(44.))
                    .flex_none()
                    .pl(px(16.))
                    .pr(px(10.))
                    .justify_between()
                    .border_b_1()
                    .border_color(line(0.07))
                    .child(
                        h_flex()
                            .gap(px(7.))
                            .child(mono("GAMES", 11.).text_color(rgb(DIM)))
                            .child(mono("/", 11.).text_color(rgb(GHOST)))
                            .child(mono(games.len().to_string(), 11.).text_color(rgb(TEXT_2))),
                    )
                    .child(icon_button("sort", Icon::Sort, 30., 15.)),
            )
            .child(
                filter(&self.game_filter, cx)
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
                    .on_click(cx.listener(|page, _, window, cx| page.add_game(window, cx)))
                    .child(mono("+ ADD", 11.)),
            )
    }

    fn game_row(&self, index: usize, number: usize, game: &Game, cx: &mut Context<Self>) -> impl IntoElement {
        let accent = rgb(accent(cx));
        let current = Some(&game.id) == self.selected.as_ref();
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
                move |page, _, window, cx| page.select(id.clone(), window, cx)
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
                    .child(game.name.clone()),
            )
            .when(Some(&game.id) == self.play.live(), |row| row.child(self.live(cx)))
            .when(game.support == Support::UpdatePending, |row| row.child(icon(Icon::Warn, 14., rgb(WARN))))
    }

    fn live(&self, cx: &App) -> impl IntoElement {
        let label = mono("LIVE", 10.).font_weight(FontWeight::BOLD).text_color(rgb(accent(cx)));
        label
            .with_animation("live", Animation::new(Duration::from_millis(1200)).repeat(), |label, t| {
                label.opacity(if t < 0.5 { 1. } else { 0.35 })
            })
            .into_any_element()
    }
}

fn section_title(title: &'static str, count: usize) -> impl IntoElement {
    h_flex()
        .h(px(26.))
        .flex_none()
        .px(px(16.))
        .gap(px(7.))
        .child(mono(title, 10.).text_color(rgb(DIM)))
        .child(mono(format!("{count:02}"), 10.).text_color(rgb(GHOST)))
}
