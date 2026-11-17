//! Sources coming, going or replaced while the app runs (plugins), told once per burst of registry changes.

use client::{Change, Registry};
use futures::StreamExt;
use gpui_kit::{Context, EventEmitter, Task};
use model::SourceId;

/// The sources that came, went or were replaced.
pub struct SourcesChanged(pub Vec<SourceId>);

pub struct Sources {
    _watch: Task<()>,
}

impl EventEmitter<SourcesChanged> for Sources {}

impl Sources {
    pub fn new(registry: &Registry, cx: &mut Context<Self>) -> Self {
        let mut changes = registry.changes();
        let _watch = cx.spawn(async move |sources, cx| {
            while let Some(change) = changes.next().await {
                let mut changed = Vec::new();
                let mut note = |change: Change| {
                    let (Change::Added(id) | Change::Removed(id)) = change;
                    if !changed.contains(&id) {
                        changed.push(id);
                    }
                };
                note(change);
                while let Ok(change) = changes.try_recv() {
                    note(change);
                }
                if sources.update(cx, |_, cx| cx.emit(SourcesChanged(changed))).is_err() {
                    return;
                }
            }
        });
        Self { _watch }
    }
}
