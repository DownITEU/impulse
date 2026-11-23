//! GPUI's side of the media cache (`client::media`): the registry as a global, the http client `img(url)` loads
//! through, and pictures decoded within bounds.

use std::{io::Cursor, path::PathBuf, sync::Arc};

use anyhow::{Result, bail};
use futures::future::BoxFuture;
use gpui_kit::{
    App, Global,
    http_client::{AsyncBody, HttpClient, Request, Response, Url, http::HeaderValue},
};
use image::{DynamicImage, ImageFormat, ImageReader, Limits};

use client::{Registry, media};

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
            let bytes = blocking::unblock(move || media::load(&cache, &url, &registry)).await?;
            Ok(Response::new(AsyncBody::from_bytes(bytes.into())))
        })
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
