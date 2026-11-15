//! The open game's hero: its art and title, the run's button and the game's actions.

use client::library::{Game, Support};
use gpui_kit::{
    Context, Div, Entity, IntoElement, ObjectFit, Render, SharedString, Stateful, Window, component::h_flex, div, img, linear_color_stop,
    linear_gradient, prelude::*, px, rgb,
};
use i18n::Arg;

use crate::{
    layer::{Layer, overlay_hotkey},
    library::{Library, pick_source, source_options},
    run::{Runs, run_button},
    trainer::{Trainers, title},
    ui::{
        assets::Icon,
        dropdown::dropdown,
        key,
        text::{t, tf},
        theme::*,
        widgets::{action, icon, tip, tracked},
    },
};

pub struct Hero {
    library: Entity<Library>,
    trainers: Entity<Trainers>,
    runs: Entity<Runs>,
    layer: Entity<Layer>,
    /// The sources menu is open.
    menu: Option<()>,
}

impl Hero {
    pub fn new(
        library: Entity<Library>,
        trainers: Entity<Trainers>,
        runs: Entity<Runs>,
        layer: Entity<Layer>,
        cx: &mut Context<Self>,
    ) -> Self {
        cx.observe(&library, |_, _, cx| cx.notify()).detach();
        cx.observe(&trainers, |_, _, cx| cx.notify()).detach();
        cx.observe(&runs, |_, _, cx| cx.notify()).detach();
        cx.observe(&layer, |_, _, cx| cx.notify()).detach();
        Self { library, trainers, runs, layer, menu: None }
    }

    pub fn close(&mut self, cx: &mut Context<Self>) {
        self.menu = None;
        cx.notify();
    }

    fn actions(&self, game: &Game, cx: &Context<Self>) -> impl IntoElement {
        let accent = rgb(accent(cx));
        let running = self.runs.read(cx).is_live(&game.id);
        let favorite = self.trainers.read(cx).game_profile(&game.id).favorite;
        let (id, location) = (game.id.clone(), game.install.location.clone());
        let layer = self.layer.clone();
        h_flex()
            .relative()
            .justify_end()
            .gap(px(8.))
            .child(run_button(&self.runs, game, cx))
            // Over the running game; the mode and the hotkey are in Settings.
            .when(running, |bar| {
                bar.child(
                    action("overlay")
                        .px(px(16.))
                        .on_click(move |_, _, cx| layer.read(cx).toggle())
                        .child(icon(Icon::Layers, 14., rgb(TEXT)))
                        .child(tracked(&t(cx, key::PAGE_OVERLAY), 12.5, 0.14))
                        .child(tracked(&overlay_hotkey(self.layer.read(cx)), 12.5, 0.14).text_color(rgb(FAINT))),
                )
            })
            .child(
                icon_button("favorite", t(cx, key::PAGE_STAR))
                    .on_click(cx.listener(move |hero, _, _, cx| {
                        hero.trainers.update(cx, |trainers, cx| trainers.toggle_favorite(&id, cx));
                    }))
                    .child(match favorite {
                        true => icon(Icon::StarFill, 16., accent),
                        false => icon(Icon::Star, 16., rgb(MUTED)),
                    }),
            )
            .child(icon_button("folder", t(cx, key::PAGE_FOLDER)).on_click(move |_, _, cx| cx.open_with_system(&location)).child(icon(
                Icon::Folder,
                15.,
                rgb(MUTED),
            )))
            .when_some(game.page.clone(), |bar, (launcher, uri)| {
                bar.child(
                    icon_button("launcher", tf(cx, key::PAGE_LAUNCHER, &[("launcher", Arg::Str(launcher))]))
                        .on_click(move |_, _, cx| cx.open_url(&uri))
                        .child(icon(Icon::External, 15., rgb(MUTED))),
                )
            })
            // A run holds its install.
            .when(!self.runs.read(cx).holds(&game.id), |bar| {
                let (options, current) = source_options(game, cx);
                let more = icon_button("source", t(cx, key::PAGE_SOURCE)).child(icon(Icon::More, 15., rgb(MUTED)));
                let place = |list: Div| list.right(px(-1.)).min_w(px(200.));
                bar.child(dropdown(more, (), open_menu, self.menu.is_some(), options, current, place, cx, |hero, at, _, cx| {
                    if let Some(game) = hero.library.read(cx).opened().cloned() {
                        pick_source(&hero.library, &game, at, cx);
                    }
                }))
            })
    }
}

impl Render for Hero {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(game) = self.library.read(cx).opened().cloned() else { return div() };
        let has_trainer = game.support != Support::NoTrainer;
        let cover = || div().absolute().top_0().left_0().size_full();
        let art = move |url: String| img(url).size_full().object_fit(ObjectFit::Cover).grayscale(!has_trainer);
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
                hero.child(cover().child(art(url).with_fallback(move || match spare.clone() {
                    Some(url) => art(url).into_any_element(),
                    None => div().into_any_element(),
                })))
            })
            .child(cover().bg(linear_gradient(90., linear_color_stop(alpha(BG, 0.94), 0.), linear_color_stop(alpha(BG, 0.15), 0.8))))
            .child(cover().bg(linear_gradient(180., linear_color_stop(alpha(BG, 0.), 0.4), linear_color_stop(rgb(BG), 1.))))
            .child(self.actions(&game, cx))
            .child(title(self.trainers.read(cx), &game, 56., cx).relative())
    }
}

fn open_menu(hero: &mut Hero) -> &mut Option<()> {
    &mut hero.menu
}

/// A 38 px square button that names itself in a tip.
fn icon_button(id: &'static str, text: SharedString) -> Stateful<Div> {
    action(id).w(px(38.)).justify_center().tooltip(move |window, cx| tip(text.clone(), window, cx))
}
