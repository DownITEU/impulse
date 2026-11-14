//! `+ ADD`: a game by its executable. The catalog games that keep their `exe` beside it are offered;
//! one is added at once, several or none open the picker, where the user finds it by name.

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
use crate::{Page, dialog::dialog, library::Found, theme::*, widgets::mono};

/// Rows shown at once: the filter narrows the rest.
const SHOWN: usize = 50;

pub struct Picker {
    found: Found,
    /// Indices into the catalog: those the executable belongs to, else every game with a trainer.
    choices: Vec<usize>,
    owned: bool,
    filter: Entity<InputState>,
    _typed: Subscription,
}

impl Page {
    pub(crate) fn add_game(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let (paths, library) = (prompt(cx), self.library.clone());
        cx.spawn_in(window, async move |page, cx| {
            let Some(exe) = picked_file(paths).await else { return };
            let Some(found) = library.update(cx, |library, cx| library.find(exe, cx)).await else { return };
            page.update_in(cx, |page, window, cx| page.offer(found, window, cx)).ok();
        })
        .detach();
    }

    fn offer(&mut self, found: Found, window: &mut Window, cx: &mut Context<Self>) {
        if let [at] = found.owners[..] {
            return self.add(found, at, cx);
        }
        let owned = !found.owners.is_empty();
        let mut choices = match owned {
            true => found.owners.clone(),
            false => (0..found.games.len()).filter(|&at| found.games[at].trainer.is_some()).collect(),
        };
        choices.sort_by(|&a, &b| found.names[a].cmp(&found.names[b]));
        let filter = cx.new(|cx| InputState::new(window, cx).placeholder("game name"));
        let typed = cx.subscribe(&filter, |_, _, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Change) {
                cx.notify();
            }
        });
        filter.update(cx, |filter, cx| filter.focus(window, cx));
        self.picker = Some(Picker { found, choices, owned, filter, _typed: typed });
        cx.notify();
    }

    fn choose(&mut self, at: usize, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(picker) = self.picker.take() {
            self.add(picker.found, at, cx);
        }
    }

    fn add(&mut self, Found { exe, games, .. }: Found, at: usize, cx: &mut Context<Self>) {
        self.library.update(cx, |library, cx| library.add(exe, games, at, cx));
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
        let library = self.library.read(cx);
        let found = &picker.found;
        let rows = picker.choices.iter().filter(|&&at| found.names[at].contains(&query)).take(shown).map(|&at| {
            let game = &found.games[at];
            let mut store = game.store_ids.first().and_then(|id| id.split(':').next()).unwrap_or_default().to_uppercase();
            // Picking it adds a source to the listed game.
            if library.game(&game.id).is_some() {
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
            .child(mono(found.exe.display().to_string(), 11.).text_color(rgb(MUTED)).overflow_hidden().text_ellipsis())
            .child(mono(caption, 10.5).text_color(rgb(DIM)))
            .child(field)
            .child(div().id("choices").max_h(px(320.)).overflow_y_scroll().flex().flex_col().children(rows));
        Some(dialog("picker", "ADD A GAME", 520., &self.dialog_focus, Self::close_picker, body, cx))
    }
}
