//! A row per source folder (`Folders`): a path field, the source's ready picks and a folder picker.
//! The source checks and saves every edit; the row shows what it answers.

use std::path::PathBuf;

use gpui_kit::{
    Context, Div, Entity, PathPromptOptions, SharedString, Stateful, Window,
    component::{
        h_flex,
        input::{Input, InputEvent},
        v_flex,
    },
    div,
    prelude::*,
    px, rgb,
};

use super::{
    Folders, SettingsView, label,
    view::{Menu, open_menu},
};
use crate::ui::{
    assets::Icon,
    dropdown::dropdown,
    key,
    text::{said, t, text},
    theme::*,
    widgets::{icon_button, input, mono},
};

impl SettingsView {
    /// A row's field shows the source's path; a new row gets its field.
    pub(super) fn folder_shown(&mut self, folders: &Entity<Folders>, at: usize, window: &mut Window, cx: &mut Context<Self>) {
        if at == self.fields.len() {
            // One per row, kept for the life of the dialog.
            let (state, relabel) = input(key::FOLDER_PATH, window, cx);
            relabel.detach();
            // Only typing emits `Change`; `set_value` does not.
            cx.subscribe(&state, move |view, input, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    let text = input.read(cx).value().to_string();
                    view.folders.update(cx, |folders, cx| folders.set(at, &text, cx));
                }
            })
            .detach();
            self.fields.push(state);
        }
        let text = folders.read(cx).get(at).path.as_ref().map(|path| path.to_string_lossy().into_owned()).unwrap_or_default();
        self.fields[at].update(cx, |input, cx| input.set_value(text, window, cx));
    }

    fn pick_folder(&mut self, at: usize, path: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        let text = path.to_string_lossy().into_owned();
        self.fields[at].update(cx, |input, cx| input.set_value(text.clone(), window, cx));
        self.folders.update(cx, |folders, cx| folders.set(at, &text, cx));
    }

    fn browse_folder(&mut self, at: usize, window: &mut Window, cx: &mut Context<Self>) {
        let picked = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some(t(cx, key::FOLDER_SELECT)),
        });
        cx.spawn_in(window, async move |view, cx| match picked.await {
            Ok(Ok(Some(paths))) => {
                if let Some(path) = paths.into_iter().next() {
                    view.update_in(cx, |view, window, cx| view.pick_folder(at, path, window, cx)).ok();
                }
            }
            Ok(Err(e)) => log::warn!("folder picker: {e:#}"),
            _ => {}
        })
        .detach();
    }

    pub(super) fn folder_rows(&self, cx: &mut Context<Self>) -> Vec<Div> {
        (0..self.fields.len()).map(|at| self.folder_row(at, cx)).collect()
    }

    fn folder_row(&self, at: usize, cx: &mut Context<Self>) -> Div {
        let (folder, input) = (self.folders.read(cx).get(at), &self.fields[at]);
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
                    .child(Input::new(input).appearance(false).px_0().font_family(MONO).text_size(px(TYPE_FIELD))),
            )
            .when(!choices.is_empty(), |field| field.child(edge(icon_button(("folder-choices", at), Icon::ChevronDown, 30., 12.))))
            .child(edge(icon_button(("folder-browse", at), Icon::Folder, 30., 14.)).on_click(cx.listener(move |view, _, window, cx| {
                cx.stop_propagation();
                view.browse_folder(at, window, cx);
            })));

        let menu = Menu::Folder(at);
        let field =
            dropdown(field, menu, open_menu, self.menu == Some(menu), choices, current, px(200.), cx, move |view, pick, window, cx| {
                let path = view.folders.read(cx).get(at).choices.get(pick).map(|(_, path)| path.clone());
                if let Some(path) = path {
                    view.pick_folder(at, path, window, cx);
                }
            });
        let status = match &folder.status {
            Ok(found) => mono(text(cx, found), TYPE_LABEL).text_color(rgb(accent(cx))),
            Err(failure) => mono(said(cx, failure), TYPE_LABEL).text_color(rgb(WARN)),
        };
        v_flex().gap(px(8.)).py(px(12.)).child(label(text(cx, &folder.label), text(cx, &folder.hint))).child(field).child(status)
    }
}
