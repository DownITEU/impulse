//! Every source's titles (M2, M3): the disk copies read at start and on a language change; the sources are asked
//! again only by `refresh`.

use std::sync::Arc;

use client::Registry;
use futures::StreamExt;
use gpui_kit::{Context, EventEmitter, Task};
use model::{Locale, Snapshot, Title};

use crate::ui::text::{Texts, content_locale};

pub enum TitlesEvent {
    /// A read or a refresh ended, with titles or none.
    Changed,
}

pub struct Titles {
    registry: Arc<Registry>,
    all: Vec<Title>,
    locale: Option<Locale>,
    /// The disk copies of `locale` are read: a game without a match before then is not a miss.
    ready: bool,
    refreshing: bool,
    _ask: Task<()>,
}

impl EventEmitter<TitlesEvent> for Titles {}

impl Titles {
    pub fn new(registry: Arc<Registry>, cx: &mut Context<Self>) -> Self {
        cx.observe_global::<Texts>(|titles, cx| {
            if titles.locale.as_ref() != Some(&content_locale(cx)) {
                titles.ask(false, cx);
            }
        })
        .detach();
        let mut titles = Self { registry, all: Vec::new(), locale: None, ready: false, refreshing: false, _ask: Task::ready(()) };
        titles.ask(false, cx);
        titles
    }

    pub fn all(&self) -> &[Title] {
        &self.all
    }

    /// The disk copies of `locale` are read; a language change may not have reached this yet.
    pub fn ready(&self, locale: &Locale) -> bool {
        self.ready && self.locale.as_ref() == Some(locale)
    }

    /// Asks each source again: one calls only when its copy is stale.
    pub fn refresh(&mut self, cx: &mut Context<Self>) {
        if !self.refreshing {
            self.ask(true, cx);
        }
    }

    /// `fresh`: past the disk copy, as far as each source goes; else the copy alone, and no source is called.
    fn ask(&mut self, fresh: bool, cx: &mut Context<Self>) {
        let locale = content_locale(cx);
        if self.locale.as_ref() != Some(&locale) {
            (self.all, self.ready) = (Vec::new(), false);
        }
        let streams: Vec<_> = self.registry.titled().iter().map(|source| self.registry.titles(source, &locale)).collect();
        (self.locale, self.refreshing) = (Some(locale), fresh);
        self._ask = cx.spawn(async move |titles, cx| {
            let mut all = Vec::new();
            for mut snapshots in streams {
                let mut kept = None;
                while let Some(snapshot) = snapshots.next().await {
                    match snapshot {
                        Snapshot::Cached(copy) => kept = copy,
                        Snapshot::Fresh(Ok(list)) => kept = Some(list),
                        Snapshot::Fresh(Err(_)) => {}
                    }
                    if !fresh {
                        break;
                    }
                }
                all.extend(kept.into_iter().flatten());
            }
            titles
                .update(cx, |titles, cx| {
                    (titles.all, titles.ready, titles.refreshing) = (all, true, false);
                    cx.emit(TitlesEvent::Changed);
                    cx.notify();
                })
                .ok();
        });
    }
}
