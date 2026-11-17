//! The snapshot stream every guest answer shares (maps, titles, offers): the kept copy, then the guest's answer only
//! when the copy is missing or stale.

use std::sync::Arc;

use anyhow::Result;
use blocking::unblock;
use futures::{
    FutureExt, StreamExt, future,
    stream::{self, BoxStream},
};
use model::{Failure, Locale, Snapshot};
use sdk::wire;
use serde::{Serialize, de::DeserializeOwned};

use crate::{
    Inner,
    cache::{Cache, Stamp},
    manifest,
    scope::Scope,
};

/// How a guest's answer becomes the app's value.
pub type Import<W, T> = fn(&Scope, W) -> Result<T>;

pub fn map_key(map: &str) -> String {
    format!("map:{map}")
}

pub fn offers_key(title: &str) -> String {
    format!("maps:{title}")
}

pub const TITLES_KEY: &str = "titles";

impl Inner {
    /// `Cached`, then `Fresh` when the copy is missing or stale. The guest is not called before the stream is polled
    /// past `Cached`: a reader content with the copy drops it for nothing.
    pub fn serve<W, T>(
        self: &Arc<Self>,
        key: String,
        call: wire::Call,
        revision: Option<String>,
        locale: &Locale,
        import: Import<W, T>,
    ) -> BoxStream<'static, Snapshot<T>>
    where
        W: Serialize + DeserializeOwned + 'static,
        T: Send + 'static,
    {
        let inner = self.clone();
        let stamp = self.stamp(locale, revision);
        let cached = unblock({
            let (inner, key, stamp) = (inner.clone(), key.clone(), stamp.clone());
            move || inner.cached(&key, &stamp, import)
        });
        let then = cached.map(move |(copy, fresh)| {
            let ask =
                (!fresh).then(|| stream::once(async move { Snapshot::Fresh(unblock(move || inner.ask(&key, call, stamp, import)).await) }));
            stream::once(future::ready(Snapshot::Cached(copy))).chain(stream::iter(ask).flatten())
        });
        stream::once(then).flatten().boxed()
    }

    fn stamp(&self, locale: &Locale, revision: Option<String>) -> Stamp {
        let locale = manifest::closest(&self.manifest.locales, locale).to_owned();
        Stamp { api: sdk::API, version: self.manifest.version.clone(), locale, settings: self.settings.clone(), revision }
    }

    /// The copy on disk and whether it is fresh: a key `refresh` marked is not (blocking).
    fn cached<W: DeserializeOwned, T>(&self, key: &str, stamp: &Stamp, import: Import<W, T>) -> (Option<T>, bool) {
        let Some((answer, fresh)) = self.cache.read::<W>(key, stamp) else { return (None, false) };
        match import(&self.scope, answer) {
            Ok(value) => (Some(value), fresh && !self.stale_keys().contains(key)),
            Err(e) => {
                log::warn!("{}: {key} on disk: {e:#}", self.scope.source.as_str());
                (None, false)
            }
        }
    }

    /// The guest's answer, imported and kept (blocking).
    fn ask<W: Serialize + DeserializeOwned, T>(
        &self,
        key: &str,
        call: wire::Call,
        stamp: Stamp,
        import: Import<W, T>,
    ) -> Result<T, Failure> {
        let request = wire::Request { call, locale: stamp.locale.clone(), settings: stamp.settings.clone() };
        let asked = self.runtime.call::<W>(&request, self.net.clone()).and_then(|answer| {
            let kept = Cache::encode(stamp, &answer)?;
            let value = import(&self.scope, answer)?;
            self.cache.put(key, &kept);
            Ok(value)
        });
        match asked {
            Ok(value) => {
                self.stale_keys().remove(key);
                self.set_failure(None);
                Ok(value)
            }
            Err(e) => {
                log::warn!("{}: {key}: {e:#}", self.scope.source.as_str());
                let failure = Failure::from(e);
                self.set_failure(Some(failure.clone()));
                Err(failure)
            }
        }
    }

    fn set_failure(&self, failure: Option<Failure>) {
        *self.failure.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = failure;
    }

    /// The keys `refresh` marked; a poisoned lock holds a plain set that is still good.
    pub fn stale_keys(&self) -> std::sync::MutexGuard<'_, std::collections::HashSet<String>> {
        self.stale.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}
