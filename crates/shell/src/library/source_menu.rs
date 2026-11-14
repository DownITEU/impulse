//! The hero's `⋯`: switches the open game's sources.

use client::library::{
    Game,
    sources::{Entry, entries},
};
use gpui_kit::{App, Entity, SharedString};

use super::{
    Library,
    picker::{picked_file, prompt},
};

/// The menu's options and the one in use.
pub fn source_options(game: &Game) -> (Vec<SharedString>, usize) {
    let (entries, current) = entries(game);
    (entries.into_iter().map(|(_, label)| label.into()).collect(), current)
}

/// A pick in `game`'s menu.
pub fn pick_source(library: &Entity<Library>, game: &Game, at: usize, cx: &mut App) {
    let (id, entry) = (game.id.clone(), entries(game).0.get(at).map(|(entry, _)| *entry));
    match entry {
        Some(Entry::Use(true)) => library.update(cx, |library, cx| library.switch(&id, cx)),
        Some(Entry::Pick) => {
            let (paths, library) = (prompt(cx), library.downgrade());
            cx.spawn(async move |cx| {
                if let Some(exe) = picked_file(paths).await {
                    library.update(cx, |library, cx| library.pick(id, exe, cx)).ok();
                }
            })
            .detach();
        }
        Some(Entry::Forget) => library.update(cx, |library, cx| library.forget(&id, cx)),
        Some(Entry::Use(false)) | None => {}
    }
}
