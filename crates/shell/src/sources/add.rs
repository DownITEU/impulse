//! `+ ADD`: a game by its executable. The catalog games that keep their `exe` beside it are offered;
//! one is added at once, several or none open the picker, where the user finds it by name.

use std::{collections::HashSet, path::PathBuf, sync::Arc};

use blocking::unblock;
use gpui_kit::{
    Context, Entity, IntoElement, Subscription, Window,
    component::{
        h_flex,
        input::{Input, InputEvent, InputState},
        v_flex,
    },
    div,
    prelude::*,
    px, rgb,
};

use super::{picked_file, prompt};
use crate::{
    Page,
    boot::{catalog, rescan},
    theme::*,
    widgets::mono,
};

/// Rows shown at once: the filter narrows the rest.
const SHOWN: usize = 50;

pub struct Picker {
    exe: PathBuf,
    /// The catalog while the picker is open: the library is scanned again with it.
    games: Vec<model::Game>,
    /// Lower case, for the filter.
    names: Vec<String>,
    /// Indices into `games`: those the executable belongs to, else every game with a trainer.
    choices: Vec<usize>,
    owned: bool,
    filter: Entity<InputState>,
    _typed: Subscription,
}

impl Page {
    pub(crate) fn add_game(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let (paths, provider) = (prompt(cx), self.provider.clone());
        cx.spawn_in(window, async move |page, cx| {
            let Some(exe) = picked_file(paths).await else { return };
            let Some(games) = catalog(provider.as_ref()).await else {
                log::warn!("no catalog to add {} from", exe.display());
                return;
            };
            let found = unblock(move || {
                let owners: HashSet<&str> = launchers::owners(&games, &exe).into_iter().map(|(game, _)| game.id.as_str()).collect();
                let owners = (0..games.len()).filter(|&at| owners.contains(games[at].id.as_str())).collect();
                let names = games.iter().map(|game| game.name.to_lowercase()).collect();
                (exe, games, names, owners)
            })
            .await;
            page.update_in(cx, |page, window, cx| page.offer(found, window, cx)).ok();
        })
        .detach();
    }

    fn offer(
        &mut self,
        (exe, games, names, owners): (PathBuf, Vec<model::Game>, Vec<String>, Vec<usize>),
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let [at] = owners[..] {
            return self.add(exe, games, at, window, cx);
        }
        let owned = !owners.is_empty();
        let mut choices = if owned { owners } else { (0..games.len()).filter(|&at| games[at].trainer.is_some()).collect() };
        choices.sort_by(|&a, &b| names[a].cmp(&names[b]));
        let filter = cx.new(|cx| InputState::new(window, cx).placeholder("game name"));
        let typed = cx.subscribe(&filter, |_, _, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Change) {
                cx.notify();
            }
        });
        filter.update(cx, |filter, cx| filter.focus(window, cx));
        self.picker = Some(Picker { exe, games, names, choices, owned, filter, _typed: typed });
        cx.notify();
    }

    fn choose(&mut self, at: usize, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(Picker { exe, games, .. }) = self.picker.take() {
            self.add(exe, games, at, window, cx);
        }
    }

    /// Adds `games[at]` with its executable; a game already listed takes it as its source.
    fn add(&mut self, exe: PathBuf, games: Vec<model::Game>, at: usize, window: &mut Window, cx: &mut Context<Self>) {
        let id: Arc<str> = games[at].id.as_str().into();
        if self.games().iter().any(|game| game.id == id) {
            self.use_exe(id.clone(), exe, window, cx);
            self.select(id, window, cx);
            return;
        }
        self.picked.set(&id, Some(exe));
        cx.spawn_in(window, async move |page, cx| {
            rescan(&page, games, cx).await;
            page.update_in(cx, |page, window, cx| {
                // Gone if the file went meanwhile.
                if page.games().iter().any(|game| game.id == id) {
                    page.select(id, window, cx);
                }
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    pub(crate) fn close_picker(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        if self.picker.take().is_some() {
            cx.notify();
        }
    }

    pub(crate) fn game_picker(&self, cx: &mut Context<Self>) -> Option<impl IntoElement> {
        let picker = self.picker.as_ref()?;
        let query = picker.filter.read(cx).value().trim().to_lowercase();
        // The whole catalog waits for a name.
        let shown = if picker.owned || !query.is_empty() { SHOWN } else { 0 };
        let rows = picker.choices.iter().filter(|&&at| picker.names[at].contains(&query)).take(shown).map(|&at| {
            let game = &picker.games[at];
            let mut store = game.store_ids.first().and_then(|id| id.split(':').next()).unwrap_or_default().to_uppercase();
            // Picking it adds a source to the listed game.
            if self.games().iter().any(|listed| *listed.id == *game.id) {
                store.push_str("  IN LIBRARY");
            }
            h_flex()
                .id(("choice", at))
                .h(px(32.))
                .flex_none()
                .px(px(10.))
                .gap(px(12.))
                .justify_between()
                .cursor_pointer()
                .hover(|style| style.bg(rgb(ROW_HOVER)))
                .on_click(cx.listener(move |page, _, window, cx| page.choose(at, window, cx)))
                .child(mono(game.name.to_uppercase(), 12.).text_color(rgb(TEXT_2)).overflow_hidden().text_ellipsis())
                .child(mono(store, 10.5).text_color(rgb(FAINT)))
        });
        let caption = match picker.owned {
            true => "MORE THAN ONE GAME MATCHES: WHICH ONE IS IT?",
            false => "NO CATALOG GAME MATCHES: FIND IT BY NAME",
        };
        let field = h_flex()
            .h(px(36.))
            .px(px(10.))
            .border_1()
            .border_color(line(0.14))
            .bg(rgb(SUNKEN))
            .child(Input::new(&picker.filter).appearance(false).px_0().font_family(MONO).text_size(px(12.5)));
        let body = v_flex()
            .p(px(16.))
            .gap(px(12.))
            .child(mono(picker.exe.display().to_string(), 11.).text_color(rgb(MUTED)).overflow_hidden().text_ellipsis())
            .child(mono(caption, 10.5).text_color(rgb(DIM)))
            .child(field)
            .child(div().id("choices").max_h(px(320.)).overflow_y_scroll().flex().flex_col().children(rows));
        Some(self.dialog("picker", "ADD A GAME", 520., Self::close_picker, body, cx))
    }
}
