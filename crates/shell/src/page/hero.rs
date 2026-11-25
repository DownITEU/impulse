//! The open game's hero: its art and title, the run's button and the game's actions.

use client::library::{Game, Support};
use gpui_kit::{
    Context, Div, Entity, IntoElement, ObjectFit, Render, SharedString, Stateful, Window, component::h_flex, div, img, linear_color_stop,
    linear_gradient, prelude::*, px, rgb,
};
use i18n::Arg;

use crate::{
    library::{GamePicker, Library, pick_source, source_options},
    overlay::{Layer, overlay_hotkey},
    run::{Ask, Runs, run_button},
    trainer::{Trainers, title},
    ui::{
        assets::Icon,
        dropdown::dropdown,
        key,
        text::{t, tf},
        theme::*,
        widgets::{action, icon, primary_action, tip, tracked},
    },
};

pub struct Hero {
    library: Entity<Library>,
    trainers: Entity<Trainers>,
    runs: Entity<Runs>,
    ask: Entity<Ask>,
    layer: Entity<Layer>,
    picker: Entity<GamePicker>,
    /// The sources menu is open.
    menu: Option<()>,
}

impl Hero {
    pub fn new(
        library: Entity<Library>,
        trainers: Entity<Trainers>,
        runs: Entity<Runs>,
        ask: Entity<Ask>,
        layer: Entity<Layer>,
        picker: Entity<GamePicker>,
        cx: &mut Context<Self>,
    ) -> Self {
        cx.observe(&library, |_, _, cx| cx.notify()).detach();
        cx.observe(&trainers, |_, _, cx| cx.notify()).detach();
        cx.observe(&runs, |_, _, cx| cx.notify()).detach();
        cx.observe(&layer, |_, _, cx| cx.notify()).detach();
        Self { library, trainers, runs, ask, layer, picker, menu: None }
    }

    pub fn close(&mut self, cx: &mut Context<Self>) {
        self.menu = None;
        cx.notify();
    }

    /// ADD GAME: GET ON each store of the title, then the executable by hand, as the catalog record open.
    fn add_game(&self, game: &Game, cx: &Context<Self>) -> Stateful<Div> {
        let get = game.stores.iter().map(|(store, _)| tf(cx, key::PAGE_GET_ON, &[("store", Arg::Str(store))]));
        let options = get.chain([t(cx, key::PAGE_ADD_MANUALLY)]).collect();
        let trigger = primary_action("add-game", cx).px(px(16.)).gap(px(10.)).child(icon(Icon::Plus, 13., rgb(BG))).child(tracked(
            &t(cx, key::PAGE_ADD_GAME),
            TYPE_FIELD,
            0.14,
        ));
        dropdown(trigger, (), open_menu, self.menu.is_some(), options, usize::MAX, px(220.), cx, |hero, at, window, cx| {
            let Some(game) = hero.library.read(cx).opened().cloned() else { return };
            match game.stores.get(at) {
                Some((_, link)) => cx.open_url(link),
                None => hero.picker.update(cx, |picker, cx| picker.add_to(game.source.id.clone(), window, cx)),
            }
        })
    }

    /// A catalog game has ADD GAME and the star only: no run, folder or source.
    fn actions(&self, game: &Game, cx: &Context<Self>) -> Div {
        let accent = rgb(accent(cx));
        let running = self.runs.read(cx).is_live(&game.id);
        let favorite = self.trainers.read(cx).game_profile(&game.id).favorite;
        let id = game.id.clone();
        let star = icon_button("favorite", t(cx, key::PAGE_STAR))
            .on_click(cx.listener(move |hero, _, _, cx| {
                hero.trainers.update(cx, |trainers, cx| trainers.toggle_favorite(&id, cx));
            }))
            .child(match favorite {
                true => icon(Icon::StarFill, 16., accent),
                false => icon(Icon::Star, 16., rgb(MUTED)),
            });
        let bar = h_flex().relative().justify_end().gap(px(8.));
        let Some(location) = game.install.as_ref().map(|install| install.location.clone()) else {
            return bar.child(self.add_game(game, cx)).child(star);
        };
        let layer = self.layer.clone();
        bar.child(run_button(&self.runs, &self.ask, game, cx))
            // Over the running game; the mode and the hotkey are in Settings.
            .when(running, |bar| {
                bar.child(
                    action("overlay")
                        .px(px(16.))
                        .on_click(move |_, _, cx| layer.read(cx).toggle())
                        .child(icon(Icon::Layers, 14., rgb(TEXT)))
                        .child(tracked(&t(cx, key::PAGE_OVERLAY), TYPE_FIELD, 0.14))
                        .child(tracked(&overlay_hotkey(self.layer.read(cx)), TYPE_FIELD, 0.14).text_color(rgb(FAINT))),
                )
            })
            .child(star)
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
                bar.child(dropdown(more, (), open_menu, self.menu.is_some(), options, current, px(200.), cx, |hero, at, window, cx| {
                    if let Some(game) = hero.library.read(cx).opened().cloned() {
                        pick_source(&hero.library, &hero.picker, &game, at, window, cx);
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
            .child(title(self.trainers.read(cx), &game, TYPE_HERO, cx).relative())
    }
}

fn open_menu(hero: &mut Hero) -> &mut Option<()> {
    &mut hero.menu
}

/// A 38 px square button that names itself in a tip.
fn icon_button(id: &'static str, text: SharedString) -> Stateful<Div> {
    action(id).w(px(38.)).justify_center().tooltip(move |window, cx| tip(text.clone(), window, cx))
}
