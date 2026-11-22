pub mod account;
mod api;
mod vault;

use std::{
    fmt,
    path::PathBuf,
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result};
use blocking::unblock;
use serde::{Deserialize, Serialize, de::DeserializeOwned};

use crate::{
    store::{self, WandToken},
    transport::Transport,
};

const CLIENT_ID: &str = "overlay";
// Refresh with under a day left, so a running app does not hit expiry mid-use.
const REFRESH_MARGIN: u64 = 86_400;

#[derive(Default, Serialize, Deserialize)]
pub struct Session {
    access_token: String,
    refresh_token: String,
    expires_at: u64,
    // The API refused the access token before its expiry: the next connect refreshes it.
    #[serde(skip)]
    rejected: AtomicBool,
}

impl fmt::Debug for Session {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Session(<redacted>)")
    }
}

#[derive(Clone, Copy)]
pub enum Source {
    Restored,
    Refreshed,
    Wand,
}

#[derive(Deserialize)]
struct Code {
    code: String,
}

#[derive(Deserialize)]
struct Token {
    access_token: String,
    refresh_token: String,
    expires_in: u64,
}

impl From<Token> for Session {
    fn from(t: Token) -> Self {
        Self { access_token: t.access_token, refresh_token: t.refresh_token, expires_at: now() + t.expires_in, ..Self::default() }
    }
}

/// Stored session, refreshed when close to expiry or `refresh`; a new one from Wand only when there is none or it was revoked.
pub async fn connect(t: &Transport, vault: PathBuf, refresh: bool) -> Result<(Session, Source)> {
    let stored = {
        let vault = vault.clone();
        unblock(move || vault::load(&vault)).await
    };
    let (session, source) = match stored {
        Some(s) if !refresh && s.expires_at > now() + REFRESH_MARGIN => return Ok((s, Source::Restored)),
        Some(s) => match s.refresh(t).await {
            Ok(s) => (s, Source::Refreshed),
            // Network errors must not burn a new Wand handoff; only a rejected refresh token does.
            Err(e) if api::is_rejected(&e) => (from_wand(t).await?, Source::Wand),
            Err(e) => return Err(e),
        },
        None => (from_wand(t).await?, Source::Wand),
    };
    // A failed save keeps the session usable for this run; the next start falls back to Wand.
    let (session, saved) = unblock(move || {
        let saved = vault::save(&vault, &session);
        (session, saved)
    })
    .await;
    if let Err(e) = saved {
        log::warn!("session not saved: {e:#}");
    }
    Ok((session, source))
}

async fn from_wand(t: &Transport) -> Result<Session> {
    let wand = unblock(store::read).await?;
    sign_in(t, wand).await
}

/// Wand's access token only mints the overlay code; the resulting session is Impulse's own.
async fn sign_in(t: &Transport, wand: WandToken) -> Result<Session> {
    let Code { code } = api::call(t, "POST", "/v3/auth/overlay_code", Some(wand.expose()), None, 201).await?;
    let form = [("grant_type", "tiny_code"), ("code", code.as_str()), ("client_id", CLIENT_ID)];
    let token: Token = api::call(t, "POST", "/auth/token", None, Some(&form), 200).await.context("exchange Wand sign-in code")?;
    Ok(token.into())
}

impl Session {
    pub fn expires_in(&self) -> Duration {
        Duration::from_secs(self.expires_at.saturating_sub(now()))
    }

    async fn refresh(&self, t: &Transport) -> Result<Self> {
        let form = [("grant_type", "refresh_token"), ("refresh_token", self.refresh_token.as_str()), ("client_id", CLIENT_ID)];
        let token: Token = api::call(t, "POST", "/auth/token", None, Some(&form), 200).await.context("refresh Impulse session")?;
        Ok(token.into())
    }

    /// Authorized GET of a JSON resource.
    pub async fn get<T: DeserializeOwned>(&self, t: &Transport, path: &str) -> Result<T> {
        let got = api::call(t, "GET", path, Some(&self.access_token), None, 200).await;
        if got.as_ref().is_err_and(api::is_unauthorized) {
            self.rejected.store(true, Ordering::Relaxed);
        }
        got
    }

    pub fn is_rejected(&self) -> bool {
        self.rejected.load(Ordering::Relaxed)
    }
}

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_is_redacted() {
        let session =
            Session { access_token: "secret-access".to_owned(), refresh_token: "secret-refresh".to_owned(), ..Session::default() };
        assert_eq!(format!("{session:?}"), "Session(<redacted>)");
    }
}
