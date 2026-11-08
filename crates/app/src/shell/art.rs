//! Game art and avatars. Every image is kept on disk, so it loads at once and offline.

use std::{
    fs,
    path::{Path, PathBuf},
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

pub fn cache_dir(data_dir: &Path) -> PathBuf {
    data_dir.join("art")
}

pub fn forget(cache: &Path) {
    let _ = fs::remove_dir_all(cache);
}

/// Whether the CDN already answered 404 for `url` (blocking), so nothing asks for it again.
pub fn known_missing(cache: &Path, url: &str) -> bool {
    fs::metadata(file(cache, url)).is_ok_and(|meta| meta.len() == 0)
}

// Disk first and never revalidated: art under one URL does not change.
fn load(cache: &Path, url: &str) -> Result<Vec<u8>> {
    let file = file(cache, url);
    match fs::read(&file) {
        Ok(bytes) if bytes.is_empty() => bail!("{url} does not exist"),
        Ok(bytes) => return Ok(bytes),
        Err(_) => {}
    }
    let response = fetch::get(url, None)?;
    // An empty file marks art the CDN does not have.
    let keep: &[u8] = match response.status {
        200 => &response.body,
        404 => &[],
        status => bail!("GET {url}: HTTP {status}"),
    };
    if let Err(e) = disk::replace(&file, keep) {
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
