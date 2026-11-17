//! Where games, trainers, maps and the account come from: a source, and what it can do, one trait each.

use std::{path::PathBuf, sync::Arc};

use futures::{future::BoxFuture, stream::BoxStream};

use crate::{Failure, Game, Installation, Locale, Map, MapOffer, Offer, Ref, SourceId, StoreId, Text, Trainer};

pub trait Source: Send + Sync {
    /// The source of every `Ref` it mints, and the domain of its texts.
    fn id(&self) -> SourceId;

    fn texts(&self) -> Files {
        Files::default()
    }
}

/// Catalogs: a language tag and its Fluent source each. Owned: a plugin's are read when it opens.
pub type Files = Arc<[(Arc<str>, Arc<str>)]>;

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

    /// Marks the copy of `map` stale, so the next `map` asks again. Nothing for a source whose copies follow their
    /// revision alone.
    fn refresh(&self, _map: &Ref) {}
}

/// Discovery: the games a source has maps for, matched to the library by store id.
pub trait Titles: Source {
    fn titles(&self, locale: &Locale) -> BoxStream<'static, Snapshot<Vec<Title>>>;
    fn offers(&self, title: &Ref, locale: &Locale) -> BoxStream<'static, Snapshot<Vec<MapOffer>>>;
}

/// A game as a discovering source names it.
#[derive(Clone, Debug, PartialEq)]
pub struct Title {
    pub id: Ref,
    pub name: String,
    pub stores: Vec<StoreId>,
}

/// The bytes behind URLs a source put in its data, under that source's network scope.
pub trait Downloads: Source {
    /// Blocking. `NotFound`: the server lacks it; `Refused`: never to be fetched; any other kind may pass.
    fn download(&self, url: &str) -> Result<Vec<u8>, Failure>;
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
