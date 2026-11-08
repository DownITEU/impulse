//! Where games, trainers and the account come from.

use std::path::PathBuf;

use anyhow::Result;
use futures::{future::BoxFuture, stream::BoxStream};

use crate::{Game, Installation, Trainer};

pub trait Provider: Send + Sync {
    fn sign_in(&self) -> BoxStream<'static, SignIn>;
    fn games(&self) -> BoxStream<'static, Snapshot<Vec<Game>>>;
    fn trainer(&self, game: &Game, install: &Installation) -> BoxStream<'static, Snapshot<Trainer>>;
    fn folders(&self) -> BoxFuture<'static, Vec<Folder>>;
    /// Saves `path` when its status is `Ok`. Calls are ordered: an older one never saves over a newer.
    fn set_folder(&self, id: &'static str, path: PathBuf) -> BoxFuture<'static, Folder>;
    /// Blocking.
    fn forget_caches(&self);
}

/// A snapshot stream yields `Cached` first, then `Fresh` only when the copy was missing or stale.
pub enum Snapshot<T> {
    /// The disk copy, if any.
    Cached(Option<T>),
    /// An `Err` keeps the cached value.
    Fresh(Result<T>),
}

pub enum SignIn {
    /// Cached first, then fresh.
    Account(Account),
    /// No session: signed out, offline or the provider cannot start. The provider logs why.
    Failed,
}

pub struct Account {
    pub name: String,
}

/// A folder a provider reads from, set in Settings.
pub struct Folder {
    pub id: &'static str,
    pub label: &'static str,
    pub hint: &'static str,
    /// The saved choice, else the provider's default.
    pub path: Option<PathBuf>,
    /// What `path` holds, or why it cannot be used.
    pub status: Result<String, String>,
    /// Ready picks: a label and a path.
    pub choices: Vec<(String, PathBuf)>,
    /// The pick `path` lies in.
    pub selected: Option<PathBuf>,
}
