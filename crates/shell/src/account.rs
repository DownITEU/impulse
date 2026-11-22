//! The source's session: the account it signed in, or that it could not.

use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use client::Registry;
use futures::StreamExt;
use gpui_kit::{Context, Entity, EventEmitter, Task};
use model::{Failure, SignIn};

use crate::library::{Library, LibraryEvent};

const SHOWN_TRYING: Duration = Duration::from_millis(600);

/// A sign-in after a failed one got a session.
pub struct SignedIn;

pub struct Account {
    registry: Arc<Registry>,
    user: Option<model::Account>,
    /// Why the last sign-in got no session.
    failure: Option<Failure>,
    trying: bool,
    _sign_in: Task<()>,
}

impl EventEmitter<SignedIn> for Account {}

impl Account {
    pub fn new(registry: Arc<Registry>, library: &Entity<Library>, cx: &mut Context<Self>) -> Self {
        // A game's trainer needs the session: opening one while signed out asks again.
        cx.subscribe(library, |account, _, event: &LibraryEvent, cx| {
            if account.signed_out() && matches!(event, LibraryEvent::Opened) {
                account.reload(cx);
            }
        })
        .detach();
        let mut account = Self { registry, user: None, failure: None, trying: false, _sign_in: Task::ready(()) };
        account.reload(cx);
        account
    }

    pub fn user(&self) -> Option<&model::Account> {
        self.user.as_ref()
    }

    /// No session, as last asked.
    pub fn signed_out(&self) -> bool {
        self.failure.is_some()
    }

    pub fn failure(&self) -> Option<&Failure> {
        self.failure.as_ref()
    }

    /// A sign-in is under way.
    pub fn trying(&self) -> bool {
        self.trying
    }

    /// Asks for the session again; what shows stays until the answer.
    pub fn reload(&mut self, cx: &mut Context<Self>) {
        let mut sign_in = self.registry.sign_in();
        self.trying = true;
        cx.notify();
        let started = Instant::now();
        self._sign_in = cx.spawn(async move |account, cx| {
            let mut failed = None;
            while let Some(state) = sign_in.next().await {
                let shown = account.update(cx, |account, cx| {
                    match state {
                        SignIn::Account(user) => account.user = Some(user),
                        SignIn::Failed(failure) => failed = Some(failure),
                    }
                    cx.notify();
                });
                if shown.is_err() {
                    return;
                }
            }
            // The stream ends once the session is known. A retry that fails at once would not show it was tried.
            if let Some(left) = SHOWN_TRYING.checked_sub(started.elapsed()) {
                cx.background_executor().timer(left).await;
            }
            let _ = account.update(cx, |account, cx| {
                if account.signed_out() && failed.is_none() {
                    cx.emit(SignedIn);
                }
                (account.failure, account.trying) = (failed, false);
                cx.notify();
            });
        });
    }
}
