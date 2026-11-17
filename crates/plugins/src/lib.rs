//! Plugins: WASM modules the user installs, each a source (`plugin-<id>`) behind `model`'s traits. A guest speaks
//! `sdk`'s wire; everything it answers passes the import step before the app sees it.

mod cache;
mod import;
mod manifest;
mod net;
mod runtime;
mod scope;
mod serve;
mod settings;
#[cfg(test)]
mod tests;
mod texts;

use std::{
    collections::HashSet,
    fs,
    io::{self, Read},
    path::Path,
    sync::{Arc, Mutex},
    time::Duration,
};

use anyhow::{Context, anyhow};
use futures::stream::BoxStream;
use model::{Downloads, Failure, FailureKind, Files, Locale, Map, MapOffer, Maps, Ref, Snapshot, Source, SourceId, Title, Titles};
use sdk::{manifest::Manifest, wire};

use cache::Cache;
use net::net;
use runtime::Net;
use scope::Scope;
pub use settings::Values;

// Far past a Rust guest's size (the MapGenie one is 170 KB).
const MODULE: u64 = 16 << 20;
// What a download may bring back, and how long it may take.
const DOWNLOAD: u64 = 64 << 20;
const DOWNLOAD_TIME: Duration = Duration::from_secs(60);

pub struct Plugin(Arc<Inner>);

struct Inner {
    manifest: Manifest,
    scope: Arc<Scope>,
    runtime: runtime::Plugin,
    cache: Cache,
    /// The user's, each checked against its field.
    settings: Values,
    net: Net,
    texts: Files,
    /// Keys `Maps::refresh` marked: asked again whatever their age, until an answer arrives.
    stale: Mutex<HashSet<String>>,
}

impl Plugin {
    /// The plugin in `dir`, its answers kept in `caches/<its id>`; `values`: the user's settings for it (blocking).
    pub fn open(dir: &Path, caches: &Path, values: &Values) -> Result<Self, Failure> {
        Self::with(dir, caches, values, net)
    }

    fn with(dir: &Path, caches: &Path, values: &Values, net: impl FnOnce(Arc<Scope>) -> Net) -> Result<Self, Failure> {
        let (manifest, scope) = manifest::read(dir)?;
        let wasm = read(&dir.join(&manifest.module), MODULE)?;
        let runtime = runtime::Plugin::new(scope.source.clone(), &wasm)?;
        let settings = settings::resolve(&manifest.settings, values);
        let cache = Cache::new(caches.join(&manifest.id), Duration::from_secs(u64::from(manifest.refresh) * 24 * 60 * 60));
        let scope = Arc::new(scope);
        let texts = texts::load(dir);
        let stale = Mutex::default();
        Ok(Self(Arc::new(Inner { net: net(scope.clone()), manifest, scope, runtime, cache, settings, texts, stale })))
    }
}

impl Source for Plugin {
    fn id(&self) -> SourceId {
        self.0.scope.source.clone()
    }

    fn texts(&self) -> Files {
        self.0.texts.clone()
    }
}

impl Maps for Plugin {
    fn map(&self, offer: &MapOffer, locale: &Locale) -> BoxStream<'static, Snapshot<Map>> {
        let id = offer.id.id();
        let call = wire::Call::Map { map: id.to_owned() };
        self.0.serve(serve::map_key(id), call, offer.revision.clone(), locale, import::map)
    }

    fn refresh(&self, map: &Ref) {
        if map.source() == &self.0.scope.source {
            self.0.stale_keys().insert(serve::map_key(map.id()));
        }
    }
}

impl Titles for Plugin {
    fn titles(&self, locale: &Locale) -> BoxStream<'static, Snapshot<Vec<Title>>> {
        self.0.serve(serve::TITLES_KEY.to_owned(), wire::Call::Titles, None, locale, import::titles)
    }

    fn offers(&self, title: &Ref, locale: &Locale) -> BoxStream<'static, Snapshot<Vec<MapOffer>>> {
        let id = title.id();
        self.0.serve(serve::offers_key(id), wire::Call::Maps { title: id.to_owned() }, None, locale, import::offers)
    }
}

impl Downloads for Plugin {
    fn download(&self, url: &str) -> Result<Vec<u8>, Failure> {
        let source = &self.0.scope.source;
        let ours = Ref::parse(url).filter(|url| url.source() == source && url.id().starts_with("//"));
        let Some(ours) = ours else {
            return Err(Failure { detail: format!("{}: not a URL of this plugin", source.as_str()), ..Failure::new(FailureKind::Refused) });
        };
        let url = format!("https:{}", ours.id());
        (self.0.net)(&url, DOWNLOAD, DOWNLOAD_TIME).map_err(|e| Failure::from(runtime::guest(&self.0.scope.source, e)))
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
