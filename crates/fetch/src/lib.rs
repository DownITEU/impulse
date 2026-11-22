//! Public files over HTTPS: a blocking GET for CDNs (catalog, art, trainer DLLs).

use std::{sync::LazyLock, time::Duration};

use anyhow::{Context, Result};
use ureq::Agent;

mod guarded;

pub use guarded::{Refused, guarded};

// Public CDNs send no CORS headers, so the null-origin bridge cannot read them.
static AGENT: LazyLock<Agent> = LazyLock::new(|| {
    Agent::config_builder()
        .http_status_as_error(false)
        .user_agent("Impulse")
        // Covers connecting and reading the body: a server that stalls must not hold a worker, or a run, forever.
        .timeout_global(Some(Duration::from_secs(120)))
        // Map tiles load 6 at a time; a reopened connection costs a TLS handshake.
        .max_idle_connections_per_host(6)
        .max_idle_age(Duration::from_secs(90))
        .build()
        .into()
});

const MAX_BODY: u64 = 64 << 20;

pub struct Response {
    pub status: u16,
    pub etag: Option<String>,
    pub body: Vec<u8>,
    /// Who answered an error (`server`, `cf-ray`, `cf-cache-status`), for the log; empty for a success.
    pub edge: String,
}

pub(crate) fn etag_of(headers: &ureq::http::HeaderMap) -> Option<String> {
    headers.get("etag").and_then(|v| v.to_str().ok()).map(|v| v.trim_start_matches("W/").trim_matches('"').to_owned())
}

pub(crate) fn edge_of(status: u16, headers: &ureq::http::HeaderMap) -> String {
    if status < 400 {
        return String::new();
    }
    let named = |name| headers.get(name).and_then(|v| v.to_str().ok()).map(|v| format!("{name} {v}"));
    ["server", "cf-ray", "cf-cache-status"].into_iter().filter_map(named).collect::<Vec<_>>().join(", ")
}

/// Blocking GET; `etag` makes it conditional (304 has an empty body).
pub fn get(url: &str, etag: Option<&str>) -> Result<Response> {
    let mut request = AGENT.get(url);
    if let Some(tag) = etag {
        request = request.header("If-None-Match", format!("\"{tag}\""));
    }
    let mut response = request.call().with_context(|| format!("GET {url}"))?;
    let etag = etag_of(response.headers());
    let status = response.status().as_u16();
    let edge = edge_of(status, response.headers());
    // An error's body is never used: one past the limit must not hide its status. A 304 sends none, yet may name an
    // encoding (GitHub's does): reading it would fail in the decoder.
    let body = match status {
        304 | 400.. => Vec::new(),
        _ => response.body_mut().with_config().limit(MAX_BODY).read_to_vec().with_context(|| format!("read {url}"))?,
    };
    Ok(Response { status, etag, body, edge })
}

#[cfg(test)]
mod tests {
    use std::{
        io::{Read, Write},
        net::TcpListener,
        thread,
    };

    use super::*;

    /// GitHub answers a conditional request with a 304 that names an encoding and sends no body.
    #[test]
    fn a_not_modified_answer_that_names_an_encoding_has_no_body() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/feed", listener.local_addr().unwrap());
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let _ = stream.read(&mut [0; 1024]);
            stream.write_all(b"HTTP/1.1 304 Not Modified\r\nETag: \"x\"\r\nContent-Encoding: gzip\r\nConnection: close\r\n\r\n").unwrap();
        });
        let response = get(&url, Some("x")).unwrap();
        server.join().unwrap();
        assert_eq!((response.status, response.etag.as_deref(), response.body.len()), (304, Some("x"), 0));
    }
}
