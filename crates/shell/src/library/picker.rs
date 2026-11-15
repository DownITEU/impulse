//! `+ ADD`: a game by its executable. The catalog games that keep their `exe` beside it are offered;
//! one is added at once, several or none open the picker, where the user finds it by name.

use std::path::PathBuf;

use anyhow::Result;
use futures::channel::oneshot;
use gpui_kit::{
    App, Context, Entity, FocusHandle, IntoElement, PathPromptOptions, Render, Subscription, Window,
    component::{
        h_flex,
        input::{Input, InputState},
        v_flex,
    },
    div,
    prelude::*,
    px, rgb,
};
use i18n::Arg;

use super::{Found, Library};
use crate::ui::{
    dialog::dialog,
    key,
    text::{t, tf},
    theme::*,
    widgets::{filter_input, mono},
};
use client::library::GameId;

/// Rows shown at once: the filter narrows the rest.
const SHOWN: usize = 50;

struct Picker {
    found: Found,
    /// Indices into the catalog: those the executable belongs to, else every game with a trainer.
    choices: Vec<usize>,
    owned: bool,
    filter: Entity<InputState>,
    _typed: Subscription,
}

pub struct GamePicker {
    library: Entity<Library>,
    picker: Option<Picker>,
    focus: FocusHandle,
}

impl GamePicker {
    pub fn new(library: Entity<Library>, cx: &mut Context<Self>) -> Self {
        Self { library, picker: None, focus: cx.focus_handle() }
    }

    pub fn is_open(&self) -> bool {
        self.picker.is_some()
    }

    /// Asks for the game's executable, then adds it or opens the picker.
    pub fn add(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let (paths, library) = (prompt(cx), self.library.clone());
        cx.spawn_in(window, async move |view, cx| {
            let Some(exe) = picked_file(paths).await else { return };
            let Some(found) = library.update(cx, |library, cx| library.find(exe, cx)).await else { return };
            view.update_in(cx, |view, window, cx| view.offer(found, window, cx)).ok();
        })
        .detach();
    }

    fn offer(&mut self, found: Found, window: &mut Window, cx: &mut Context<Self>) {
        if let [at] = found.owners[..] {
            return self.added(found, at, cx);
        }
        let owned = !found.owners.is_empty();
        let mut choices = match owned {
            true => found.owners.clone(),
            false => (0..found.games.len()).filter(|&at| found.games[at].trainer.is_some()).collect(),
        };
        choices.sort_by(|&a, &b| found.names[a].cmp(&found.names[b]));
        let (filter, typed) = filter_input(key::PICKER_FILTER, window, cx);
        filter.update(cx, |filter, cx| filter.focus(window, cx));
        self.picker = Some(Picker { found, choices, owned, filter, _typed: typed });
        cx.notify();
    }

    fn choose(&mut self, at: usize, cx: &mut Context<Self>) {
        if let Some(picker) = self.picker.take() {
            self.added(picker.found, at, cx);
        }
    }

    fn added(&mut self, Found { exe, games, .. }: Found, at: usize, cx: &mut Context<Self>) {
        self.library.update(cx, |library, cx| library.add(exe, games, at, cx));
        cx.notify();
    }

    fn close(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        if self.picker.take().is_some() {
            cx.notify();
        }
    }
}

impl Render for GamePicker {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(picker) = self.picker.as_ref() else { return div().into_any_element() };
        let query = picker.filter.read(cx).value().trim().to_lowercase();
        // The whole catalog waits for a name.
        let shown = if picker.owned || !query.is_empty() { SHOWN } else { 0 };
        let library = self.library.read(cx);
        let found = &picker.found;
        let rows = picker.choices.iter().filter(|&&at| found.names[at].contains(&query)).take(shown).map(|&at| {
            let game = &found.games[at];
            let store = game.store_ids.first().map_or(String::new(), |id| id.store().to_uppercase());
            // Picking it adds a source to the listed game.
            let store = match library.game(&GameId::of(&game.id)).is_some() {
                true => tf(cx, key::PICKER_IN_LIBRARY, &[("store", Arg::Str(&store))]),
                false => store.into(),
            };
            h_flex()
                .id(("choice", at))
                .h(px(32.))
                .flex_none()
                .px(px(10.))
                .gap(px(12.))
                .justify_between()
                .cursor_pointer()
                .hover(|style| style.bg(rgb(ROW_HOVER)))
                .on_click(cx.listener(move |view, _, _, cx| view.choose(at, cx)))
                .child(mono(game.name.to_uppercase(), 12.).text_color(rgb(TEXT_2)).overflow_hidden().text_ellipsis())
                .child(mono(store, 10.5).text_color(rgb(FAINT)))
        });
        let caption = match picker.owned {
            true => t(cx, key::PICKER_OWNED),
            false => t(cx, key::PICKER_UNOWNED),
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
        dialog("picker", t(cx, key::PICKER_TITLE), 520., &self.focus, Self::close, body, cx).into_any_element()
    }
}

pub(super) fn prompt(cx: &mut App) -> oneshot::Receiver<Result<Option<Vec<PathBuf>>>> {
    cx.prompt_for_paths(PathPromptOptions { files: true, directories: false, multiple: false, prompt: Some(t(cx, key::PICKER_PROMPT)) })
}

/// The file picked, if it is an executable; a shortcut comes as its target.
pub(super) async fn picked_file(paths: oneshot::Receiver<Result<Option<Vec<PathBuf>>>>) -> Option<PathBuf> {
    let exe = match paths.await {
        Ok(Ok(Some(paths))) => paths.into_iter().next()?,
        Ok(Err(e)) => {
            log::warn!("file picker: {e:#}");
            return None;
        }
        _ => return None,
    };
    let is_exe = exe.extension().is_some_and(|extension| extension.eq_ignore_ascii_case("exe"));
    if !is_exe {
        log::warn!("{} is not an .exe", exe.display());
    }
    is_exe.then_some(exe)
}
