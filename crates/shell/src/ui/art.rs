//! Pictures, map tiles and pin videos: from public CDNs (`http(s)://`) and from a source's own URLs (any other
//! scheme, through the registry). Each is kept on disk, so it loads at once and offline; an empty file marks one the
//! server lacks.

use std::{
    fs,
    io::Cursor,
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, SystemTime},
};

use anyhow::{Result, anyhow, bail};
use futures::future::BoxFuture;
use gpui_kit::{
    App, Global,
    http_client::{AsyncBody, HttpClient, Request, Response, Url, http::HeaderValue},
};
use image::{DynamicImage, ImageFormat, ImageReader, Limits};

use client::{Registry, media::file};
use model::FailureKind;

/// The registry for the fetches that run off the UI thread: read it on the UI thread, move the `Arc` into the closure.
pub struct Router(Arc<Registry>);

impl Global for Router {}

impl Router {
    pub fn set(registry: Arc<Registry>, cx: &mut App) {
        cx.set_global(Self(registry));
    }

    pub fn registry(cx: &App) -> Arc<Registry> {
        cx.global::<Self>().0.clone()
    }
}

/// Lets `img(url)` load art through the disk cache and the native CDN client; a source's URL goes to the registry.
pub struct CdnClient {
    pub cache: PathBuf,
    pub registry: Arc<Registry>,
}

impl HttpClient for CdnClient {
    fn user_agent(&self) -> Option<&HeaderValue> {
        None
    }

    fn proxy(&self) -> Option<&Url> {
        None
    }

    fn send(&self, req: Request<AsyncBody>) -> BoxFuture<'static, anyhow::Result<Response<AsyncBody>>> {
        let url = req.uri().to_string();
        let (cache, registry) = (self.cache.clone(), self.registry.clone());
        Box::pin(async move {
            let bytes = blocking::unblock(move || load(&cache, &url, &registry)).await?;
            Ok(Response::new(AsyncBody::from_bytes(bytes.into())))
        })
    }
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

/// A WebP, PNG or JPEG, told by its first bytes, at most `side` texels across: a few compressed bytes can
/// claim a picture too large to hold.
pub fn picture(bytes: &[u8], side: u32) -> Result<DynamicImage> {
    let format = image::guess_format(bytes)?;
    if !matches!(format, ImageFormat::WebP | ImageFormat::Png | ImageFormat::Jpeg) {
        bail!("not a picture: {format:?}");
    }
    let mut reader = ImageReader::with_format(Cursor::new(bytes), format);
    let mut limits = Limits::default();
    (limits.max_image_width, limits.max_image_height) = (Some(side), Some(side));
    reader.limits(limits);
    Ok(reader.decode()?)
}

const ABSENT: [u16; 1] = [404];
// A provider serving a window of a larger pyramid answers 403 or 410 outside it.
const ABSENT_TILE: [u16; 3] = [403, 404, 410];

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
    // An empty file marks what the server does not have.
    if let Err(e) = disk::replace(file, if gone { &[] } else { &body }) {
        log::warn!("art not cached: {e:#}");
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
