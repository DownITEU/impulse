//! One connection: a request head read within limits, then a panel file, pairing, a cover or the WebSocket.
//! Every answer closes the connection.

use std::net::SocketAddr;

use async_net::TcpStream;
use client::api::{Ask, ServerSide};
use futures::{AsyncReadExt, AsyncWriteExt, channel::oneshot};
use http::StatusCode;
use rust_embed::Embed;

use crate::{socket, within};

const HEAD: usize = 8 * 1024;
const BODY: usize = 256;
const COOKIE: &str = "impulse";

#[derive(Embed)]
#[folder = "../../web/panel/dist"]
#[allow_missing = true]
struct Panel;

#[derive(Default)]
struct Request {
    method: String,
    path: String,
    host: Option<String>,
    origin: Option<String>,
    token: Option<String>,
    user_agent: String,
    length: Option<usize>,
}

pub async fn connection(mut stream: TcpStream, address: SocketAddr, side: &ServerSide) {
    let Some(Some((mut read, at, request))) = within(head(&mut stream)).await else { return };
    let (host, origin) = (address.to_string(), format!("http://{address}"));
    if request.host.as_deref() != Some(host.as_str()) {
        return refuse(&mut stream, StatusCode::MISDIRECTED_REQUEST).await;
    }
    let ours = request.origin.as_deref() == Some(origin.as_str());
    match (request.method.as_str(), request.path.as_str()) {
        ("GET", "/ws") if ours => match device(&request, side).await {
            Some(device) => socket::serve(stream, read, device, side).await,
            None => refuse(&mut stream, StatusCode::UNAUTHORIZED).await,
        },
        ("POST", "/pair") if ours && request.length.is_some_and(|length| length <= BODY) => {
            let end = at + request.length.unwrap_or_default();
            let Some(Some(())) = within(body(&mut stream, &mut read, end)).await else { return };
            pair(&mut stream, String::from_utf8_lossy(&read[at..]).trim(), &request.user_agent, side).await;
        }
        ("GET", "/ws") | ("POST", "/pair") => refuse(&mut stream, StatusCode::FORBIDDEN).await,
        ("GET", path) if path.starts_with("/art/") => {
            let game = path["/art/".len()..].parse().ok();
            let cover = match (device(&request, side).await, game) {
                (Some(_), Some(game)) => ask(side, |reply| Ask::Art { game, reply }).await.flatten(),
                _ => None,
            };
            match cover {
                Some(bytes) => answer(&mut stream, StatusCode::OK, image(&bytes), &bytes, "").await,
                None => refuse(&mut stream, StatusCode::NOT_FOUND).await,
            }
        }
        ("GET", path) => file(&mut stream, path).await,
        _ => refuse(&mut stream, StatusCode::NOT_FOUND).await,
    }
}

/// The device whose cookie the request carries; the app decides.
async fn device(request: &Request, side: &ServerSide) -> Option<u32> {
    let token = request.token.clone()?;
    ask(side, |reply| Ask::Device { token, reply }).await.flatten()
}

async fn pair(stream: &mut TcpStream, code: &str, user_agent: &str, side: &ServerSide) {
    let (code, user_agent) = (code.to_owned(), user_agent.to_owned());
    match ask(side, |reply| Ask::Pair { code, user_agent, reply }).await.flatten() {
        Some(token) => {
            let cookie = format!("Set-Cookie: {COOKIE}={token}; HttpOnly; SameSite=Strict; Path=/; Max-Age=31536000\r\n");
            answer(stream, StatusCode::OK, "text/plain", b"", &cookie).await;
        }
        None => refuse(stream, StatusCode::FORBIDDEN).await,
    }
}

/// Asks the app and waits for its reply; `None`: the app is gone or stalled.
async fn ask<T>(side: &ServerSide, make: impl FnOnce(oneshot::Sender<T>) -> Ask) -> Option<T> {
    let (reply, answer) = oneshot::channel();
    let asked = async {
        if !side.ask(make(reply)).await {
            return None;
        }
        answer.await.ok()
    };
    within(asked).await.flatten()
}

async fn file(stream: &mut TcpStream, path: &str) {
    let name = match path.split('?').next().unwrap_or_default().trim_start_matches('/') {
        "" => "index.html",
        name => name,
    };
    match Panel::get(name) {
        Some(file) => answer(stream, StatusCode::OK, kind(name), &file.data, "").await,
        None if name == "index.html" => {
            let text = b"The panel is not built: scripts\\panel.ps1";
            answer(stream, StatusCode::SERVICE_UNAVAILABLE, "text/plain", text, "").await;
        }
        None => refuse(stream, StatusCode::NOT_FOUND).await,
    }
}

fn kind(name: &str) -> &'static str {
    match name.rsplit('.').next().unwrap_or_default() {
        "html" => "text/html; charset=utf-8",
        "js" => "text/javascript",
        "css" => "text/css",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "woff2" => "font/woff2",
        "json" | "webmanifest" => "application/json",
        _ => "application/octet-stream",
    }
}

fn image(bytes: &[u8]) -> &'static str {
    match bytes {
        [0x89, b'P', b'N', b'G', ..] => "image/png",
        [0xff, 0xd8, ..] => "image/jpeg",
        [b'R', b'I', b'F', b'F', _, _, _, _, b'W', b'E', b'B', b'P', ..] => "image/webp",
        _ => "application/octet-stream",
    }
}

/// The request head, bounded, with what was read and where the head ends.
async fn head(stream: &mut TcpStream) -> Option<(Vec<u8>, usize, Request)> {
    let mut read = Vec::with_capacity(1024);
    let mut chunk = [0u8; 1024];
    loop {
        let count = stream.read(&mut chunk).await.ok().filter(|count| *count > 0)?;
        read.extend_from_slice(&chunk[..count]);
        let mut headers = [httparse::EMPTY_HEADER; 32];
        let mut parsed = httparse::Request::new(&mut headers);
        match parsed.parse(&read).ok()? {
            httparse::Status::Complete(at) if at <= HEAD => {
                let request = request(&parsed)?;
                return Some((read, at, request));
            }
            httparse::Status::Partial if read.len() < HEAD => {}
            _ => return None,
        }
    }
}

fn request(parsed: &httparse::Request) -> Option<Request> {
    let mut request = Request { method: parsed.method?.to_owned(), path: parsed.path?.to_owned(), ..Request::default() };
    for header in parsed.headers.iter() {
        let value = String::from_utf8_lossy(header.value).into_owned();
        match header.name.to_ascii_lowercase().as_str() {
            "host" if request.host.is_none() => request.host = Some(value),
            "origin" if request.origin.is_none() => request.origin = Some(value),
            "content-length" if request.length.is_none() => request.length = Some(value.trim().parse().ok()?),
            // Twice is ambiguous; a chunked body is never ours.
            "host" | "origin" | "content-length" | "transfer-encoding" => return None,
            "user-agent" => request.user_agent = value,
            "cookie" => {
                let ours = value.split(';').find_map(|cookie| cookie.trim().strip_prefix(COOKIE)?.strip_prefix('='));
                request.token = ours.map(str::to_owned);
            }
            _ => {}
        }
    }
    Some(request)
}

async fn body(stream: &mut TcpStream, read: &mut Vec<u8>, end: usize) -> Option<()> {
    let mut chunk = [0u8; BODY];
    while read.len() < end {
        let count = stream.read(&mut chunk).await.ok().filter(|count| *count > 0)?;
        read.extend_from_slice(&chunk[..count]);
    }
    read.truncate(end);
    Some(())
}

async fn refuse(stream: &mut TcpStream, status: StatusCode) {
    answer(stream, status, "text/plain", b"", "").await;
}

async fn answer(stream: &mut TcpStream, status: StatusCode, kind: &str, body: &[u8], extra: &str) {
    let head = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {kind}\r\nContent-Length: {}\r\nCache-Control: no-store\r\n\
         X-Content-Type-Options: nosniff\r\nReferrer-Policy: no-referrer\r\nContent-Security-Policy: frame-ancestors 'none'\r\n\
         Connection: close\r\n{extra}\r\n",
        body.len()
    );
    let write = async {
        stream.write_all(head.as_bytes()).await?;
        stream.write_all(body).await?;
        stream.flush().await
    };
    let _ = within(write).await;
}
