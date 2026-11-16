//! The sources' capabilities, built in `app` one by one: a `Ref` goes to the source it names.

use std::{path::PathBuf, sync::Arc};

use futures::{
    FutureExt, StreamExt,
    future::{self, BoxFuture},
    stream::{self, BoxStream},
};
use model::{
    Accounts, Caches, Failure, FailureKind, Files, Folder, Folders, Game, Games, Installation, Locale, Map, MapOffer, Maps, Offer, Ref,
    SignIn, Snapshot, SourceId, Store, Text, Trainer, Trainers,
};

/// Each capability's sources, with their ids.
type List<T> = Vec<(SourceId, Arc<T>)>;

#[derive(Default)]
pub struct Registry {
    accounts: List<dyn Accounts>,
    games: List<dyn Games>,
    trainers: List<dyn Trainers>,
    maps: List<dyn Maps>,
    folders: List<dyn Folders>,
    caches: List<dyn Caches>,
    /// Each source's catalogs, once.
    texts: Vec<(SourceId, Files)>,
}

macro_rules! add {
    ($($add:ident: $list:ident $trait:ident),*) => {
        impl Registry {
            $(pub fn $add(mut self, source: Arc<dyn $trait>) -> Self {
                let id = source.id();
                if !self.texts.iter().any(|(known, _)| *known == id) {
                    self.texts.push((id.clone(), source.texts()));
                }
                self.$list.push((id, source));
                self
            })*
        }
    };
}

add!(
    with_accounts: accounts Accounts,
    with_games: games Games,
    with_trainers: trainers Trainers,
    with_maps: maps Maps,
    with_folders: folders Folders,
    with_caches: caches Caches
);

impl Registry {
    pub fn texts(&self) -> &[(SourceId, Files)] {
        &self.texts
    }

    /// The first source's account.
    pub fn sign_in(&self) -> BoxStream<'static, SignIn> {
        self.accounts.first().map_or_else(|| stream::iter([SignIn::Failed]).boxed(), |(_, source)| source.sign_in())
    }

    /// The first source's catalog.
    pub fn games(&self) -> BoxStream<'static, Snapshot<Vec<Game>>> {
        self.games.first().map_or_else(|| missing("games"), |(_, source)| source.games())
    }

    pub fn trainer(&self, offer: &Offer, install: &Installation, locale: &Locale) -> BoxStream<'static, Snapshot<Trainer>> {
        route(&self.trainers, &offer.id).map_or_else(|| missing(&offer.id), |source| source.trainer(offer, install, locale))
    }

    pub fn map(&self, offer: &MapOffer, locale: &Locale) -> BoxStream<'static, Snapshot<Map>> {
        route(&self.maps, &offer.id).map_or_else(|| missing(&offer.id), |source| source.map(offer, locale))
    }

    /// Every source's.
    pub fn folders(&self) -> BoxFuture<'static, Vec<Folder>> {
        let all = future::join_all(self.folders.iter().map(|(_, source)| source.folders()));
        all.map(|lists| lists.into_iter().flatten().collect()).boxed()
    }

    pub fn set_folder(&self, id: &Ref, path: PathBuf) -> BoxFuture<'static, Folder> {
        match route(&self.folders, id) {
            Some(source) => source.set_folder(id, path),
            None => {
                let (label, hint) = (Text::Plain(String::new()), Text::Plain(String::new()));
                let status = Err(unserved(id));
                let folder = Folder { id: id.clone(), label, hint, path: Some(path), status, choices: Vec::new(), selected: None };
                future::ready(folder).boxed()
            }
        }
    }

    /// Every source's.
    pub fn stores(&self) -> BoxFuture<'static, Vec<Store>> {
        let all = future::join_all(self.caches.iter().map(|(_, source)| source.stores()));
        all.map(|lists| lists.into_iter().flatten().collect()).boxed()
    }

    pub fn clear(&self, id: &Ref) -> BoxFuture<'static, ()> {
        route(&self.caches, id).map_or_else(|| future::ready(()).boxed(), |source| source.clear(id))
    }
}

fn route<'a, T: ?Sized>(list: &'a List<T>, id: &Ref) -> Option<&'a Arc<T>> {
    list.iter().find(|(source, _)| source == id.source()).map(|(_, source)| source)
}

/// A fresh error for what no source serves.
fn missing<T: Send + 'static>(what: impl std::fmt::Display) -> BoxStream<'static, Snapshot<T>> {
    stream::iter([Snapshot::Fresh(Err(unserved(what)))]).boxed()
}

fn unserved(what: impl std::fmt::Display) -> Failure {
    Failure { detail: format!("no source for {what}"), ..Failure::new(FailureKind::Unsupported) }
}

#[cfg(test)]
mod tests;
