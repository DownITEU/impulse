//! The Settings row naming the Wand install the trainer binaries are read from: a path field that
//! starts at the newest installed version, a list of the installed versions and a folder picker.

use std::{mem, path::PathBuf};

use blocking::unblock;
use gpui_kit::{
    Context, Div, Entity, PathPromptOptions, SharedString, Task, Window,
    component::{
        h_flex,
        input::{Input, InputEvent, InputState},
        v_flex,
    },
    div,
    prelude::*,
    px, rgb,
};
use wand::install::{self, Install};

use super::{
    Menu, Page,
    assets::Icon,
    settings::label,
    theme::*,
    widgets::{icon_button, mono},
};
use crate::preferences;

pub(super) struct WandDir {
    input: Entity<InputState>,
    installs: Vec<Install>,
    // What the field's text points at; `None` while it is not a Wand folder.
    found: Option<Install>,
    // Starting a check drops the previous one, so the newest text wins.
    check: Task<()>,
    // Counts the user's edits: a scan that started before one must not overwrite it.
    edits: u32,
    // Each save waits for the one before it, so the last choice is the last write.
    save: Task<()>,
}

impl WandDir {
    pub fn new(window: &mut Window, cx: &mut Context<Page>) -> Self {
        let input = cx.new(|cx| InputState::new(window, cx).placeholder("path to the Wand folder"));
        // Only typing emits `Change`; `set_value` does not.
        cx.subscribe(&input, |page, _, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Change) {
                page.check_wand_dir(true, cx);
            }
        })
        .detach();
        Self {
            input,
            installs: Vec::new(),
            found: None,
            check: Task::ready(()),
            edits: 0,
            save: Task::ready(()),
        }
    }
}

impl Page {
    /// Lists the installed versions and shows the saved choice, else the newest version.
    pub(super) fn scan_wand(&self, window: &Window, cx: &mut Context<Self>) {
        let (chosen, edits) = (self.preferences.wand_dir.clone(), self.wand.edits);
        cx.spawn_in(window, async move |page, cx| {
            let (installs, chosen) = unblock(move || {
                // Squirrel removes old versions after an update, so a gone choice falls back to the newest.
                let chosen = chosen.filter(|dir| install::resolve(dir).is_some());
                (install::installs(), chosen)
            })
            .await;
            page.update_in(cx, |page, window, cx| {
                let shown = chosen
                    .or_else(|| installs.first().map(|install| install.root.clone()))
                    .unwrap_or_default();
                page.wand.installs = installs;
                match page.wand.edits == edits {
                    true => page.show_wand_dir(shown, false, window, cx),
                    false => cx.notify(),
                }
            })
            .ok();
        })
        .detach();
    }

    /// Wand's `trainerlib` folder, once the field points at an install.
    pub(super) fn wand_trainerlib(&self) -> Option<PathBuf> {
        self.wand.found.as_ref().map(Install::trainerlib)
    }

    /// `choice`: the user picked this, so it is remembered once it checks out.
    fn show_wand_dir(&mut self, path: PathBuf, choice: bool, window: &mut Window, cx: &mut Context<Self>) {
        let text = path.to_string_lossy().into_owned();
        self.wand.input.update(cx, |input, cx| input.set_value(text, window, cx));
        self.check_wand_dir(choice, cx);
    }

    /// Resolves the field's text off the UI thread: it may be a slow network path.
    fn check_wand_dir(&mut self, choice: bool, cx: &mut Context<Self>) {
        if choice {
            self.wand.edits = self.wand.edits.wrapping_add(1);
        }
        // "Copy as path" in Explorer wraps the path in quotes.
        let path = PathBuf::from(self.wand.input.read(cx).value().trim().trim_matches('"'));
        self.wand.check = cx.spawn(async move |page, cx| {
            let found = {
                let path = path.clone();
                unblock(move || install::resolve(&path)).await
            };
            page.update(cx, |page, cx| page.wand_dir_checked(path, found, choice, cx)).ok();
        });
    }

    fn wand_dir_checked(&mut self, path: PathBuf, found: Option<Install>, choice: bool, cx: &mut Context<Self>) {
        if let Some(found) = found.as_ref().filter(|_| choice) {
            // The newest version is the default: choosing it, or a folder in it, clears the choice,
            // which keeps following updates.
            let newest = self.wand.installs.first().map(|install| &install.root);
            let wanted = (Some(&found.root) != newest).then_some(path);
            if self.preferences.wand_dir != wanted {
                self.preferences.wand_dir = wanted;
                let (dir, saved) = (self.data_dir.clone(), self.preferences.clone());
                let previous = mem::replace(&mut self.wand.save, Task::ready(()));
                self.wand.save = cx.background_spawn(async move {
                    previous.await;
                    if let Err(e) = unblock(move || preferences::save(&dir, &saved)).await {
                        log::warn!("preferences not saved: {e:#}");
                    }
                });
            }
        }
        self.wand.found = found;
        cx.notify();
    }

    fn pick_wand_version(&mut self, at: usize, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(install) = self.wand.installs.get(at) {
            self.show_wand_dir(install.root.clone(), true, window, cx);
        }
    }

    fn browse_wand(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let picked = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some("Select the Wand folder".into()),
        });
        cx.spawn_in(window, async move |page, cx| match picked.await {
            Ok(Ok(Some(paths))) => {
                if let Some(path) = paths.into_iter().next() {
                    page.update_in(cx, |page, window, cx| page.show_wand_dir(path, true, window, cx)).ok();
                }
            }
            Ok(Err(e)) => log::warn!("folder picker: {e:#}"),
            _ => {}
        })
        .detach();
    }

    pub(super) fn wand_dir_row(&self, cx: &mut Context<Self>) -> Div {
        let wand = &self.wand;
        let versions: Vec<SharedString> = wand.installs.iter().map(|install| install.label().into()).collect();
        let current = wand
            .found
            .as_ref()
            .and_then(|found| wand.installs.iter().position(|install| install.root == found.root))
            .unwrap_or(usize::MAX);
        let edge = |button: gpui_kit::Stateful<Div>| button.border_l_1().border_color(line(0.14));
        let field = h_flex()
            .id("wand-dir")
            .h(px(32.))
            .border_1()
            .border_color(line(0.14))
            .bg(rgb(SUNKEN))
            .child(
                div()
                    .id("wand-dir-text")
                    .flex_1()
                    .min_w_0()
                    .px(px(10.))
                    // The field is inside the list's trigger: clicking into it must not open the list.
                    .on_click(|_, _, cx| cx.stop_propagation())
                    .child(
                        Input::new(&wand.input)
                            .appearance(false)
                            .px_0()
                            .font_family(MONO)
                            .text_size(px(12.5)),
                    ),
            )
            .when(!versions.is_empty(), |field| {
                field.child(edge(icon_button("wand-versions", Icon::ChevronDown, 30., 12.)))
            })
            .child(edge(icon_button("wand-browse", Icon::Folder, 30., 14.)).on_click(cx.listener(
                |page, _, window, cx| {
                    cx.stop_propagation();
                    page.browse_wand(window, cx);
                },
            )));
        let field = self.dropdown(
            field,
            Menu::WandVersions,
            versions,
            current,
            cx,
            |page, at, window, cx| page.pick_wand_version(at, window, cx),
        );
        let status = match &wand.found {
            Some(found) => mono(format!("WAND {}", found.label()), 11.).text_color(rgb(self.accent)),
            None => mono("NO WAND TRAINER FILES IN THIS FOLDER", 11.).text_color(rgb(WARN)),
        };
        v_flex()
            .gap(px(8.))
            .py(px(12.))
            .child(label("WAND FOLDER", "where the trainer binaries are read from"))
            .child(field)
            .child(status)
    }
}
