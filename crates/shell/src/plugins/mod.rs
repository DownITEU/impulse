//! The plugins the user installed: the list, installing what the user approved, enabling, removing, their settings.
//! Each command runs off the UI thread through `client::plugins`, one at a time, and the list is read again after.

mod fields;
mod view;

use std::{path::PathBuf, sync::Arc};

use blocking::unblock;
use gpui_kit::{Context, PathPromptOptions};
use model::{Approval, Failure, Installed, SourceId, Value};

use crate::ui::{key, text::t};
pub use view::PluginsView;

pub struct Plugins {
    plugins: Arc<client::plugins::Plugins>,
    list: Vec<Installed>,
    /// A pick waiting for the user's approval.
    approval: Option<(PathBuf, Approval)>,
    /// The last command's failure.
    failure: Option<Failure>,
    busy: bool,
}

impl Plugins {
    pub fn new(plugins: Arc<client::plugins::Plugins>, cx: &mut Context<Self>) -> Self {
        let mut this = Self { plugins, list: Vec::new(), approval: None, failure: None, busy: false };
        this.refresh(cx);
        this
    }

    pub fn list(&self) -> &[Installed] {
        &self.list
    }

    pub fn approval(&self) -> Option<&Approval> {
        self.approval.as_ref().map(|(_, approval)| approval)
    }

    pub fn failure(&self) -> Option<&Failure> {
        self.failure.as_ref()
    }

    /// Reads the list again: sizes and failures change on their own.
    pub fn refresh(&mut self, cx: &mut Context<Self>) {
        self.run(|_| Ok(()), cx);
    }

    /// Asks for a folder or a `.zip`, then what installing it would grant.
    pub fn pick(&mut self, zip: bool, cx: &mut Context<Self>) {
        let prompt = Some(t(cx, if zip { key::PLUGINS_PICK_ZIP } else { key::PLUGINS_PICK_FOLDER }));
        let picked = cx.prompt_for_paths(PathPromptOptions { files: zip, directories: !zip, multiple: false, prompt });
        let plugins = self.plugins.clone();
        cx.spawn(async move |this, cx| {
            let Ok(Ok(Some(mut paths))) = picked.await else { return };
            let Some(pick) = paths.pop() else { return };
            let inspected = unblock({
                let pick = pick.clone();
                move || plugins.inspect(&pick)
            })
            .await;
            this.update(cx, |this, cx| {
                match inspected {
                    Ok(approval) => (this.approval, this.failure) = (Some((pick, approval)), None),
                    Err(e) => (this.approval, this.failure) = (None, Some(e)),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// Installs the pending pick if it is the one the user saw: `digest` is the shown card's.
    pub fn approve(&mut self, digest: &str, cx: &mut Context<Self>) {
        if self.busy || self.approval.as_ref().is_none_or(|(_, approval)| approval.digest != digest) {
            return;
        }
        let Some((pick, approval)) = self.approval.take() else { return };
        self.run(move |plugins| plugins.install(&pick, &approval), cx);
    }

    pub fn cancel(&mut self, cx: &mut Context<Self>) {
        self.approval = None;
        cx.notify();
    }

    pub fn set_enabled(&mut self, id: SourceId, on: bool, cx: &mut Context<Self>) {
        self.run(move |plugins| plugins.set_enabled(&id, on), cx);
    }

    pub fn remove(&mut self, id: SourceId, cx: &mut Context<Self>) {
        self.run(move |plugins| plugins.remove(&id), cx);
    }

    pub fn set_value(&mut self, id: SourceId, setting: String, value: Value, cx: &mut Context<Self>) {
        self.run(move |plugins| plugins.set_value(&id, &setting, value), cx);
    }

    /// One command at a time, so the lock and the registry stay in step: a click while one runs is dropped.
    fn run(&mut self, command: impl FnOnce(&client::plugins::Plugins) -> Result<(), Failure> + Send + 'static, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        self.busy = true;
        let plugins = self.plugins.clone();
        cx.spawn(async move |this, cx| {
            let (done, list) = unblock(move || (command(&plugins), plugins.list())).await;
            this.update(cx, |this, cx| {
                (this.list, this.failure, this.busy) = (list, done.err(), false);
                cx.notify();
            })
            .ok();
        })
        .detach();
    }
}
