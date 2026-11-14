//! The hero's `⋯` switches a game's sources; `+ ADD` adds a game the launchers did not find.

mod add;

use std::path::PathBuf;

use anyhow::Result;
use client::library::{
    Game,
    sources::{Entry, entries},
};
use futures::channel::oneshot;
use gpui_kit::{Context, PathPromptOptions, SharedString, Window};

pub(super) use add::Picker;

use super::Page;

impl Page {
    /// The `⋯` menu's options and the one in use.
    pub(super) fn source_options(game: &Game) -> (Vec<SharedString>, usize) {
        let (entries, current) = entries(game);
        (entries.into_iter().map(|(_, label)| label.into()).collect(), current)
    }

    /// A pick in the open game's `⋯` menu.
    pub(super) fn source_picked(&mut self, at: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(game) = self.game(cx) else { return };
        let (id, entry) = (game.id.clone(), entries(&game).0.get(at).map(|(entry, _)| *entry));
        match entry {
            Some(Entry::Use(true)) => self.library.update(cx, |library, cx| library.switch(&id, cx)),
            Some(Entry::Pick) => {
                let (paths, library) = (prompt(cx), self.library.downgrade());
                cx.spawn_in(window, async move |_, cx| {
                    if let Some(exe) = picked_file(paths).await {
                        library.update(cx, |library, cx| library.pick(id, exe, cx)).ok();
                    }
                })
                .detach();
            }
            Some(Entry::Forget) => self.library.update(cx, |library, cx| library.forget(&id, cx)),
            Some(Entry::Use(false)) | None => {}
        }
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
