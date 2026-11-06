use std::{
    borrow::Cow,
    collections::HashMap,
    path::Path,
    sync::{Arc, Mutex, mpsc::SyncSender},
    thread,
    time::{Duration, Instant},
};

use anyhow::{Result, anyhow};
use futures::channel::oneshot;
use serde::{Deserialize, Serialize};
use tao::{
    event::Event,
    event_loop::{ControlFlow, EventLoop, EventLoopBuilder, EventLoopProxy},
    platform::windows::{EventLoopBuilderExtWindows, WindowBuilderExtWindows},
    window::{Window, WindowBuilder},
};
use webview2_com::Error::WindowsError;
use wry::{WebContext, WebView, WebViewBuilder, http::Response as HttpResponse};

use super::{Cmd, Request, Response};

type Pending = Arc<Mutex<HashMap<u64, oneshot::Sender<Result<Response>>>>>;

#[derive(Serialize)]
struct Call<'a> {
    id: u64,
    #[serde(flatten)]
    request: &'a Request,
}

#[derive(Deserialize)]
struct Reply {
    id: u64,
    status: Option<u16>,
    body: Option<String>,
    error: Option<String>,
}

pub(super) fn run(data_dir: &Path, started: SyncSender<EventLoopProxy<Cmd>>) {
    let event_loop = EventLoopBuilder::<Cmd>::with_user_event()
        .with_any_thread(true)
        .with_dpi_aware(false)
        .build();
    let proxy = event_loop.create_proxy();
    let _ = started.send(proxy.clone());
    let pending = Pending::default();
    // Commands sent meanwhile wait in the proxy queue. `run` never returns, so the parts stay alive.
    let built = build_retrying(data_dir, &event_loop, proxy, pending.clone());

    let (mut ready, mut queue, mut next_id) = (false, Vec::new(), 0u64);
    event_loop.run(move |event, _, flow| {
        *flow = ControlFlow::Wait;
        let Event::UserEvent(cmd) = event else { return };
        let webview = match &built {
            Ok((_, _, webview)) => webview,
            Err(e) => {
                if let Cmd::Fetch(_, tx) = cmd {
                    let _ = tx.send(Err(anyhow!("{e:#}")));
                }
                return;
            }
        };
        match cmd {
            Cmd::Ready => ready = true,
            Cmd::Fetch(request, tx) => queue.push((request, tx)),
        }
        if !ready {
            return;
        }
        for (request, tx) in queue.drain(..) {
            next_id += 1;
            pending.lock().unwrap().insert(next_id, tx);
            let sent = serde_json::to_string(&Call {
                id: next_id,
                request: &request,
            })
            .is_ok_and(|json| webview.evaluate_script(&format!("impulse({json})")).is_ok());
            if !sent && let Some(tx) = pending.lock().unwrap().remove(&next_id) {
                let _ = tx.send(Err(anyhow!("webview dispatch failed")));
            }
        }
    });
}

// After some hard kills WebView2 keeps the profile busy for about 50 s; a clean exit frees it at once.
fn build_retrying(
    data_dir: &Path,
    event_loop: &EventLoop<Cmd>,
    proxy: EventLoopProxy<Cmd>,
    pending: Pending,
) -> Result<(Window, WebContext, WebView)> {
    const ERROR_BUSY: i32 = 0x800700AA_u32 as i32;
    let deadline = Instant::now() + Duration::from_secs(90);
    loop {
        match build(data_dir, event_loop, proxy.clone(), pending.clone()) {
            Err(e)
                if Instant::now() < deadline
                    && matches!(e.downcast_ref(), Some(wry::Error::WebView2Error(WindowsError(w))) if w.code().0 == ERROR_BUSY) =>
            {
                thread::sleep(Duration::from_secs(1));
            }
            built => return built,
        }
    }
}

// COM: WebViewBuilder::build runs CoInitializeEx(STA) on this thread, and tao skips OLE with drag and drop off.
fn build(
    data_dir: &Path,
    event_loop: &EventLoop<Cmd>,
    proxy: EventLoopProxy<Cmd>,
    pending: Pending,
) -> Result<(Window, WebContext, WebView)> {
    let window = WindowBuilder::new()
        .with_visible(false)
        .with_drag_and_drop(false)
        .build(event_loop)?;
    let mut context = WebContext::new(Some(data_dir.join("webview")));
    let url = url::Url::from_file_path(data_dir.join("bridge.html"))
        .map_err(|()| anyhow!("data dir is not an absolute path"))?;
    let webview = WebViewBuilder::new_with_web_context(&mut context)
        .with_custom_protocol("impulse".into(), move |_, request| {
            match request.uri().path() {
                "/ready" => {
                    let _ = proxy.send_event(Cmd::Ready);
                }
                "/response" => reply(&pending, request.body()),
                _ => {}
            }
            HttpResponse::builder()
                .status(204)
                .header("Access-Control-Allow-Origin", "*")
                .body(Cow::Borrowed(&[][..]))
                .unwrap()
        })
        .with_url(url.as_str())
        .build(&window)?;
    Ok((window, context, webview))
}

fn reply(pending: &Pending, raw: &[u8]) {
    let Ok(reply) = serde_json::from_slice::<Reply>(raw) else {
        return;
    };
    let result = match reply.status {
        Some(status) => Ok(Response {
            status,
            body: reply.body.unwrap_or_default(),
        }),
        None => Err(anyhow!("fetch failed: {}", reply.error.unwrap_or_default())),
    };
    if let Some(tx) = pending.lock().unwrap().remove(&reply.id) {
        let _ = tx.send(result);
    }
}
