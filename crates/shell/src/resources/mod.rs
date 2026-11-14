//! What Impulse keeps on disk, by kind: sizes, clearing, and the limit on media.
//! The Settings RESOURCES tab shows it (`ResourcesView`).

mod view;

use std::{sync::Arc, time::Duration};

use blocking::unblock;
use gpui_kit::{Context, EventEmitter, Task};
use model::{Clear, Ref, SourceId, Store};

use client::{
    Registry,
    media::{Media, limit},
    storage::{Paths, Stored},
};

pub use view::ResourcesView;

const SWEEP_EVERY: Duration = Duration::from_secs(600);

pub enum ResourcesEvent {
    /// A source's store was emptied: what came from it is asked for again.
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
    registry: Arc<Registry>,
    paths: Paths,
    usage: Option<Usage>,
    kept: Stored<limit::Kept>,
    _measure: Task<()>,
    _sweep: Task<()>,
}

impl EventEmitter<ResourcesEvent> for Resources {}

impl Resources {
    pub fn new(registry: Arc<Registry>, paths: &Paths, cx: &mut Context<Self>) -> Self {
        let kept = Stored::load(paths.resources());
        let mut resources = Self { registry, paths: paths.clone(), usage: None, kept, _measure: Task::ready(()), _sweep: Task::ready(()) };
        resources.keep_limit(cx);
        resources
    }

    /// Reads every size again.
    pub fn measure(&mut self, cx: &mut Context<Self>) {
        let (stores, paths) = (self.registry.stores(), self.paths.clone());
        self._measure = cx.spawn(async move |view, cx| {
            let own = unblock(move || {
                let media = Media::ALL.map(|media| {
                    let (_, name, hint) = describe(media);
                    Store { id: store_ref(media), name, hint, bytes: disk::size(&media.dir(&paths)), clear: Clear::Now }
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

    pub fn limit(&self) -> Option<u64> {
        self.kept.limit
    }

    /// Empties the stores `ids` name, then reads the sizes again: the app's media here, the rest through their source.
    pub fn clear(&mut self, ids: Vec<Ref>, cx: &mut Context<Self>) {
        let (registry, paths) = (self.registry.clone(), self.paths.clone());
        cx.spawn(async move |view, cx| {
            let mut provided = false;
            for id in ids {
                match media_of(&id) {
                    Some(media) => {
                        let dir = media.dir(&paths);
                        unblock(move || std::fs::remove_dir_all(dir)).await.ok();
                    }
                    None => {
                        registry.clear(&id).await;
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

    pub fn set_limit(&mut self, limit: Option<u64>, cx: &mut Context<Self>) {
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
fn store_ref(media: Media) -> Ref {
    Ref::new(&SourceId::app(), describe(media).0)
}

fn media_of(id: &Ref) -> Option<Media> {
    let app = *id.source() == SourceId::app();
    Media::ALL.into_iter().find(|media| app && describe(*media).0 == id.id())
}

fn describe(media: Media) -> (&'static str, &'static str, &'static str) {
    match media {
        Media::Pictures => ("pictures", "PICTURES", "game art and pin pictures"),
        Media::Tiles => ("tiles", "MAP TILES", "the ground of the maps you opened"),
        Media::Videos => ("videos", "VIDEOS", "pin clips"),
    }
}
