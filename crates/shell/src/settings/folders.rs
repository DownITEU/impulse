//! The source's folders: each one's last answer, and the user's text checked and saved by the source.

use std::{path::PathBuf, sync::Arc};

use client::Registry;
use gpui_kit::{Context, EventEmitter, Task};
use model::Folder;

struct Row {
    // The source's last answer; its path is not the field's text once the user edits.
    folder: Folder,
    // Starting a check drops the previous one, so the newest text wins.
    check: Task<()>,
    // A check is out: the source may still apply it, so a listing must not show another path.
    checking: bool,
    // Counts the user's edits: a listing that started before one, or during a check, must not overwrite it.
    edits: u32,
}

/// The row's field shows the source's path now: a new row, or one not edited since the listing started.
/// Rows only grow, in listing order: `at` names a row for good.
pub struct Shown(pub usize);

pub struct Folders {
    registry: Arc<Registry>,
    rows: Vec<Row>,
}

impl EventEmitter<Shown> for Folders {}

impl Folders {
    pub fn new(registry: Arc<Registry>, cx: &mut Context<Self>) -> Self {
        let mut folders = Self { registry, rows: Vec::new() };
        folders.scan(cx);
        folders
    }

    pub fn get(&self, at: usize) -> &Folder {
        &self.rows[at].folder
    }

    /// Lists the folders: their picks always, their path and status unless the user edited since.
    pub fn scan(&mut self, cx: &mut Context<Self>) {
        let edits: Vec<Option<u32>> = self.rows.iter().map(|row| (!row.checking).then_some(row.edits)).collect();
        let listing = self.registry.folders();
        cx.spawn(async move |folders, cx| {
            let listed = listing.await;
            folders
                .update(cx, |folders, cx| {
                    for folder in listed {
                        folders.list(folder, &edits, cx);
                    }
                    cx.notify();
                })
                .ok();
        })
        .detach();
    }

    /// Checks the user's `text` for the folder `at`; resolved off the UI thread: the path may be a slow network share.
    pub fn set(&mut self, at: usize, text: &str, cx: &mut Context<Self>) {
        let row = &mut self.rows[at];
        row.edits = row.edits.wrapping_add(1);
        // "Copy as path" in Explorer wraps the path in quotes.
        let path = PathBuf::from(text.trim().trim_matches('"'));
        let checked = self.registry.set_folder(&row.folder.id, path);
        row.checking = true;
        row.check = cx.spawn(async move |folders, cx| {
            let folder = checked.await;
            folders
                .update(cx, |folders, cx| {
                    let row = &mut folders.rows[at];
                    (row.folder, row.checking) = (folder, false);
                    cx.notify();
                })
                .ok();
        });
    }

    fn list(&mut self, folder: Folder, edits: &[Option<u32>], cx: &mut Context<Self>) {
        let Some(at) = self.rows.iter().position(|row| row.folder.id == folder.id) else {
            cx.emit(Shown(self.rows.len()));
            self.rows.push(Row { folder, check: Task::ready(()), checking: false, edits: 0 });
            return;
        };
        let row = &mut self.rows[at];
        if edits.get(at) == Some(&Some(row.edits)) {
            row.folder = folder;
            cx.emit(Shown(at));
        } else {
            row.folder.choices = folder.choices;
        }
    }
}
