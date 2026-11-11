//! Pictures, map tiles and pin videos from public CDNs. Each is kept on disk, so it loads at once and offline.

use std::{
    fs,
    path::{Path, PathBuf},
    time::{Duration, SystemTime},
};

use anyhow::{Result, bail};
use futures::future::BoxFuture;
use gpui_kit::http_client::{AsyncBody, HttpClient, Request, Response, Url, http::HeaderValue};

/// Lets `img(url)` load public art through the disk cache and the native CDN client.
pub struct CdnClient {
    pub cache: PathBuf,
}

impl HttpClient for CdnClient {
    fn user_agent(&self) -> Option<&HeaderValue> {
        None
    }

    fn proxy(&self) -> Option<&Url> {
        None
    }

    fn send(
        &self,
        req: Request<AsyncBody>,
    ) -> BoxFuture<'static, anyhow::Result<Response<AsyncBody>>> {
        let url = req.uri().to_string();
        let cache = self.cache.clone();
        Box::pin(async move {
            let bytes = blocking::unblock(move || load(&cache, &url)).await?;
            Ok(Response::new(AsyncBody::from_bytes(bytes.into())))
        })
    }
}

/// A kind of cached file, in a folder of its own: sized, cleared and limited apart.
#[derive(Clone, Copy, PartialEq)]
pub enum Media {
    Pictures,
    Tiles,
    Videos,
}

impl Media {
    pub const ALL: [Self; 3] = [Self::Pictures, Self::Tiles, Self::Videos];

    pub fn dir(self, data_dir: &Path) -> PathBuf {
        data_dir.join(match self {
            Self::Pictures => "art",
            Self::Tiles => "tiles",
            Self::Videos => "videos",
        })
    }
}

/// Whether the CDN already answered 404 for `url` (blocking), so nothing asks for it again.
pub fn known_missing(cache: &Path, url: &str) -> bool {
    fs::metadata(file(cache, url)).is_ok_and(|meta| meta.len() == 0)
}

// Disk first and never revalidated: files under one URL do not change.
pub(super) fn load(cache: &Path, url: &str) -> Result<Vec<u8>> {
    let file = file(cache, url);
    match fs::read(&file) {
        Ok(bytes) if bytes.is_empty() => bail!("{url} does not exist"),
        Ok(bytes) => {
            touch(&file);
            Ok(bytes)
        }
        Err(_) => download(&file, url),
    }
}

/// `url`'s file on disk, downloaded first when it is not there (blocking).
pub fn path(cache: &Path, url: &str) -> Result<PathBuf> {
    let file = file(cache, url);
    match fs::metadata(&file) {
        Ok(meta) if meta.len() == 0 => bail!("{url} does not exist"),
        Ok(_) => touch(&file),
        Err(_) => drop(download(&file, url)?),
    }
    Ok(file)
}

// The media limit drops the least recently used files first; a day's precision spares a write per hit.
fn touch(file: &Path) {
    let age = fs::metadata(file).and_then(|meta| meta.modified()).map(|at| at.elapsed().unwrap_or_default());
    if age.is_ok_and(|age| age < Duration::from_secs(24 * 60 * 60)) {
        return;
    }
    let _ = fs::File::options().write(true).open(file).and_then(|file| file.set_modified(SystemTime::now()));
}

fn download(file: &Path, url: &str) -> Result<Vec<u8>> {
    let response = fetch::get(url, None)?;
    // An empty file marks art the CDN does not have.
    let keep: &[u8] = match response.status {
        200 => &response.body,
        404 => &[],
        status => bail!("GET {url}: HTTP {status}"),
    };
    if let Err(e) = disk::replace(file, keep) {
        log::warn!("art not cached: {e:#}");
    }
    if response.status == 404 {
        bail!("{url} does not exist");
    }
    Ok(response.body)
}

fn file(cache: &Path, url: &str) -> PathBuf {
    let keep = |c: char| c.is_ascii_alphanumeric() || c == '.';
    let name: String = url
        .trim_start_matches("https://")
        .chars()
        .map(|c| if keep(c) { c } else { '_' })
        .collect();
    cache.join(name)
}

pub fn steam_hero(app: u32) -> String {
    format!("https://cdn.cloudflare.steamstatic.com/steam/apps/{app}/library_hero.jpg")
}
