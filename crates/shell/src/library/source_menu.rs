//! The hero's `⋯`: switches the open game's sources.

use client::library::{
    Game,
    sources::{Entry, Label, entries},
};
use gpui_kit::{App, Entity, SharedString, Window};
use i18n::Arg;

use super::{
    GamePicker, Library,
    picker::{picked_file, prompt},
};
use crate::ui::{
    key,
    text::{t, tf},
};

/// The menu's options and the one in use.
pub fn source_options(game: &Game, cx: &App) -> (Vec<SharedString>, usize) {
    let (entries, current) = entries(game);
    (entries.into_iter().map(|(_, label)| caption(&label, cx)).collect(), current)
}

fn caption(label: &Label, cx: &App) -> SharedString {
    match label {
        Label::Store(store) => store.clone().into(),
        Label::Exe(file) => tf(cx, key::LIBRARY_SOURCE_EXE, &[("file", Arg::Str(file))]),
        Label::Add => t(cx, key::LIBRARY_SOURCE_ADD),
        Label::Change => t(cx, key::LIBRARY_SOURCE_CHANGE),
        Label::Remove => t(cx, key::LIBRARY_SOURCE_REMOVE),
        Label::Trainer => t(cx, key::LIBRARY_SOURCE_TRAINER),
        Label::OwnTrainer => t(cx, key::LIBRARY_SOURCE_OWN_TRAINER),
    }
}

pub fn pick_source(library: &Entity<Library>, picker: &Entity<GamePicker>, game: &Game, at: usize, window: &mut Window, cx: &mut App) {
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
        Some(Entry::Trainer) => picker.update(cx, |picker, cx| picker.trainer(&id, window, cx)),
        Some(Entry::OwnTrainer) => library.update(cx, |library, cx| library.set_trainer(&id, None, cx)),
        Some(Entry::Use(false)) | None => {}
    }
}
