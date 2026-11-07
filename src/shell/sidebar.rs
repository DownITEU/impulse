use std::time::Duration;

use gpui_kit::{
    Animation, AnimationExt, Context, FontWeight, IntoElement, ObjectFit,
    component::{h_flex, scroll::ScrollableElement, v_flex},
    div, img,
    prelude::*,
    px, rgb,
};

use super::{
    Lead, Page, SIDEBAR_WIDTH,
    art::steam_art,
    assets::Icon,
    mock::{GAMES, Game, Trainer},
    theme::*,
    widgets::{filter, icon, icon_button, mono, square},
};

impl Page {
    pub(super) fn sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let query = self.game_filter.read(cx).value().to_lowercase();
        let rows: Vec<_> = GAMES
            .iter()
            .enumerate()
            .filter(|(_, game)| game.name.to_lowercase().contains(&query))
            .map(|(index, game)| self.game_row(index, game, cx).into_any_element())
            .collect();
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
                            .child(mono(GAMES.len().to_string(), 11.).text_color(rgb(TEXT_2))),
                    )
                    .child(icon_button("sort", Icon::Sort, 30., 15.)),
            )
            .child(
                filter(&self.game_filter, self.accent)
                    .h(px(34.))
                    .flex_none()
                    .mx(px(12.))
                    .mt(px(10.))
                    .mb(px(6.))
                    .px(px(10.))
                    .border_1()
                    .border_color(line(0.1)),
            )
            .child(
                v_flex()
                    .id("games")
                    .flex_1()
                    .min_h_0()
                    .pt(px(4.))
                    .pb(px(12.))
                    .children(rows)
                    .overflow_y_scrollbar(),
            )
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
                    .child(mono("+ ADD", 11.)),
            )
    }

    fn game_row(&self, index: usize, game: &Game, cx: &mut Context<Self>) -> impl IntoElement {
        let accent = rgb(self.accent);
        let current = index == self.selected;
        let no_trainer = game.trainer == Trainer::None;
        let name_color = match (current, no_trainer) {
            (true, _) => accent,
            (false, true) => rgb(FAINT),
            (false, false) => rgb(TEXT_2),
        };
        let lead = match self.lead {
            Lead::Number => mono(format!("{:02}", index + 1), 11.)
                .w(px(22.))
                .flex_none()
                .text_color(if current { accent } else { rgb(FAINT) })
                .into_any_element(),
            Lead::Thumbnail => img(steam_art(game.steam_app, "library_600x900.jpg"))
                .size(px(22.))
                .flex_none()
                .object_fit(ObjectFit::Cover)
                .grayscale(no_trainer)
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
            .on_click(cx.listener(move |page, _, _, cx| {
                page.selected = index;
                cx.notify();
            }))
            .child(
                div()
                    .w(px(8.))
                    .flex_none()
                    .when(current, |cell| cell.child(square(6., accent))),
            )
            .child(lead)
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_size(px(13.))
                    .text_color(name_color)
                    .child(game.name.to_uppercase()),
            )
            .when(self.running == Some(index), |row| row.child(self.live()))
            .when(game.trainer == Trainer::UpdatePending, |row| {
                row.child(icon(Icon::Warn, 14., rgb(WARN)))
            })
    }

    fn live(&self) -> impl IntoElement {
        let label = mono("LIVE", 10.)
            .font_weight(FontWeight::BOLD)
            .text_color(rgb(self.accent));
        if self.reduce_motion {
            return label.into_any_element();
        }
        label
            .with_animation(
                "live",
                Animation::new(Duration::from_millis(1200)).repeat(),
                |label, t| label.opacity(if t < 0.5 { 1. } else { 0.35 }),
            )
            .into_any_element()
    }
}
