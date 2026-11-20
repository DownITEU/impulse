//! The source's session: the account it signed in, or that it could not.

use std::sync::Arc;

use client::Registry;
use futures::StreamExt;
use gpui_kit::{Context, Entity, EventEmitter, Task};
use model::SignIn;

use crate::library::{Library, LibraryEvent};

/// A sign-in after a failed one got a session.
pub struct SignedIn;

pub struct Account {
    registry: Arc<Registry>,
    user: Option<model::Account>,
    signed_out: bool,
    _sign_in: Task<()>,
}

impl EventEmitter<SignedIn> for Account {}

impl Account {
    pub fn new(registry: Arc<Registry>, library: &Entity<Library>, cx: &mut Context<Self>) -> Self {
        // A game's trainer needs the session: opening one while signed out asks again.
        cx.subscribe(library, |account, _, event: &LibraryEvent, cx| {
            if account.signed_out && matches!(event, LibraryEvent::Opened) {
                account.reload(cx);
            }
        })
        .detach();
        let mut account = Self { registry, user: None, signed_out: false, _sign_in: Task::ready(()) };
        account.reload(cx);
        account
    }

    pub fn user(&self) -> Option<&model::Account> {
        self.user.as_ref()
    }

    /// No session, as last asked.
    pub fn signed_out(&self) -> bool {
        self.signed_out
    }

    /// Asks for the session again; what shows stays until the answer.
    pub fn reload(&mut self, cx: &mut Context<Self>) {
        let mut sign_in = self.registry.sign_in();
        self._sign_in = cx.spawn(async move |account, cx| {
            let mut failed = false;
            while let Some(state) = sign_in.next().await {
                let shown = account.update(cx, |account, cx| {
                    match state {
                        SignIn::Account(user) => account.user = Some(user),
                        SignIn::Failed => failed = true,
                    }
                    cx.notify();
                });
                if shown.is_err() {
                    return;
                }
            }
            // The stream ends once the session is known.
            let _ = account.update(cx, |account, cx| {
                if account.signed_out && !failed {
                    cx.emit(SignedIn);
                }
                account.signed_out = failed;
                cx.notify();
            });
        });
    }
}
