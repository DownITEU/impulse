//! Public files over HTTPS: a blocking GET for CDNs (catalog, art, trainer DLLs).

use std::{sync::LazyLock, time::Duration};

use anyhow::{Context, Result};
use ureq::Agent;

// Public CDNs send no CORS headers, so the null-origin bridge cannot read them.
static AGENT: LazyLock<Agent> = LazyLock::new(|| {
    Agent::config_builder()
        .http_status_as_error(false)
        .user_agent("Impulse")
        // Covers connecting and reading the body: a server that stalls must not hold a worker, or a run, forever.
        .timeout_global(Some(Duration::from_secs(120)))
        .build()
        .into()
});

const MAX_BODY: u64 = 64 << 20;

pub struct Response {
    pub status: u16,
    pub etag: Option<String>,
    pub body: Vec<u8>,
}

/// Blocking GET; `etag` makes it conditional (304 has an empty body).
pub fn get(url: &str, etag: Option<&str>) -> Result<Response> {
    let mut request = AGENT.get(url);
    if let Some(tag) = etag {
        request = request.header("If-None-Match", format!("\"{tag}\""));
    }
    let mut response = request.call().with_context(|| format!("GET {url}"))?;
    let etag = response
        .headers()
        .get("etag")
        .and_then(|v| v.to_str().ok())
        .map(|v| v.trim_start_matches("W/").trim_matches('"').to_owned());
    let body = response
        .body_mut()
        .with_config()
        .limit(MAX_BODY)
        .read_to_vec()
        .with_context(|| format!("read {url}"))?;
    Ok(Response {
        status: response.status().as_u16(),
        etag,
        body,
    })
}
