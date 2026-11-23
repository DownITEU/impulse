//! The media cache: pictures, map tiles and pin videos from public CDNs (`http(s)://`) and from a source's own URLs
//! (any other scheme, through the registry). A file per URL, named from it, in a folder per kind, so it loads at once
//! and offline; an empty file marks what the server does not have.

pub mod limit;

use std::{
    fs,
    path::{Path, PathBuf},
    time::{Duration, SystemTime},
};

use anyhow::{Result, anyhow, bail};
use model::FailureKind;

use crate::{Registry, storage::Paths};

const ABSENT: [u16; 1] = [404];
// A provider serving a window of a larger pyramid answers 403 or 410 outside it.
const ABSENT_TILE: [u16; 3] = [403, 404, 410];

/// A kind of cached file, in a folder of its own: sized, cleared and limited apart.
#[derive(Clone, Copy, PartialEq)]
pub enum Media {
    Pictures,
    Tiles,
    Videos,
}

impl Media {
    pub const ALL: [Self; 3] = [Self::Pictures, Self::Tiles, Self::Videos];

    pub fn dir(self, paths: &Paths) -> PathBuf {
        match self {
            Self::Pictures => paths.pictures(),
            Self::Tiles => paths.tiles(),
            Self::Videos => paths.videos(),
        }
    }
}

pub fn file(cache: &Path, url: &str) -> PathBuf {
    cache.join(name(url))
}

/// Whether the CDN already answered 404 for `url` (blocking), so nothing asks for it again.
pub fn known_missing(cache: &Path, url: &str) -> bool {
    fs::metadata(file(cache, url)).is_ok_and(|meta| meta.len() == 0)
}

/// Forgets the absences `cache` remembers for the tiles of `templates` (URLs with `{z}`, `{x}`, `{y}`), so they are
/// asked again (blocking). Matched by the file name's part before the first `{`: a wider match only asks again.
pub fn forget_missing(cache: &Path, templates: &[&str]) {
    let Ok(entries) = fs::read_dir(cache) else { return };
    let heads: Vec<String> = templates.iter().filter_map(|template| template.split_once('{')).map(|(head, _)| name(head)).collect();
    for entry in entries.flatten() {
        let named = entry.file_name().to_str().is_some_and(|file| heads.iter().any(|head| file.starts_with(head.as_str())));
        if named && entry.metadata().is_ok_and(|meta| meta.is_file() && meta.len() == 0) {
            let _ = fs::remove_file(entry.path());
        }
    }
}

fn name(url: &str) -> String {
    let keep = |c: char| c.is_ascii_alphanumeric() || c == '.';
    url.trim_start_matches("https://").chars().map(|c| if keep(c) { c } else { '_' }).collect()
}

pub fn steam_hero(app: u32) -> String {
    format!("https://cdn.cloudflare.steamstatic.com/steam/apps/{app}/library_hero.jpg")
}

pub enum Fetched<T = Vec<u8>> {
    Ready(T),
    /// Not on the server: remembered, never asked again.
    Absent,
    /// Failed this time.
    Retry,
}

impl<T> Fetched<T> {
    pub fn and_then<U>(self, next: impl FnOnce(T) -> Fetched<U>) -> Fetched<U> {
        match self {
            Self::Ready(value) => next(value),
            Self::Absent => Fetched::Absent,
            Self::Retry => Fetched::Retry,
        }
    }
}

/// `url`'s bytes (blocking).
pub fn load(cache: &Path, url: &str, registry: &Registry) -> Result<Vec<u8>> {
    match get(cache, url, &ABSENT, registry)? {
        Some(bytes) => Ok(bytes),
        None => bail!("{url} does not exist"),
    }
}

/// A tile's bytes (blocking).
pub fn tile(cache: &Path, url: &str, registry: &Registry) -> Fetched {
    match get(cache, url, &ABSENT_TILE, registry) {
        Ok(Some(bytes)) => Fetched::Ready(bytes),
        Ok(None) => Fetched::Absent,
        Err(e) => {
            log::debug!("tile: {e:#}");
            Fetched::Retry
        }
    }
}

/// `url`'s kept bytes, never asked for (blocking); `None` when not kept or absent.
pub fn cached(cache: &Path, url: &str) -> Option<Vec<u8>> {
    fs::read(file(cache, url)).ok().filter(|bytes| !bytes.is_empty())
}

/// `url`'s file on disk, downloaded first when it is not there (blocking).
pub fn path(cache: &Path, url: &str, registry: &Registry) -> Result<PathBuf> {
    let file = file(cache, url);
    let kept = match fs::metadata(&file) {
        Ok(meta) => meta.len() > 0,
        Err(_) => download(&file, url, &ABSENT, registry)?.is_some(),
    };
    if !kept {
        bail!("{url} does not exist");
    }
    touch(&file);
    Ok(file)
}

// Disk first and never revalidated: files under one URL do not change. `None`: the server lacks it.
fn get(cache: &Path, url: &str, absent: &[u16], registry: &Registry) -> Result<Option<Vec<u8>>> {
    let file = file(cache, url);
    match fs::read(&file) {
        Ok(bytes) if bytes.is_empty() => Ok(None),
        Ok(bytes) => {
            touch(&file);
            Ok(Some(bytes))
        }
        Err(_) => download(&file, url, absent, registry),
    }
}

// The media limit drops the least recently used files first; a day's precision spares a write per hit.
fn touch(file: &Path) {
    let age = fs::metadata(file).and_then(|meta| meta.modified()).map(|at| at.elapsed().unwrap_or_default());
    if age.is_ok_and(|age| age < Duration::from_secs(24 * 60 * 60)) {
        return;
    }
    let _ = fs::File::options().write(true).open(file).and_then(|file| file.set_modified(SystemTime::now()));
}

fn download(file: &Path, url: &str, absent: &[u16], registry: &Registry) -> Result<Option<Vec<u8>>> {
    let (status, body) = request(url, registry)?;
    let gone = absent.contains(&status);
    if !gone && status != 200 {
        bail!("GET {url}: HTTP {status}");
    }
    if let Err(e) = disk::replace(file, if gone { &[] } else { &body }) {
        log::warn!("media not cached: {e:#}");
    }
    Ok((!gone).then_some(body))
}

/// A status and body: the CDN's, or a source's answer told as the status it stands for (`NotFound` 404, `Refused`
/// 403); any other failure is an error, never a status.
fn request(url: &str, registry: &Registry) -> Result<(u16, Vec<u8>)> {
    let web = ["http://", "https://"].iter().any(|scheme| url.get(..scheme.len()).is_some_and(|head| head.eq_ignore_ascii_case(scheme)));
    if web {
        let response = fetch::get(url, None)?;
        return Ok((response.status, response.body));
    }
    match registry.download(url) {
        Ok(body) => Ok((200, body)),
        Err(failure) => match failure.kind {
            FailureKind::NotFound => Ok((404, Vec::new())),
            FailureKind::Refused => Ok((403, Vec::new())),
            _ => Err(anyhow!("GET {url}: {failure}")),
        },
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use model::{Downloads, Failure, Source, SourceId};

    use super::*;
    use crate::Capabilities;

    /// Answers `<path>` with its bytes, `gone` with `NotFound`, `refused` with `Refused`; keeps what it was asked.
    struct Server(Mutex<Vec<String>>);

    impl Source for Server {
        fn id(&self) -> SourceId {
            SourceId::new("server").unwrap()
        }
    }

    impl Downloads for Server {
        fn download(&self, url: &str) -> Result<Vec<u8>, Failure> {
            self.0.lock().unwrap().push(url.to_owned());
            match url.rsplit('/').next() {
                Some("gone") => Err(Failure::new(FailureKind::NotFound)),
                Some("refused") => Err(Failure::new(FailureKind::Refused)),
                _ => Ok(url.as_bytes().to_vec()),
            }
        }
    }

    /// A fresh cache folder and a registry with the server; `asked` lists its requests.
    fn served(test: &str) -> (PathBuf, Registry, Arc<Server>) {
        let cache = std::env::temp_dir().join(format!("impulse-media-{test}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&cache);
        fs::create_dir_all(&cache).unwrap();
        let server = Arc::new(Server(Mutex::default()));
        let registry = Registry::default();
        registry.add(Capabilities { downloads: Some(server.clone()), ..Capabilities::default() });
        (cache, registry, server)
    }

    fn asked(server: &Server) -> usize {
        server.0.lock().unwrap().len()
    }

    #[test]
    fn a_kept_file_is_served_without_a_request_and_its_old_time_is_refreshed() {
        let (cache, registry, server) = served("kept");
        let url = "server://cdn/a.png";
        assert!(cached(&cache, url).is_none() && asked(&server) == 0);
        assert_eq!(load(&cache, url, &registry).unwrap(), url.as_bytes());
        assert_eq!(cached(&cache, url).as_deref(), Some(url.as_bytes()));
        let kept = file(&cache, url);
        let old = SystemTime::now() - Duration::from_secs(3 * 24 * 60 * 60);
        fs::File::options().write(true).open(&kept).unwrap().set_modified(old).unwrap();
        assert_eq!(load(&cache, url, &registry).unwrap(), url.as_bytes());
        assert_eq!(asked(&server), 1);
        assert!(fs::metadata(&kept).unwrap().modified().unwrap() > old);
        let _ = fs::remove_dir_all(cache);
    }

    #[test]
    fn what_the_source_lacks_is_remembered_and_never_asked_again() {
        let (cache, registry, server) = served("gone");
        let url = "server://cdn/gone";
        assert!(load(&cache, url, &registry).is_err() && known_missing(&cache, url) && cached(&cache, url).is_none());
        assert!(matches!(tile(&cache, url, &registry), Fetched::Absent));
        assert!(path(&cache, url, &registry).is_err());
        assert_eq!(asked(&server), 1);
        let _ = fs::remove_dir_all(cache);
    }

    #[test]
    fn a_refused_tile_is_absent_but_a_refused_picture_is_asked_again() {
        let (cache, registry, server) = served("refused");
        let url = "server://cdn/refused";
        for _ in 0..2 {
            assert!(load(&cache, url, &registry).is_err() && !known_missing(&cache, url));
        }
        assert_eq!(asked(&server), 2);
        assert!(matches!(tile(&cache, url, &registry), Fetched::Absent) && known_missing(&cache, url));
        assert_eq!(asked(&server), 3);
        let _ = fs::remove_dir_all(cache);
    }

    #[test]
    fn only_the_remembered_absences_of_the_templates_are_forgotten() {
        let cache = std::env::temp_dir().join(format!("impulse-media-{}", std::process::id()));
        fs::create_dir_all(&cache).unwrap();
        let ours = "plugin-maps://tiles.example/night/{z}/{x}/{y}.webp";
        let absent = file(&cache, &ours.replace("{z}/{x}/{y}", "1/0/0"));
        let kept = file(&cache, &ours.replace("{z}/{x}/{y}", "1/0/1"));
        let other = file(&cache, "https://cdn.example/day/1/0/0.webp");
        fs::write(&absent, b"").unwrap();
        fs::write(&kept, b"tile").unwrap();
        fs::write(&other, b"").unwrap();
        forget_missing(&cache, &[ours]);
        assert!(!absent.exists() && kept.exists() && other.exists());
        let _ = fs::remove_dir_all(cache);
    }
}
