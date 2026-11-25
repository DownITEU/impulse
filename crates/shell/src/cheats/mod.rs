//! The open game's cheat list: its rows, controls, collections and saved bar. The page and the HUD draw the same
//! list; the collection editor it opens is drawn over the window by whoever draws it (`editor`).

mod collections;
mod controls;
mod list;
mod notes;
mod printed;

use std::sync::Arc;

use client::library::{Game, GameId};
use gpui_kit::{App, Context, Entity, EventEmitter, IntoElement, Render, Task, Window, component::input::InputState};

use crate::{
    hotkeys::Hotkeys,
    library::{GamePicker, Library, LibraryEvent},
    run::{RunEvent, Runs},
    trainer::{TrainerEvent, Trainers},
    ui::{key, text::Texts, widgets},
};
pub use collections::{CollectionEditor, DEFAULT_ICON};
pub use printed::category;
use printed::{Printed, number};

/// The open dropdown; both hold a cheat's or a collection's index in the shown trainer.
#[derive(Clone, Copy, PartialEq)]
enum Menu {
    Cheat(usize),
    Collection(usize),
}

/// The collection editor closed: whoever drew it takes the focus back.
pub struct EditorClosed;

pub struct CheatList {
    library: Entity<Library>,
    trainers: Entity<Trainers>,
    runs: Entity<Runs>,
    hotkeys: Entity<Hotkeys>,
    picker: Entity<GamePicker>,
    printed: Option<Printed>,
    filter: Entity<InputState>,
    menu: Option<Menu>,
    // The press-only button showing its confirmation.
    flashed: Option<(usize, Task<()>)>,
    editor: Option<Entity<CollectionEditor>>,
    // The slider pressed on its ticks: a drag begun elsewhere (selecting its field's text) moves none.
    sliding: Option<usize>,
}

impl EventEmitter<EditorClosed> for CheatList {}

impl CheatList {
    pub fn new(
        library: Entity<Library>,
        trainers: Entity<Trainers>,
        runs: Entity<Runs>,
        hotkeys: Entity<Hotkeys>,
        picker: Entity<GamePicker>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        cx.observe(&library, |_, _, cx| cx.notify()).detach();
        cx.observe(&trainers, |_, _, cx| cx.notify()).detach();
        cx.observe(&runs, |_, _, cx| cx.notify()).detach();
        cx.observe(&hotkeys, |_, _, cx| cx.notify()).detach();
        cx.subscribe_in(&library, window, |list, _, event: &LibraryEvent, window, cx| {
            if matches!(event, LibraryEvent::Opened | LibraryEvent::Retrained(_)) {
                // Another trainer's names: the old filter would hide them.
                list.filter.update(cx, |filter, cx| filter.set_value("", window, cx));
                list.drop_state();
                list.print(window, cx);
            }
        })
        .detach();
        cx.subscribe_in(&trainers, window, Self::trainer_changed).detach();
        // Categories are worded in the user's language.
        cx.observe_global_in::<Texts>(window, |list, window, cx| list.print(window, cx)).detach();
        cx.subscribe(&runs, |list, _, event: &RunEvent, cx| match event {
            RunEvent::Pressed(index) if list.is_running(cx) => list.flash(*index, cx),
            // The ticks stop listening with the run: a drag under way would outlive it.
            RunEvent::Ended => list.sliding = None,
            _ => {}
        })
        .detach();
        let (filter, typed) = widgets::filter_input(key::CHEATS_FILTER, window, cx);
        typed.detach();
        Self { library, trainers, runs, hotkeys, picker, printed: None, filter, menu: None, flashed: None, editor: None, sliding: None }
    }

    /// The collection being named, drawn over the window.
    pub fn editor(&self) -> Option<&Entity<CollectionEditor>> {
        self.editor.as_ref()
    }

    /// Escape: the editor closes first, then the open menu; `false` when neither was open.
    pub fn close_top(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if let Some(editor) = self.editor.clone() {
            editor.update(cx, |editor, cx| editor.close(window, cx));
            return true;
        }
        if self.menu.take().is_some() {
            cx.notify();
            return true;
        }
        false
    }

    /// Closes what belongs to the window it was drawn in.
    pub fn close(&mut self, cx: &mut Context<Self>) {
        (self.menu, self.editor) = (None, None);
        cx.notify();
    }

    fn game(&self, cx: &App) -> Option<Arc<Game>> {
        self.library.read(cx).opened().cloned()
    }

    fn selected(&self, cx: &App) -> Option<GameId> {
        self.library.read(cx).open_id().cloned()
    }

    fn is_running(&self, cx: &App) -> bool {
        self.library.read(cx).open_id().is_some_and(|game| self.runs.read(cx).is_live(game))
    }

    /// The open game's cheat list as printed, made anew when another game opens or another copy shows.
    fn print(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let game = self.selected(cx);
        self.printed = self.trainers.update(cx, |trainers, cx| {
            let game = game?;
            Some(Printed::new(game.clone(), trainers.sheet(&game)?, window, cx))
        });
        self.follow_fields(window, cx);
    }

    fn trainer_changed(&mut self, trainers: &Entity<Trainers>, event: &TrainerEvent, window: &mut Window, cx: &mut Context<Self>) {
        match event {
            TrainerEvent::Installed(game) => {
                if self.library.read(cx).open_id() == Some(game) {
                    self.drop_state();
                    self.print(window, cx);
                }
            }
            TrainerEvent::Rebound(game) => {
                if let Some(printed) = self.printed.as_mut().filter(|printed| printed.game == *game)
                    && let Some(sheet) = trainers.read(cx).sheet(game)
                {
                    printed.recap(sheet);
                }
            }
            TrainerEvent::Set { game, index, value } => {
                if let Some(printed) = self.printed.as_ref().filter(|printed| printed.game == *game) {
                    printed.put(*index, *value, window, cx);
                }
            }
            TrainerEvent::Changed(game) => {
                // A device may have deleted a collection: an open menu's index would mean another.
                if matches!(self.menu, Some(Menu::Collection(_))) {
                    self.menu = None;
                }
                if let Some(printed) = self.printed.as_ref().filter(|printed| printed.game == *game)
                    && let Some(values) = trainers.read(cx).sheet(game).map(|sheet| sheet.values.clone())
                {
                    printed.refill(&values, window, cx);
                }
            }
        }
    }

    /// The open menu, flash and editor are keyed by index, which means another cheat in another trainer.
    fn drop_state(&mut self) {
        (self.menu, self.flashed, self.editor, self.sliding) = (None, None, None, None);
    }

    fn flash(&mut self, index: usize, cx: &mut Context<Self>) {
        let timer = widgets::flash(cx, |list: &mut Self| list.flashed = None);
        self.flashed = Some((index, timer));
        cx.notify();
    }

    fn is_flashed(&self, index: usize) -> bool {
        self.flashed.as_ref().is_some_and(|(at, _)| *at == index)
    }
}

impl Render for CheatList {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let game = self.game(cx);
        self.cheats(game.as_deref(), cx)
    }
}

fn open_menu(list: &mut CheatList) -> &mut Option<Menu> {
    &mut list.menu
}
