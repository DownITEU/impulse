//! The source's session: the account it signed in, or that it could not.

use std::sync::Arc;

use client::Registry;
use futures::StreamExt;
use gpui_kit::{Context, Task};
use model::SignIn;

pub struct Account {
    registry: Arc<Registry>,
    user: Option<model::Account>,
    signed_out: bool,
    _sign_in: Task<()>,
}

impl Account {
    pub fn new(registry: Arc<Registry>, cx: &mut Context<Self>) -> Self {
        let mut account = Self { registry, user: None, signed_out: false, _sign_in: Task::ready(()) };
        account.sign_in(cx);
        account
    }

    pub fn user(&self) -> Option<&model::Account> {
        self.user.as_ref()
    }

    /// No session, as last asked.
    pub fn signed_out(&self) -> bool {
        self.signed_out
    }

    /// Asks for the session again.
    pub fn reload(&mut self, cx: &mut Context<Self>) {
        self.signed_out = false;
        self.sign_in(cx);
        cx.notify();
    }

    fn sign_in(&mut self, cx: &mut Context<Self>) {
        let mut sign_in = self.registry.sign_in();
        self._sign_in = cx.spawn(async move |account, cx| {
            while let Some(state) = sign_in.next().await {
                let known = account.update(cx, |account, cx| {
                    match state {
                        SignIn::Account(user) => account.user = Some(user),
                        SignIn::Failed => account.signed_out = true,
                    }
                    cx.notify();
                });
                if known.is_err() {
                    break;
                }
            }
        });
    }
}
