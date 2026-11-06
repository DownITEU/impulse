mod api;
mod vault;

use std::{
    fmt,
    path::PathBuf,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use anyhow::Result;
use blocking::unblock;
use serde::{Deserialize, Serialize, de::IgnoredAny};

use crate::{
    transport::Transport,
    wand::store::{self, WandToken},
};

const CLIENT_ID: &str = "overlay";
// Refresh with under a day left, so a running app does not hit expiry mid-use.
const REFRESH_MARGIN: u64 = 86_400;

#[derive(Serialize, Deserialize)]
pub struct Session {
    access_token: String,
    refresh_token: String,
    expires_at: u64,
}

impl fmt::Debug for Session {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Session(<redacted>)")
    }
}

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
        Self {
            access_token: t.access_token,
            refresh_token: t.refresh_token,
            expires_at: now() + t.expires_in,
        }
    }
}

/// Stored session, refreshed when close to expiry; a new one from Wand only when there is none or it was revoked.
pub async fn connect(t: &Transport, vault: PathBuf) -> Result<(Session, Source)> {
    let stored = {
        let vault = vault.clone();
        unblock(move || vault::load(&vault)).await
    };
    let (session, source) = match stored {
        Some(s) if s.expires_at > now() + REFRESH_MARGIN => return Ok((s, Source::Restored)),
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
        eprintln!("session not saved: {e:#}");
    }
    Ok((session, source))
}

async fn from_wand(t: &Transport) -> Result<Session> {
    let wand = unblock(store::read).await?;
    sign_in(t, wand).await
}

/// Wand's access token only mints the overlay code; the resulting session is Impulse's own.
async fn sign_in(t: &Transport, wand: WandToken) -> Result<Session> {
    let Code { code } = api::call(
        t,
        "POST",
        "/v3/auth/overlay_code",
        Some(wand.expose()),
        None,
        201,
    )
    .await?;
    let form = [
        ("grant_type", "tiny_code"),
        ("code", code.as_str()),
        ("client_id", CLIENT_ID),
    ];
    let token: Token = api::call(t, "POST", "/auth/token", None, Some(&form), 200).await?;
    Ok(token.into())
}

impl Session {
    pub fn expires_in(&self) -> Duration {
        Duration::from_secs(self.expires_at.saturating_sub(now()))
    }

    async fn refresh(&self, t: &Transport) -> Result<Self> {
        let form = [
            ("grant_type", "refresh_token"),
            ("refresh_token", self.refresh_token.as_str()),
            ("client_id", CLIENT_ID),
        ];
        let token: Token = api::call(t, "POST", "/auth/token", None, Some(&form), 200).await?;
        Ok(token.into())
    }

    /// Enshrouded (82102) trainer count, proving the session works.
    pub async fn check(&self, t: &Transport) -> Result<usize> {
        let trainers: Vec<IgnoredAny> = api::call(
            t,
            "GET",
            "/v3/games/82102/trainers",
            Some(&self.access_token),
            None,
            200,
        )
        .await?;
        Ok(trainers.len())
    }
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_is_redacted() {
        let session = Session {
            access_token: "secret-access".to_owned(),
            refresh_token: "secret-refresh".to_owned(),
            expires_at: 0,
        };
        assert_eq!(format!("{session:?}"), "Session(<redacted>)");
    }
}
