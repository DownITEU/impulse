//! Plugins: WASM modules the user installs, each a source (`plugin-<id>`) behind `model`'s traits. A guest speaks
//! `sdk`'s wire; everything it answers passes the import step before the app sees it.

mod cache;
mod import;
mod manifest;
mod runtime;
mod scope;
mod settings;
#[cfg(test)]
mod tests;

use std::{
    fs,
    io::{self, Read},
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use anyhow::{Context, anyhow};
use blocking::unblock;
use futures::{
    StreamExt, future,
    stream::{self, BoxStream},
};
use model::{Failure, FailureKind, Locale, Map, MapOffer, Maps, Snapshot, Source, SourceId};
use sdk::{
    manifest::Manifest,
    wire::{self, ErrorKind},
};

use cache::{Cache, Stamp};
use runtime::Net;
use scope::Scope;
pub use settings::Values;

// Far past a Rust guest's size (the MapGenie one is 170 KB).
const MODULE: u64 = 16 << 20;

pub struct Plugin(Arc<Inner>);

struct Inner {
    manifest: Manifest,
    scope: Arc<Scope>,
    runtime: runtime::Plugin,
    cache: Cache,
    /// The user's, each checked against its field.
    settings: Values,
    net: Net,
}

impl Plugin {
    /// The plugin in `dir`, its answers kept in `cache`; `values`: the user's settings for it (blocking).
    pub fn open(dir: &Path, cache: PathBuf, values: &Values) -> Result<Self, Failure> {
        Self::with(dir, cache, values, net)
    }

    fn with(dir: &Path, cache: PathBuf, values: &Values, net: fn(Arc<Scope>) -> Net) -> Result<Self, Failure> {
        let (manifest, scope) = manifest::read(dir)?;
        let wasm = read(&dir.join(&manifest.module), MODULE)?;
        let runtime = runtime::Plugin::new(scope.source.clone(), &wasm)?;
        let settings = settings::resolve(&manifest.settings, values);
        let cache = Cache::new(cache, Duration::from_secs(u64::from(manifest.refresh) * 24 * 60 * 60));
        let scope = Arc::new(scope);
        Ok(Self(Arc::new(Inner { net: net(scope.clone()), manifest, scope, runtime, cache, settings })))
    }
}

impl Source for Plugin {
    fn id(&self) -> SourceId {
        self.0.scope.source.clone()
    }
}

impl Maps for Plugin {
    fn map(&self, offer: &MapOffer, locale: &Locale) -> BoxStream<'static, Snapshot<Map>> {
        let (inner, id) = (self.0.clone(), offer.id.id().to_owned());
        let stamp = inner.stamp(locale, offer.revision.clone());
        let cached = unblock({
            let (inner, id, stamp) = (inner.clone(), id.clone(), stamp.clone());
            move || inner.cached(&id, &stamp)
        });
        stream::once(cached)
            .flat_map(move |(copy, fresh)| {
                let inner = inner.clone();
                let (id, stamp) = (id.clone(), stamp.clone());
                let ask = (!fresh).then(|| unblock(move || Snapshot::Fresh(inner.ask(&id, stamp))));
                stream::once(future::ready(Snapshot::Cached(copy))).chain(stream::iter(ask).then(|asked| asked))
            })
            .boxed()
    }
}

impl Inner {
    fn stamp(&self, locale: &Locale, revision: Option<String>) -> Stamp {
        let locale = manifest::closest(&self.manifest.locales, locale).to_owned();
        Stamp { api: sdk::API, version: self.manifest.version.clone(), locale, settings: self.settings.clone(), revision }
    }

    /// The copy on disk and whether it is fresh (blocking).
    fn cached(&self, id: &str, stamp: &Stamp) -> (Option<Map>, bool) {
        let Some((answer, fresh)) = self.cache.read::<wire::Map>(&key(id), stamp) else { return (None, false) };
        match import::map(&self.scope, answer) {
            Ok(map) => (Some(map), fresh),
            Err(e) => {
                log::warn!("{}: map {id} on disk: {e:#}", self.scope.source.as_str());
                (None, false)
            }
        }
    }

    /// The guest's answer, imported and kept (blocking).
    fn ask(&self, id: &str, stamp: Stamp) -> Result<Map, Failure> {
        let request =
            wire::Request { call: wire::Call::Map { map: id.to_owned() }, locale: stamp.locale.clone(), settings: stamp.settings.clone() };
        let asked = self.runtime.call::<wire::Map>(&request, self.net.clone()).and_then(|answer| {
            let kept = Cache::encode(stamp, &answer)?;
            let map = import::map(&self.scope, answer)?;
            self.cache.put(&key(id), &kept);
            Ok(map)
        });
        asked.map_err(|e| {
            log::warn!("{}: map {id}: {e:#}", self.scope.source.as_str());
            Failure::from(e)
        })
    }
}

/// A file of at most `most` bytes (blocking): a plugin's folder and its cache may hold anything. Missing: `NotFound`;
/// larger: `Refused`; unreadable: `Internal`.
fn read(path: &Path, most: u64) -> anyhow::Result<Vec<u8>> {
    let file = fs::File::open(path).map_err(|e| {
        let missing = e.kind() == io::ErrorKind::NotFound;
        let e = anyhow::Error::new(e).context(format!("open {}", path.display()));
        if missing { e.context(Failure::new(FailureKind::NotFound)) } else { e }
    })?;
    let mut bytes = Vec::new();
    file.take(most + 1).read_to_end(&mut bytes).with_context(|| format!("read {}", path.display()))?;
    if bytes.len() as u64 > most {
        return Err(anyhow!("{} is past {most} bytes", path.display())).context(Failure::new(FailureKind::Refused));
    }
    Ok(bytes)
}

fn key(map: &str) -> String {
    format!("map:{map}")
}

/// The app's GET for a guest: on the plugin's hosts only, its outcome as the guest's error kinds.
fn net(scope: Arc<Scope>) -> Net {
    Arc::new(move |url, left| {
        let error = |kind, message: String| Err(wire::Error::new(kind, message));
        match fetch::guarded(url, &|host| scope.hosts.allows(host), runtime::ANSWER as u64, left) {
            Ok(response) if (200..300).contains(&response.status) => Ok(response.body),
            Ok(response) => {
                let kind = match response.status {
                    404 | 410 => ErrorKind::NotFound,
                    401 | 403 | 429 => ErrorKind::Refused,
                    500.. => ErrorKind::Offline,
                    _ => ErrorKind::Internal,
                };
                error(kind, format!("HTTP {}", response.status))
            }
            Err(e) if e.downcast_ref::<fetch::Refused>().is_some() => error(ErrorKind::Refused, format!("{e:#}")),
            Err(e) => error(ErrorKind::Offline, format!("{e:#}")),
        }
    })
}
