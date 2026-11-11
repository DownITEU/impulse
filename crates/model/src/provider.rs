//! Where games, trainers and the account come from.

use std::path::PathBuf;

use anyhow::Result;
use futures::{future::BoxFuture, stream::BoxStream};

use crate::{Game, Installation, Map, MapOffer, Trainer};

pub trait Provider: Send + Sync {
    fn sign_in(&self) -> BoxStream<'static, SignIn>;
    fn games(&self) -> BoxStream<'static, Snapshot<Vec<Game>>>;
    fn trainer(&self, game: &Game, install: &Installation) -> BoxStream<'static, Snapshot<Trainer>>;
    fn map(&self, offer: &MapOffer) -> BoxStream<'static, Snapshot<Map>>;
    fn folders(&self) -> BoxFuture<'static, Vec<Folder>>;
    /// Saves `path` when its status is `Ok`. Calls are ordered: an older one never saves over a newer.
    fn set_folder(&self, id: &'static str, path: PathBuf) -> BoxFuture<'static, Folder>;
    /// What it keeps on disk (sizes are read when asked).
    fn stores(&self) -> BoxFuture<'static, Vec<Store>>;
    /// Empties the store `id` names; the next use downloads it again.
    fn clear(&self, id: &'static str) -> BoxFuture<'static, ()>;
}

/// Files a provider keeps, of one kind.
pub struct Store {
    pub id: &'static str,
    pub name: &'static str,
    pub hint: &'static str,
    pub bytes: u64,
    pub clear: Clear,
}

#[derive(Clone, Copy, PartialEq)]
pub enum Clear {
    Now,
    /// Only while nothing uses it: at the next start. `asked` once it is due.
    NextStart { asked: bool },
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
