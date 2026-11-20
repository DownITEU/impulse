//! Every source's titles: the disk copies read at start and on a language change; the sources are asked
//! again only by `refresh`.

use std::sync::Arc;

use client::Registry;
use futures::StreamExt;
use gpui_kit::{Context, Entity, EventEmitter, Task};
use model::{Locale, Snapshot, SourceId, Text, Title};

use crate::{
    sources::{Sources, SourcesChanged},
    ui::text::{Texts, content_locale},
};

pub enum TitlesEvent {
    /// A read or a refresh ended, with titles or none.
    Changed,
}

pub struct Titles {
    registry: Arc<Registry>,
    all: Vec<Title>,
    // What the titled sources are called, where the picker tells them apart.
    names: Vec<(SourceId, Text)>,
    locale: Option<Locale>,
    /// The disk copies of `locale` are read: a game without a match before then is not a miss.
    ready: bool,
    refreshing: bool,
    // A titled source gave no copy at all yet: only a refresh can tell.
    pending: bool,
    _ask: Task<()>,
}

impl EventEmitter<TitlesEvent> for Titles {}

impl Titles {
    pub fn new(registry: Arc<Registry>, sources: &Entity<Sources>, cx: &mut Context<Self>) -> Self {
        // A source gone takes its titles at once; a new one's are read from its disk copy.
        cx.subscribe(sources, |titles, _, _: &SourcesChanged, cx| {
            let registry = titles.registry.clone();
            titles.all.retain(|title| registry.has(title.id.source()));
            // Keeps a running refresh running instead of cancelling it.
            titles.ask(titles.refreshing, cx);
        })
        .detach();
        cx.observe_global::<Texts>(|titles, cx| {
            if titles.locale.as_ref() != Some(&content_locale(cx)) {
                titles.ask(false, cx);
            }
        })
        .detach();
        let mut titles = Self {
            registry,
            all: Vec::new(),
            names: Vec::new(),
            locale: None,
            ready: false,
            refreshing: false,
            pending: false,
            _ask: Task::ready(()),
        };
        titles.ask(false, cx);
        titles
    }

    pub fn all(&self) -> &[Title] {
        &self.all
    }

    pub fn name(&self, source: &SourceId) -> Option<&Text> {
        self.names.iter().find(|(id, _)| id == source).map(|(_, name)| name)
    }

    /// The disk copies of `locale` are read; a language change may not have reached this yet.
    pub fn ready(&self, locale: &Locale) -> bool {
        self.ready && self.locale.as_ref() == Some(locale)
    }

    /// A titled source has no copy, not even an empty one: its titles are unknown until a refresh.
    pub fn pending(&self) -> bool {
        self.pending
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
        let titled = self.registry.titled();
        self.names = titled.iter().filter_map(|source| Some((source.clone(), self.registry.name(source)?))).collect();
        let streams: Vec<_> = titled.iter().map(|source| self.registry.titles(source, &locale)).collect();
        (self.locale, self.refreshing) = (Some(locale), fresh);
        self._ask = cx.spawn(async move |titles, cx| {
            let (mut all, mut pending) = (Vec::new(), false);
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
                pending |= kept.is_none();
                all.extend(kept.into_iter().flatten());
            }
            titles
                .update(cx, |titles, cx| {
                    (titles.all, titles.ready, titles.refreshing, titles.pending) = (all, true, false, pending);
                    cx.emit(TitlesEvent::Changed);
                    cx.notify();
                })
                .ok();
        });
    }
}
