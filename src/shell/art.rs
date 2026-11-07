use futures::future::BoxFuture;
use gpui_kit::http_client::{AsyncBody, HttpClient, Request, Response, Url, http::HeaderValue};

use crate::transport::cdn;

/// Lets `img(url)` load public art through the native CDN client.
pub struct CdnClient;

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
        Box::pin(async move {
            let got = blocking::unblock(move || cdn::get(&url, None)).await?;
            let body = AsyncBody::from_bytes(got.body.into());
            Ok(Response::builder().status(got.status).body(body)?)
        })
    }
}

pub fn steam_art(app: u32, file: &str) -> String {
    format!("https://cdn.cloudflare.steamstatic.com/steam/apps/{app}/{file}")
}
