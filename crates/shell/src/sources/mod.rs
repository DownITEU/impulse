//! The hero's `⋯` switches a game's sources; `+ ADD` adds a game the launchers did not find.

mod add;

use std::{mem, path::PathBuf, sync::Arc};

use anyhow::Result;
use blocking::unblock;
use client::library::{
    Game,
    sources::{Entry, entries, exe},
};
use futures::channel::oneshot;
use gpui_kit::{Context, PathPromptOptions, SharedString, Window};

pub(super) use add::Picker;

use super::{Load, Page};

impl Page {
    /// The `⋯` menu's options and the one in use.
    pub(super) fn source_options(game: &Game) -> (Vec<SharedString>, usize) {
        let (entries, current) = entries(game);
        (entries.into_iter().map(|(_, label)| label.into()).collect(), current)
    }

    /// A pick in the open game's `⋯` menu.
    pub(super) fn source_picked(&mut self, at: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(game) = self.game() else { return };
        let (id, entry) = (game.id.clone(), entries(game).0.get(at).map(|(entry, _)| *entry));
        match entry {
            Some(Entry::Use(true)) => self.switch_source(&id, window, cx),
            Some(Entry::Pick) => self.pick_exe(id, window, cx),
            Some(Entry::Forget) => self.forget_exe(&id, window, cx),
            Some(Entry::Use(false)) | None => {}
        }
    }

    fn game_mut(&mut self, id: &str) -> Option<&mut Game> {
        match &mut self.library {
            Load::Ready(library) => library.games.iter_mut().find(|game| &*game.id == id),
            _ => None,
        }
    }

    fn switch_source(&mut self, id: &Arc<str>, window: &mut Window, cx: &mut Context<Self>) {
        let Some(game) = self.game_mut(id) else { return };
        game.switch();
        let used = exe(&game.install).is_some();
        self.picked.set_used(id, used);
        self.open_trainer(window, cx);
        cx.notify();
    }

    fn pick_exe(&mut self, id: Arc<str>, window: &mut Window, cx: &mut Context<Self>) {
        let paths = prompt(cx);
        cx.spawn_in(window, async move |page, cx| {
            if let Some(exe) = picked_file(paths).await {
                page.update_in(cx, |page, window, cx| page.use_exe(id, exe, window, cx)).ok();
            }
        })
        .detach();
    }

    /// Makes `exe` the source of a game in the library.
    fn use_exe(&mut self, id: Arc<str>, exe: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        let Some(game) = self.game_mut(&id) else { return };
        // Only these two are read off the thread.
        let probe = model::Game { id: game.source.id.clone(), exe: game.source.exe.clone(), ..Default::default() };
        cx.spawn_in(window, async move |page, cx| {
            let install = unblock(move || launchers::install(&probe, &exe).map(|install| (install, exe))).await;
            page.update_in(cx, |page, window, cx| {
                let Some((install, exe)) = install else { return };
                page.picked.set(&id, Some(exe));
                if let Some(game) = page.game_mut(&id) {
                    game.pick(install);
                }
                page.open_trainer(window, cx);
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// A game the user added leaves the library with its executable.
    fn forget_exe(&mut self, id: &Arc<str>, window: &mut Window, cx: &mut Context<Self>) {
        self.picked.set(id, None);
        let Some(game) = self.game_mut(id) else { return };
        if !game.unpick()
            && let Load::Ready(mut library) = mem::replace(&mut self.library, Load::Pending)
        {
            library.games.retain(|game| game.id != *id);
            self.set_library(library, window, cx);
            return;
        }
        self.open_trainer(window, cx);
        cx.notify();
    }
}

fn prompt(cx: &mut Context<Page>) -> oneshot::Receiver<Result<Option<Vec<PathBuf>>>> {
    cx.prompt_for_paths(PathPromptOptions {
        files: true,
        directories: false,
        multiple: false,
        prompt: Some("Select the game's .exe".into()),
    })
}

/// The file picked, if it is an executable; a shortcut comes as its target.
async fn picked_file(paths: oneshot::Receiver<Result<Option<Vec<PathBuf>>>>) -> Option<PathBuf> {
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
