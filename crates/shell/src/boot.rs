//! The session from the provider, and filling the page anew after its caches were cleared.

use futures::StreamExt;
use gpui_kit::{Context, Window};
use model::SignIn;

use super::Page;

impl Page {
    pub(super) fn sign_in(&self, window: &Window, cx: &mut Context<Self>) {
        let mut sign_in = self.provider.sign_in();
        cx.spawn_in(window, async move |page, cx| {
            while let Some(state) = sign_in.next().await {
                page.update(cx, |page, cx| {
                    match state {
                        SignIn::Account(account) => page.account = Some(account),
                        SignIn::Failed => page.signed_out = true,
                    }
                    cx.notify();
                })
                .ok();
            }
        })
        .detach();
    }

    /// Fills the page anew after a provider's store was cleared. The session stays: it is not a cache.
    pub(super) fn reload(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        log::info!("caches cleared");
        self.library.update(cx, |library, cx| library.reload(cx));
        // The session is asked for again.
        self.signed_out = false;
        // The run keeps what it leaves on first, from the trainers about to go.
        self.runs.update(cx, |runs, cx| runs.stop(cx));
        self.trainers.update(cx, |trainers, cx| trainers.clear(cx));
        self.sign_in(window, cx);
        cx.notify();
    }
}
