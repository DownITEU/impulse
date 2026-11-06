mod webview;

use std::{fs, path::PathBuf, sync::mpsc, thread};

use anyhow::{Context, Result, anyhow};
use futures::channel::oneshot;
use serde::Serialize;
use tao::event_loop::EventLoopProxy;

const BRIDGE: &str = include_str!("bridge.html");

#[derive(Serialize)]
pub struct Request {
    pub method: String,
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: Option<String>,
}

pub struct Response {
    pub status: u16,
    pub body: String,
}

enum Cmd {
    Fetch(Request, oneshot::Sender<Result<Response>>),
    Ready,
}

#[derive(Clone)]
pub struct Transport {
    proxy: EventLoopProxy<Cmd>,
}

impl Transport {
    pub fn spawn(data_dir: PathBuf) -> Result<Self> {
        fs::create_dir_all(&data_dir).context("create data dir")?;
        fs::write(data_dir.join("bridge.html"), BRIDGE).context("write bridge.html")?;
        let (started, wait) = mpsc::sync_channel(0);
        thread::Builder::new()
            .name("impulse-transport".into())
            .spawn(move || webview::run(&data_dir, started))
            .context("start transport thread")?;
        let proxy = wait.recv().context("transport thread exited")?;
        Ok(Self { proxy })
    }

    pub async fn fetch(&self, request: Request) -> Result<Response> {
        let (tx, rx) = oneshot::channel();
        self.proxy
            .send_event(Cmd::Fetch(request, tx))
            .map_err(|_| anyhow!("transport is closed"))?;
        rx.await
            .map_err(|_| anyhow!("transport dropped the request"))?
    }
}
