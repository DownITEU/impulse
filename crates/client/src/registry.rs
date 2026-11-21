//! The sources' capabilities, added in `app` and while the app runs (plugins): a `Ref` goes to the source it names.

use std::{
    path::PathBuf,
    sync::{Arc, Mutex, PoisonError, RwLock},
};

use futures::{
    FutureExt, StreamExt,
    channel::{
        mpsc::{self, UnboundedReceiver, UnboundedSender},
        oneshot,
    },
    future::{self, BoxFuture, Shared},
    stream::{self, BoxStream},
};
use model::{
    Accounts, Caches, Downloads, Failure, FailureKind, Files, Folder, Folders, Game, Games, Installation, Locale, Map, MapOffer, Maps,
    Offer, Ref, SignIn, Snapshot, Source, SourceId, Store, Text, Title, Titles, Trainer, Trainers,
};

/// One source's capabilities, added and removed together; each is that source's own.
#[derive(Default)]
pub struct Capabilities {
    pub accounts: Option<Arc<dyn Accounts>>,
    pub games: Option<Arc<dyn Games>>,
    pub trainers: Option<Arc<dyn Trainers>>,
    pub maps: Option<Arc<dyn Maps>>,
    pub titles: Option<Arc<dyn Titles>>,
    pub downloads: Option<Arc<dyn Downloads>>,
    pub folders: Option<Arc<dyn Folders>>,
    pub caches: Option<Arc<dyn Caches>>,
}

impl Capabilities {
    fn sources(&self) -> impl Iterator<Item = &dyn Source> {
        let Self { accounts, games, trainers, maps, titles, downloads, folders, caches } = self;
        let all: [Option<&dyn Source>; 8] = [
            accounts.as_deref().map(|source| source as _),
            games.as_deref().map(|source| source as _),
            trainers.as_deref().map(|source| source as _),
            maps.as_deref().map(|source| source as _),
            titles.as_deref().map(|source| source as _),
            downloads.as_deref().map(|source| source as _),
            folders.as_deref().map(|source| source as _),
            caches.as_deref().map(|source| source as _),
        ];
        all.into_iter().flatten()
    }
}

/// A source came or went.
#[derive(Clone, Debug, PartialEq)]
pub enum Change {
    Added(SourceId),
    Removed(SourceId),
}

/// A registration: its streams end once it is removed, so nothing late of it passes, even after it is added again.
struct Entry {
    id: SourceId,
    name: Text,
    texts: Files,
    capabilities: Capabilities,
    // Dropped with the entry, which ends `ended`.
    _end: oneshot::Sender<()>,
    ended: Shared<oneshot::Receiver<()>>,
}

#[derive(Default)]
pub struct Registry {
    entries: RwLock<Vec<Entry>>,
    watchers: Mutex<Vec<UnboundedSender<Change>>>,
}

impl Registry {
    /// A source already there is replaced.
    pub fn add(&self, capabilities: Capabilities) {
        let (id, name, texts) = {
            let mut sources = capabilities.sources();
            let first = sources.next().expect("a source with a capability");
            let id = first.id();
            assert!(sources.all(|source| source.id() == id), "{}: a capability of another source", id.as_str());
            (id, first.name(), first.texts())
        };
        let (end, ended) = oneshot::channel();
        let entry = Entry { id: id.clone(), name, texts, capabilities, _end: end, ended: ended.shared() };
        // Under one lock, so two adds of a source never both stay; the one replaced ends once the lock is let go.
        let replaced = {
            let mut entries = self.entries.write().unwrap_or_else(PoisonError::into_inner);
            let replaced = entries.iter().position(|known| known.id == id).map(|at| entries.remove(at));
            entries.push(entry);
            replaced
        };
        if replaced.is_some() {
            self.tell(Change::Removed(id.clone()));
        }
        self.tell(Change::Added(id));
    }

    /// Its streams end; the futures already given finish.
    pub fn remove(&self, id: &SourceId) {
        let removed = {
            let mut entries = self.entries.write().unwrap_or_else(PoisonError::into_inner);
            entries.iter().position(|entry| entry.id == *id).map(|at| entries.remove(at))
        };
        if removed.is_some() {
            self.tell(Change::Removed(id.clone()));
        }
    }

    /// Every change from now on.
    pub fn changes(&self) -> UnboundedReceiver<Change> {
        let (tx, rx) = mpsc::unbounded();
        self.watchers.lock().unwrap_or_else(PoisonError::into_inner).push(tx);
        rx
    }

    fn tell(&self, change: Change) {
        self.watchers.lock().unwrap_or_else(PoisonError::into_inner).retain(|tx| tx.unbounded_send(change.clone()).is_ok());
    }

    pub fn has(&self, id: &SourceId) -> bool {
        self.entries().iter().any(|entry| entry.id == *id)
    }

    /// The source has games, trainers or accounts: what a cleared cache of it may empty from the page.
    pub fn plays(&self, id: &SourceId) -> bool {
        self.entries().iter().any(|entry| {
            let c = &entry.capabilities;
            entry.id == *id && (c.games.is_some() || c.trainers.is_some() || c.accounts.is_some())
        })
    }

    /// What the user calls a source that is there.
    pub fn name(&self, id: &SourceId) -> Option<Text> {
        self.entries().iter().find(|entry| entry.id == *id).map(|entry| entry.name.clone())
    }

    /// What the user calls each source that has an account.
    pub fn account_names(&self) -> Vec<Text> {
        self.entries().iter().filter(|entry| entry.capabilities.accounts.is_some()).map(|entry| entry.name.clone()).collect()
    }

    /// Each source's catalogs, as they are now.
    pub fn texts(&self) -> Vec<(SourceId, Files)> {
        self.entries().iter().map(|entry| (entry.id.clone(), entry.texts.clone())).collect()
    }

    /// The first source's account.
    pub fn sign_in(&self) -> BoxStream<'static, SignIn> {
        self.first(|c| c.accounts.as_ref())
            .map_or_else(|| stream::iter([SignIn::Failed(unserved("accounts"))]).boxed(), |(source, ended)| cut(source.sign_in(), ended))
    }

    /// The first source's catalog.
    pub fn games(&self) -> BoxStream<'static, Snapshot<Vec<Game>>> {
        self.first(|c| c.games.as_ref()).map_or_else(|| missing("games"), |(source, ended)| cut(source.games(), ended))
    }

    pub fn trainer(&self, offer: &Offer, install: &Installation, locale: &Locale) -> BoxStream<'static, Snapshot<Trainer>> {
        let routed = self.route(offer.id.source(), |c| c.trainers.as_ref());
        routed.map_or_else(|| missing(&offer.id), |(source, ended)| cut(source.trainer(offer, install, locale), ended))
    }

    pub fn map(&self, offer: &MapOffer, locale: &Locale) -> BoxStream<'static, Snapshot<Map>> {
        let routed = self.route(offer.id.source(), |c| c.maps.as_ref());
        routed.map_or_else(|| missing(&offer.id), |(source, ended)| cut(source.map(offer, locale), ended))
    }

    pub fn refresh(&self, map: &Ref) {
        if let Some((source, _)) = self.route(map.source(), |c| c.maps.as_ref()) {
            source.refresh(map);
        }
    }

    pub fn titled(&self) -> Vec<SourceId> {
        self.entries().iter().filter(|entry| entry.capabilities.titles.is_some()).map(|entry| entry.id.clone()).collect()
    }

    pub fn titles(&self, source: &SourceId, locale: &Locale) -> BoxStream<'static, Snapshot<Vec<Title>>> {
        let routed = self.route(source, |c| c.titles.as_ref());
        routed.map_or_else(|| missing(source.as_str()), |(titles, ended)| cut(titles.titles(locale), ended))
    }

    pub fn offers(&self, title: &Ref, locale: &Locale) -> BoxStream<'static, Snapshot<Vec<MapOffer>>> {
        let routed = self.route(title.source(), |c| c.titles.as_ref());
        routed.map_or_else(|| missing(title), |(titles, ended)| cut(titles.offers(title, locale), ended))
    }

    /// The bytes behind a URL a source gave, `<source>:...` (blocking).
    pub fn download(&self, url: &str) -> Result<Vec<u8>, Failure> {
        let routed = Ref::parse(url).and_then(|id| self.route(id.source(), |c| c.downloads.as_ref()));
        routed.map_or_else(|| Err(unserved(url)), |(source, _)| source.download(url))
    }

    /// Every source's.
    pub fn folders(&self) -> BoxFuture<'static, Vec<Folder>> {
        let all = future::join_all(self.every(|c| c.folders.as_ref()).iter().map(|source| source.folders()));
        all.map(|lists| lists.into_iter().flatten().collect()).boxed()
    }

    pub fn set_folder(&self, id: &Ref, path: PathBuf) -> BoxFuture<'static, Folder> {
        match self.route(id.source(), |c| c.folders.as_ref()) {
            Some((source, _)) => source.set_folder(id, path),
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
        let all = future::join_all(self.every(|c| c.caches.as_ref()).iter().map(|source| source.stores()));
        all.map(|lists| lists.into_iter().flatten().collect()).boxed()
    }

    pub fn clear(&self, id: &Ref) -> BoxFuture<'static, ()> {
        self.route(id.source(), |c| c.caches.as_ref()).map_or_else(|| future::ready(()).boxed(), |(source, _)| source.clear(id))
    }

    fn entries(&self) -> std::sync::RwLockReadGuard<'_, Vec<Entry>> {
        self.entries.read().unwrap_or_else(PoisonError::into_inner)
    }

    // The lock is let go before the source is asked: a source may take its time, or reach the registry itself.
    fn route<T: ?Sized>(&self, id: &SourceId, pick: impl Fn(&Capabilities) -> Option<&Arc<T>>) -> Option<(Arc<T>, Ended)> {
        let entries = self.entries();
        let entry = entries.iter().find(|entry| entry.id == *id)?;
        Some((pick(&entry.capabilities)?.clone(), entry.ended.clone()))
    }

    fn first<T: ?Sized>(&self, pick: impl Fn(&Capabilities) -> Option<&Arc<T>>) -> Option<(Arc<T>, Ended)> {
        let entries = self.entries();
        entries.iter().find_map(|entry| Some((pick(&entry.capabilities)?.clone(), entry.ended.clone())))
    }

    fn every<T: ?Sized>(&self, pick: impl Fn(&Capabilities) -> Option<&Arc<T>>) -> Vec<Arc<T>> {
        self.entries().iter().filter_map(|entry| pick(&entry.capabilities).cloned()).collect()
    }
}

type Ended = Shared<oneshot::Receiver<()>>;

/// Ends with its registration.
fn cut<T: Send + 'static>(stream: BoxStream<'static, T>, ended: Ended) -> BoxStream<'static, T> {
    stream.take_until(ended).boxed()
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
