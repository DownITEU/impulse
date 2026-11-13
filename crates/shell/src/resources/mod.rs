//! What Impulse keeps on disk, by kind: sizes, clearing, and the limit on media.
//! An entity of its own; the Settings RESOURCES tab shows it.

mod limit;
mod view;

use std::{sync::Arc, time::Duration};

use blocking::unblock;
use gpui_kit::{Context, EventEmitter, Task};
use model::{Clear, Provider, Store};

use crate::{
    art::Media,
    storage::{Paths, Stored},
};

const SWEEP_EVERY: Duration = Duration::from_secs(600);

pub enum ResourcesEvent {
    /// A provider's store was emptied: what came from it is asked for again.
    Cleared,
}

/// Sizes as last read.
struct Usage {
    caches: Vec<Store>,
    progress: u64,
    hotkeys: u64,
    logs: u64,
}

pub struct Resources {
    provider: Arc<dyn Provider>,
    paths: Paths,
    usage: Option<Usage>,
    kept: Stored<limit::Kept>,
    _measure: Task<()>,
    _sweep: Task<()>,
}

impl EventEmitter<ResourcesEvent> for Resources {}

impl Resources {
    pub fn new(provider: Arc<dyn Provider>, paths: &Paths, cx: &mut Context<Self>) -> Self {
        let kept = Stored::load(paths.resources());
        let mut resources = Self { provider, paths: paths.clone(), usage: None, kept, _measure: Task::ready(()), _sweep: Task::ready(()) };
        resources.keep_limit(cx);
        resources
    }

    /// Reads every size again.
    pub fn measure(&mut self, cx: &mut Context<Self>) {
        let (stores, paths) = (self.provider.stores(), self.paths.clone());
        self._measure = cx.spawn(async move |view, cx| {
            let own = unblock(move || {
                let media = Media::ALL.map(|media| {
                    let (id, name, hint) = describe(media);
                    Store { id, name, hint, bytes: disk::size(&media.dir(&paths)), clear: Clear::Now }
                });
                let logs = journal::dir().map_or(0, disk::size);
                (media, disk::size(&paths.map_state()), disk::size(&paths.hotkeys()), logs)
            });
            let ((media, progress, hotkeys, logs), provided) = (own.await, stores.await);
            let usage = Usage { caches: media.into_iter().chain(provided).collect(), progress, hotkeys, logs };
            view.update(cx, |view, cx| {
                view.usage = Some(usage);
                cx.notify();
            })
            .ok();
        });
    }

    /// Empties the stores `ids` name, then reads the sizes again.
    fn clear(&mut self, ids: Vec<&'static str>, cx: &mut Context<Self>) {
        let (provider, paths) = (self.provider.clone(), self.paths.clone());
        cx.spawn(async move |view, cx| {
            let mut provided = false;
            for id in ids {
                match Media::ALL.into_iter().find(|media| describe(*media).0 == id) {
                    Some(media) => {
                        let dir = media.dir(&paths);
                        unblock(move || std::fs::remove_dir_all(dir)).await.ok();
                    }
                    None => {
                        provider.clear(id).await;
                        provided = true;
                    }
                }
            }
            view.update(cx, |view, cx| {
                if provided {
                    cx.emit(ResourcesEvent::Cleared);
                }
                view.measure(cx);
            })
            .ok();
        })
        .detach();
    }

    fn set_limit(&mut self, limit: Option<u64>, cx: &mut Context<Self>) {
        self.kept.edit(|kept| kept.limit = limit);
        self.keep_limit(cx);
        cx.notify();
    }

    /// Holds the media under the limit: now, then every few minutes, and measures again after.
    fn keep_limit(&mut self, cx: &mut Context<Self>) {
        let Some(limit) = self.kept.limit else {
            self._sweep = Task::ready(());
            return;
        };
        let dirs = Media::ALL.map(|media| media.dir(&self.paths));
        self._sweep = cx.spawn(async move |view, cx| {
            loop {
                let dirs = dirs.clone();
                unblock(move || limit::sweep(&dirs, limit)).await;
                if view.update(cx, |view, cx| view.usage.is_some().then(|| view.measure(cx))).is_err() {
                    return;
                }
                cx.background_executor().timer(SWEEP_EVERY).await;
            }
        });
    }
}

/// A media folder's store id, name and what it holds.
fn describe(media: Media) -> (&'static str, &'static str, &'static str) {
    match media {
        Media::Pictures => ("pictures", "PICTURES", "game art and pin pictures"),
        Media::Tiles => ("tiles", "MAP TILES", "the ground of the maps you opened"),
        Media::Videos => ("videos", "VIDEOS", "pin clips"),
    }
}
