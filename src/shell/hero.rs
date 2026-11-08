use gpui_kit::{
    Context, Div, FontWeight, IntoElement, ObjectFit, SharedString, Stateful, component::h_flex,
    div, img, linear_color_stop, linear_gradient, prelude::*, px, relative, rgb,
};

use super::{
    Menu, Page,
    assets::Icon,
    library::{Game, Support},
    run::{Play, STOPPING, label},
    theme::*,
    widgets::{icon, mono, square, tracked},
};

pub(super) const OVERLAYS: [&str; 2] = ["STICKY", "IN-GAME"];

impl Page {
    pub(super) fn hero(&self, game: &Game, cx: &mut Context<Self>) -> impl IntoElement {
        let has_trainer = game.support != Support::NoTrainer;
        let cover = || div().absolute().top_0().left_0().size_full();
        let art = move |url: String| {
            img(url)
                .size_full()
                .object_fit(ObjectFit::Cover)
                .grayscale(!has_trainer)
        };
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
            .when_some(game.hero.clone(), |hero, url| {
                // Not every Steam app has hero art; the title thumbnail always exists.
                let spare = game.thumbnail.clone();
                hero.child(
                    cover().child(art(url).with_fallback(move || match spare.clone() {
                        Some(url) => art(url).into_any_element(),
                        None => div().into_any_element(),
                    })),
                )
            })
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
                        tracked(&game.name, 56., 0.04)
                            .font_weight(FontWeight::BOLD)
                            .line_height(relative(0.95))
                            .text_color(rgb(if has_trainer { TEXT_BRIGHT } else { MUTED })),
                    )
                    .child(self.meta(game)),
            )
    }

    fn actions(&self, game: &Game, cx: &mut Context<Self>) -> impl IntoElement {
        let accent = rgb(self.accent);
        let running = self.is_running();
        let starting = match &self.play {
            Play::Busy { game: busy, stage, .. } if *busy == game.id => Some(*stage),
            _ => None,
        };
        let stopping = matches!(&self.play, Play::Stopping { game: ending, .. } if *ending == game.id);
        let favorite = self.favorites.contains(&game.id);
        // Nothing to run before the cheat list is there, and one run at a time.
        let launchable = !self.play.is_active() && self.trainer(Some(&game.id)).is_some();
        let id = game.id.clone();
        let overlays: Vec<SharedString> = OVERLAYS.into_iter().map(SharedString::from).collect();
        h_flex()
            .relative()
            .justify_end()
            .gap(px(8.))
            .when(running || starting.is_some(), |bar| {
                bar.child(
                    action("stop")
                        .px(px(16.))
                        .border_color(accent)
                        .text_color(accent)
                        .on_click(cx.listener(|page, _, _, cx| page.stop_run(cx)))
                        .child(square(9., accent))
                        .child(tracked("STOP", 12.5, 0.14)),
                )
            })
            .when_some(starting, |bar, stage| {
                bar.child(tracked(label(stage), 12.5, 0.14).text_color(accent))
            })
            .when(stopping, |bar| {
                bar.child(tracked(STOPPING, 12.5, 0.14).text_color(accent))
            })
            .when(running, |bar| {
                bar.child(
                    self.dropdown(
                        action("overlay")
                            .pl(px(16.))
                            .pr(px(12.))
                            .child(tracked("OVERLAY", 12.5, 0.14))
                            .child(
                                tracked(OVERLAYS[self.overlay], 12.5, 0.14).text_color(rgb(FAINT)),
                            )
                            .child(icon(Icon::ChevronDown, 12., rgb(TEXT))),
                        Menu::Overlay,
                        overlays,
                        self.overlay,
                        cx,
                        |page, mode, _, _| page.overlay = mode,
                    ),
                )
            })
            .when(launchable, |bar| {
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
                        .on_click(cx.listener(|page, _, window, cx| page.start_run(window, cx)))
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
                        if !page.favorites.remove(&id) {
                            page.favorites.insert(id.clone());
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
        let mods = match game.support {
            Support::NoTrainer => "NO TRAINER".to_owned(),
            // The catalog count, until the trainer itself is there.
            _ => {
                let count = self
                    .trainer(Some(&game.id))
                    .map_or(game.mods as usize, |loaded| loaded.cheats.len());
                format!("{count} MODS")
            }
        };
        let version = match game.version {
            Some(version) => format!("V{version}"),
            None => "VERSION UNKNOWN".to_owned(),
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
                    .child(mono(version, 12.))
                    .when(game.support == Support::Ready, |version| {
                        version.child(icon(Icon::Check, 13., rgb(self.accent)))
                    })
                    .when(game.support == Support::UpdatePending, |version| {
                        version.child(mono("UPDATE PENDING", 12.).text_color(rgb(WARN)))
                    }),
            )
            .child(slash())
            .child(mono(game.store.clone(), 12.))
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
