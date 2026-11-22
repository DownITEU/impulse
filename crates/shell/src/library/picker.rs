//! `+ ADD`: a game by its executable. The catalog games that keep their `exe` beside it are offered;
//! one is added at once, several or none open the picker, where the user finds it by name. The same picker chooses
//! the trainer of a listed game: its title's other records first, then every trainer by likeness of name.

use std::{path::PathBuf, sync::Arc};

use anyhow::Result;
use futures::channel::oneshot;
use gpui_kit::{
    App, Context, Entity, FocusHandle, IntoElement, PathPromptOptions, Render, SharedString, Subscription, Window,
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
use model::Ref;

use super::{Found, Library};
use crate::ui::{
    dialog::dialog,
    key,
    text::{t, tf},
    theme::*,
    widgets::{filter_input, mono},
};
use client::{
    Load,
    library::{GameId, store_of},
};

/// Rows shown at once: the filter narrows the rest.
const SHOWN: usize = 50;

enum Purpose {
    /// `owned`: the choices are the games the executable belongs to.
    Add { exe: PathBuf, owned: bool },
    /// `current`: the record whose trainer the game uses now.
    Trainer { game: GameId, current: Option<Ref> },
}

/// A row of `games[at]`, its texts made once.
struct Choice {
    at: usize,
    name: SharedString,
    /// Lower case, for the filter.
    search: String,
    store: SharedString,
}

impl Choice {
    fn of(games: &[model::Game], at: usize) -> Self {
        let record = &games[at];
        Self { at, name: record.name.to_uppercase().into(), search: record.name.to_lowercase(), store: store_of(record).into() }
    }
}

struct Picker {
    purpose: Purpose,
    games: Arc<Vec<model::Game>>,
    /// In the order shown.
    choices: Vec<Choice>,
    /// The line over the caption: the executable, or the game.
    subject: SharedString,
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

    /// Opens the trainer's choice for the listed game `id`.
    pub fn trainer(&mut self, id: &GameId, window: &mut Window, cx: &mut Context<Self>) {
        let library = self.library.read(cx);
        let (Load::Ready(list), Some(game)) = (library.list(), library.game(id)) else { return };
        let choices = list.trainer_records(game).into_iter().map(|at| Choice::of(&list.catalog, at)).collect();
        let subject = match &game.store {
            Some(store) => format!("{}  {store}", game.name),
            None => game.name.to_string(),
        };
        let purpose = Purpose::Trainer { game: id.clone(), current: game.trained_by().map(|record| record.id.clone()) };
        let games = list.catalog.clone();
        self.open(purpose, games, choices, subject.into(), window, cx);
    }

    fn offer(&mut self, found: Found, window: &mut Window, cx: &mut Context<Self>) {
        let Found { exe, games, owners } = found;
        if let [at] = owners[..] {
            return self.library.update(cx, |library, cx| library.add(exe, games, at, cx));
        }
        let owned = !owners.is_empty();
        let shown = match owned {
            true => owners,
            false => (0..games.len()).filter(|&at| games[at].trainer.is_some()).collect(),
        };
        let mut choices: Vec<Choice> = shown.into_iter().map(|at| Choice::of(&games, at)).collect();
        choices.sort_by(|a, b| a.search.cmp(&b.search));
        let subject = exe.display().to_string().into();
        self.open(Purpose::Add { exe, owned }, games, choices, subject, window, cx);
    }

    fn open(
        &mut self,
        purpose: Purpose,
        games: Arc<Vec<model::Game>>,
        choices: Vec<Choice>,
        subject: SharedString,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let (filter, typed) = filter_input(key::PICKER_FILTER, window, cx);
        filter.update(cx, |filter, cx| filter.focus(window, cx));
        self.picker = Some(Picker { purpose, games, choices, subject, filter, _typed: typed });
        cx.notify();
    }

    fn choose(&mut self, at: usize, cx: &mut Context<Self>) {
        let Some(Picker { purpose, games, .. }) = self.picker.take() else { return };
        match purpose {
            Purpose::Add { exe, .. } => self.library.update(cx, |library, cx| library.add(exe, games, at, cx)),
            Purpose::Trainer { game, .. } => {
                let record = games[at].id.clone();
                self.library.update(cx, |library, cx| library.set_trainer(&game, Some(record), cx));
            }
        }
        cx.notify();
    }

    /// Closes the dialog, as entering the overlay does.
    pub fn dismiss(&mut self, cx: &mut Context<Self>) {
        if self.picker.take().is_some() {
            cx.notify();
        }
    }

    fn close(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        self.dismiss(cx);
    }
}

impl Render for GamePicker {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(picker) = self.picker.as_ref() else { return div().into_any_element() };
        let query = picker.filter.read(cx).value().trim().to_lowercase();
        // The whole catalog waits for a name.
        let unowned = matches!(picker.purpose, Purpose::Add { owned: false, .. });
        let shown = if !unowned || !query.is_empty() { SHOWN } else { 0 };
        let library = self.library.read(cx);
        let rows = picker.choices.iter().filter(|choice| choice.search.contains(&query)).take(shown).map(|choice| {
            let (at, record, store) = (choice.at, &picker.games[choice.at], &choice.store);
            let (tag, mods) = match &picker.purpose {
                // Picking it adds a source to the listed game.
                Purpose::Add { .. } if library.game(&GameId::of(&record.id)).is_some() => {
                    (tf(cx, key::PICKER_IN_LIBRARY, &[("store", Arg::Str(store))]), None)
                }
                Purpose::Add { .. } => (store.clone(), None),
                Purpose::Trainer { current, .. } => {
                    let cheats = record.trainer.as_ref().map_or(0, |offer| offer.cheats);
                    let mods = Some(tf(cx, key::TRAINER_MODS, &[("count", Arg::Num(cheats.into()))]));
                    match current.as_ref() == Some(&record.id) {
                        true => (tf(cx, key::PICKER_IN_USE, &[("store", Arg::Str(store))]), mods),
                        false => (store.clone(), mods),
                    }
                }
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
                .child(mono(choice.name.clone(), 12.).text_color(rgb(TEXT_2)).min_w_0().truncate())
                .child(
                    h_flex()
                        .flex_none()
                        .gap(px(12.))
                        .when_some(mods, |tags, mods| tags.child(mono(mods, 10.5).text_color(rgb(DIM))))
                        .child(mono(tag, 10.5).text_color(rgb(FAINT))),
                )
        });
        let (title, caption) = match picker.purpose {
            Purpose::Add { owned: true, .. } => (key::PICKER_TITLE, key::PICKER_OWNED),
            Purpose::Add { owned: false, .. } => (key::PICKER_TITLE, key::PICKER_UNOWNED),
            Purpose::Trainer { .. } => (key::PICKER_TRAINER_TITLE, key::PICKER_TRAINER),
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
            .child(mono(picker.subject.clone(), 11.).text_color(rgb(MUTED)).min_w_0().truncate())
            .child(mono(t(cx, caption), 10.5).text_color(rgb(DIM)))
            .child(field)
            .child(div().id("choices").max_h(px(320.)).overflow_y_scroll().flex().flex_col().children(rows));
        dialog("picker", t(cx, title), 520., &self.focus, Self::close, body, cx).into_any_element()
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
