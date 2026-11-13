//! Who is signed in. Kept on disk, so the name shows at once and offline.

use std::{fs, path::Path};

use anyhow::Result;
use blocking::unblock;
use serde::{Deserialize, Serialize};

use super::{Session, now};
use crate::transport::Transport;

const FILE: &str = "account.json";
// A name changes rarely; one request a day keeps it current.
const MAX_AGE: u64 = 86_400;

#[derive(Clone, Serialize, Deserialize)]
pub struct Account {
    pub username: String,
}

#[derive(Serialize, Deserialize)]
struct Stored {
    fetched_at: u64,
    account: Account,
}

/// The disk copy and whether it is recent enough to skip the request (blocking).
pub fn cached(data_dir: &Path) -> Option<(Account, bool)> {
    let stored: Stored = serde_json::from_slice(&fs::read(data_dir.join(FILE)).ok()?).ok()?;
    let fresh = now().saturating_sub(stored.fetched_at) < MAX_AGE;
    Some((stored.account, fresh))
}

pub fn size(data_dir: &Path) -> u64 {
    disk::size(&data_dir.join(FILE))
}

pub fn forget(data_dir: &Path) {
    let _ = fs::remove_file(data_dir.join(FILE));
}

pub async fn fetch(session: &Session, t: &Transport, data_dir: &Path) -> Result<Account> {
    let account: Account = session.get(t, "/v3/account").await?;
    let stored = Stored { fetched_at: now(), account: account.clone() };
    let file = data_dir.join(FILE);
    let saved = unblock(move || disk::replace(&file, &serde_json::to_vec(&stored)?)).await;
    if let Err(e) = saved {
        log::warn!("account not cached: {e:#}");
    }
    Ok(account)
}
