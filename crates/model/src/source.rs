//! Where games, trainers, maps and the account come from: a source, and what it can do, one trait each.

use std::path::PathBuf;

use futures::{future::BoxFuture, stream::BoxStream};

use crate::{Failure, Game, Installation, Locale, Map, MapOffer, Offer, Ref, SourceId, Text, Trainer};

pub trait Source: Send + Sync {
    /// The source of every `Ref` it mints, and the domain of its texts.
    fn id(&self) -> SourceId;

    fn texts(&self) -> Files {
        &[]
    }
}

/// Catalogs: a language tag and its Fluent source each.
pub type Files = &'static [(&'static str, &'static str)];

pub trait Accounts: Source {
    fn sign_in(&self) -> BoxStream<'static, SignIn>;
}

pub trait Games: Source {
    fn games(&self) -> BoxStream<'static, Snapshot<Vec<Game>>>;
}

pub trait Trainers: Source {
    fn trainer(&self, offer: &Offer, install: &Installation, locale: &Locale) -> BoxStream<'static, Snapshot<Trainer>>;
}

pub trait Maps: Source {
    fn map(&self, offer: &MapOffer, locale: &Locale) -> BoxStream<'static, Snapshot<Map>>;
}

pub trait Folders: Source {
    fn folders(&self) -> BoxFuture<'static, Vec<Folder>>;
    /// Saves `path` when its status is `Ok`. Calls are ordered: an older one never saves over a newer.
    fn set_folder(&self, id: &Ref, path: PathBuf) -> BoxFuture<'static, Folder>;
}

pub trait Caches: Source {
    /// What it keeps on disk (sizes are read when asked).
    fn stores(&self) -> BoxFuture<'static, Vec<Store>>;
    /// Empties the store `id` names; the next use downloads it again.
    fn clear(&self, id: &Ref) -> BoxFuture<'static, ()>;
}

/// Files a source keeps, of one kind.
pub struct Store {
    pub id: Ref,
    pub name: Text,
    pub hint: Text,
    pub bytes: u64,
    pub clear: Clear,
}

#[derive(Clone, Copy, PartialEq)]
pub enum Clear {
    Now,
    /// Only while nothing uses it: at the next start. `asked` once it is due.
    NextStart {
        asked: bool,
    },
}

/// A snapshot stream yields `Cached` first, then `Fresh` only when the copy was missing or stale (a copy in another
/// locale is stale).
pub enum Snapshot<T> {
    /// The disk copy, if any.
    Cached(Option<T>),
    /// An `Err` keeps the cached value.
    Fresh(Result<T, Failure>),
}

pub enum SignIn {
    /// Cached first, then fresh.
    Account(Account),
    /// No session: signed out, offline or the source cannot start. The source logs why.
    Failed,
}

pub struct Account {
    pub name: String,
}

/// A folder a source reads from, set in Settings.
pub struct Folder {
    pub id: Ref,
    pub label: Text,
    pub hint: Text,
    /// The saved choice, else the source's default.
    pub path: Option<PathBuf>,
    /// What `path` holds, or why it cannot be used.
    pub status: Result<Text, Failure>,
    /// Ready picks: a label and a path.
    pub choices: Vec<(String, PathBuf)>,
    /// The pick `path` lies in.
    pub selected: Option<PathBuf>,
}
