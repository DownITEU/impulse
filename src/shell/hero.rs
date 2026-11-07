use gpui_kit::{
    Context, Div, FontWeight, IntoElement, ObjectFit, Stateful, component::h_flex, div, img,
    linear_color_stop, linear_gradient, prelude::*, px, relative, rgb,
};

use super::{
    Page,
    art::steam_art,
    assets::Icon,
    mock::{Game, Trainer},
    theme::*,
    widgets::{icon, mono, square, tracked},
};

pub(super) const OVERLAYS: [&str; 2] = ["STICKY", "IN-GAME"];

impl Page {
    pub(super) fn hero(&self, game: &Game, cx: &mut Context<Self>) -> impl IntoElement {
        let has_trainer = game.trainer != Trainer::None;
        let cover = || div().absolute().top_0().left_0().size_full();
        div()
            .relative()
            .min_h(px(230.))
            .flex_none()
            .flex()
            .flex_col()
            .justify_between()
            .gap(px(20.))
            .px(px(24.))
            .pt(px(16.))
            .pb(px(22.))
            .overflow_hidden()
            .border_b_1()
            .border_color(line(0.07))
            .child(
                img(steam_art(game.steam_app, "library_hero.jpg"))
                    .absolute()
                    .top_0()
                    .left_0()
                    .size_full()
                    .object_fit(ObjectFit::Cover)
                    .grayscale(!has_trainer),
            )
            .child(cover().bg(linear_gradient(
                90.,
                linear_color_stop(alpha(BG, 0.94), 0.),
                linear_color_stop(alpha(BG, 0.15), 0.8),
            )))
            .child(cover().bg(linear_gradient(
                180.,
                linear_color_stop(alpha(BG, 0.), 0.4),
                linear_color_stop(rgb(BG), 1.),
            )))
            .child(self.actions(game, cx))
            .child(
                div()
                    .relative()
                    .flex()
                    .flex_col()
                    .gap(px(10.))
                    .child(
                        tracked(&game.name.to_uppercase(), 56., 0.04)
                            .font_weight(FontWeight::BOLD)
                            .line_height(relative(0.95))
                            .text_color(rgb(if has_trainer { TEXT_BRIGHT } else { MUTED })),
                    )
                    .child(self.meta(game)),
            )
    }

    fn actions(&self, game: &Game, cx: &mut Context<Self>) -> impl IntoElement {
        let accent = rgb(self.accent);
        let index = self.selected;
        let running = self.running == Some(index);
        let favorite = self.favorites.contains(&index);
        h_flex()
            .relative()
            .justify_end()
            .gap(px(8.))
            .when(running, |bar| {
                bar.child(
                    action("stop")
                        .px(px(16.))
                        .border_color(accent)
                        .text_color(accent)
                        .on_click(cx.listener(|page, _, _, cx| {
                            page.running = None;
                            cx.notify();
                        }))
                        .child(square(9., accent))
                        .child(tracked("STOP", 12.5, 0.14)),
                )
                .child(
                    self.dropdown(
                        action("overlay")
                            .pl(px(16.))
                            .pr(px(12.))
                            .child(tracked("OVERLAY", 12.5, 0.14))
                            .child(tracked(self.overlay, 12.5, 0.14).text_color(rgb(FAINT)))
                            .child(icon(Icon::ChevronDown, 12., rgb(TEXT))),
                        "overlay",
                        &OVERLAYS,
                        self.overlay,
                        cx,
                        |page, mode| page.overlay = mode,
                    ),
                )
            })
            .when(!running && game.trainer != Trainer::None, |bar| {
                bar.child(
                    h_flex()
                        .id("launch")
                        .occlude()
                        .h(px(38.))
                        .pl(px(16.))
                        .pr(px(10.))
                        .gap(px(12.))
                        .bg(accent)
                        .text_color(rgb(BG))
                        .font_weight(FontWeight::BOLD)
                        .cursor_pointer()
                        .on_click(cx.listener(move |page, _, _, cx| {
                            page.running = Some(index);
                            cx.notify();
                        }))
                        .child(icon(Icon::Play, 13., rgb(BG)))
                        .child(tracked("LAUNCH + ATTACH", 12.5, 0.14))
                        .child(
                            mono("ENTER", 10.)
                                .font_weight(FontWeight::BOLD)
                                .px(px(6.))
                                .py(px(2.))
                                .border_1()
                                .border_color(alpha(BG, 0.45)),
                        ),
                )
            })
            .child(
                action("favorite")
                    .w(px(38.))
                    .justify_center()
                    .on_click(cx.listener(move |page, _, _, cx| {
                        if !page.favorites.remove(&index) {
                            page.favorites.insert(index);
                        }
                        cx.notify();
                    }))
                    .child(match favorite {
                        true => icon(Icon::StarFill, 16., accent),
                        false => icon(Icon::Star, 16., rgb(MUTED)),
                    }),
            )
    }

    fn meta(&self, game: &Game) -> impl IntoElement {
        let slash = || mono("/", 12.).text_color(rgb(GHOST));
        let mods = match game.trainer {
            Trainer::None => "NO TRAINER".to_owned(),
            _ => format!("{} MODS", game.mods),
        };
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
                    .child(mono(format!("V{}", game.build), 12.))
                    .when(game.trainer == Trainer::Ready, |version| {
                        version.child(icon(Icon::Check, 13., rgb(self.accent)))
                    })
                    .when(game.trainer == Trainer::UpdatePending, |version| {
                        version.child(mono("UPDATE PENDING", 12.).text_color(rgb(WARN)))
                    }),
            )
            .child(slash())
            .child(mono(format!("{} PLAYED", game.played), 12.))
            .child(slash())
            .child(mono(format!("{} SESSIONS", game.sessions), 12.))
    }
}

fn action(id: &'static str) -> Stateful<Div> {
    h_flex()
        .id(id)
        .occlude()
        .h(px(38.))
        .gap(px(10.))
        .border_1()
        .border_color(line(0.2))
        .bg(alpha(BG, 0.75))
        .font_weight(FontWeight::BOLD)
        .cursor_pointer()
        .hover(|style| style.bg(rgb(RAISED_HOVER)))
}
