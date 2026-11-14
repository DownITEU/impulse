//! Filling the page anew after a provider's caches were cleared.

use gpui_kit::Context;

use super::Page;

impl Page {
    /// The session stays: it is not a cache, but it is asked for again.
    pub(super) fn reload(&mut self, cx: &mut Context<Self>) {
        log::info!("caches cleared");
        self.library.update(cx, |library, cx| library.reload(cx));
        self.account.update(cx, |account, cx| account.reload(cx));
        // The run keeps what it leaves on first, from the trainers about to go.
        self.runs.update(cx, |runs, cx| runs.stop(cx));
        self.trainers.update(cx, |trainers, cx| trainers.clear(cx));
        cx.notify();
    }
}
