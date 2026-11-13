//! A Settings row per provider folder: a path field, the provider's ready picks and a folder picker.
//! The provider checks and saves every edit; the row shows what it answers.

use std::path::PathBuf;

use gpui_kit::{
    Context, Div, Entity, PathPromptOptions, SharedString, Stateful, Task, Window,
    component::{
        h_flex,
        input::{Input, InputEvent, InputState},
        v_flex,
    },
    div,
    prelude::*,
    px, rgb,
};
use model::Folder;

use super::{
    Menu, Page,
    assets::Icon,
    controls::span,
    settings::label,
    theme::*,
    widgets::{icon_button, mono},
};

pub(super) struct FolderRow {
    // The provider's last answer; its path is not the field's text once the user edits.
    folder: Folder,
    input: Entity<InputState>,
    // Starting a check drops the previous one, so the newest text wins.
    check: Task<()>,
    // A check is out: the provider may still apply it, so a listing must not show another path.
    checking: bool,
    // Counts the user's edits: a listing that started before one, or during a check, must not overwrite it.
    edits: u32,
}

impl Page {
    /// Lists the folders: their picks always, their path and status unless the user edited since.
    pub(super) fn scan_folders(&self, window: &Window, cx: &mut Context<Self>) {
        let edits: Vec<Option<u32>> = self.folders.iter().map(|row| (!row.checking).then_some(row.edits)).collect();
        let listing = self.provider.folders();
        cx.spawn_in(window, async move |page, cx| {
            let folders = listing.await;
            page.update_in(cx, |page, window, cx| {
                for folder in folders {
                    page.list_folder(folder, &edits, window, cx);
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn list_folder(&mut self, folder: Folder, edits: &[Option<u32>], window: &mut Window, cx: &mut Context<Self>) {
        let Some(at) = self.folders.iter().position(|row| row.folder.id == folder.id) else {
            let at = self.folders.len();
            let input = cx.new(|cx| InputState::new(window, cx).placeholder("path to the folder"));
            // Only typing emits `Change`; `set_value` does not.
            cx.subscribe(&input, move |page, _, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    page.check_folder(at, cx);
                }
            })
            .detach();
            set_text(&input, folder.path.clone(), window, cx);
            self.folders.push(FolderRow { folder, input, check: Task::ready(()), checking: false, edits: 0 });
            return;
        };
        let row = &mut self.folders[at];
        if edits.get(at) == Some(&Some(row.edits)) {
            set_text(&row.input, folder.path.clone(), window, cx);
            row.folder = folder;
        } else {
            row.folder.choices = folder.choices;
        }
    }

    /// Resolved off the UI thread: the path may be a slow network share.
    fn check_folder(&mut self, at: usize, cx: &mut Context<Self>) {
        let row = &mut self.folders[at];
        row.edits = row.edits.wrapping_add(1);
        // "Copy as path" in Explorer wraps the path in quotes.
        let path = PathBuf::from(row.input.read(cx).value().trim().trim_matches('"'));
        let checked = self.provider.set_folder(row.folder.id, path);
        row.checking = true;
        row.check = cx.spawn(async move |page, cx| {
            let folder = checked.await;
            page.update(cx, |page, cx| {
                let row = &mut page.folders[at];
                (row.folder, row.checking) = (folder, false);
                cx.notify();
            })
            .ok();
        });
    }

    fn pick_folder(&mut self, at: usize, path: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        set_text(&self.folders[at].input, Some(path), window, cx);
        self.check_folder(at, cx);
    }

    fn browse_folder(&mut self, at: usize, window: &mut Window, cx: &mut Context<Self>) {
        let picked = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some("Select the folder".into()),
        });
        cx.spawn_in(window, async move |page, cx| match picked.await {
            Ok(Ok(Some(paths))) => {
                if let Some(path) = paths.into_iter().next() {
                    page.update_in(cx, |page, window, cx| page.pick_folder(at, path, window, cx)).ok();
                }
            }
            Ok(Err(e)) => log::warn!("folder picker: {e:#}"),
            _ => {}
        })
        .detach();
    }

    pub(super) fn folder_rows(&self, cx: &mut Context<Self>) -> Vec<Div> {
        (0..self.folders.len()).map(|at| self.folder_row(at, cx)).collect()
    }

    fn folder_row(&self, at: usize, cx: &mut Context<Self>) -> Div {
        let FolderRow { folder, input, .. } = &self.folders[at];
        let choices: Vec<SharedString> = folder.choices.iter().map(|(label, _)| label.clone().into()).collect();
        let current = folder.choices.iter().position(|(_, path)| Some(path) == folder.selected.as_ref()).unwrap_or(usize::MAX);
        let edge = |button: Stateful<Div>| button.border_l_1().border_color(line(0.14));
        let field = h_flex()
            .id(("folder", at))
            .h(px(32.))
            .border_1()
            .border_color(line(0.14))
            .bg(rgb(SUNKEN))
            .child(
                div()
                    .id(("folder-text", at))
                    .flex_1()
                    .min_w_0()
                    .px(px(10.))
                    // The field is inside the list's trigger: clicking into it must not open the list.
                    .on_click(|_, _, cx| cx.stop_propagation())
                    .child(Input::new(input).appearance(false).px_0().font_family(MONO).text_size(px(12.5))),
            )
            .when(!choices.is_empty(), |field| field.child(edge(icon_button(("folder-choices", at), Icon::ChevronDown, 30., 12.))))
            .child(edge(icon_button(("folder-browse", at), Icon::Folder, 30., 14.)).on_click(cx.listener(move |page, _, window, cx| {
                cx.stop_propagation();
                page.browse_folder(at, window, cx);
            })));
        let field = self.dropdown(field, Menu::Choices(at), choices, current, span, cx, move |page, pick, window, cx| {
            if let Some((_, path)) = page.folders[at].folder.choices.get(pick) {
                page.pick_folder(at, path.clone(), window, cx);
            }
        });
        let status = match &folder.status {
            Ok(found) => mono(found.clone(), 11.).text_color(rgb(accent(cx))),
            Err(why) => mono(why.clone(), 11.).text_color(rgb(WARN)),
        };
        v_flex().gap(px(8.)).py(px(12.)).child(label(folder.label, folder.hint)).child(field).child(status)
    }
}

fn set_text(input: &Entity<InputState>, path: Option<PathBuf>, window: &mut Window, cx: &mut Context<Page>) {
    let text = path.map(|path| path.to_string_lossy().into_owned()).unwrap_or_default();
    input.update(cx, |input, cx| input.set_value(text, window, cx));
}
