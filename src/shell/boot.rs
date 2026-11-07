//! Session and library checks, logged until the page shows real data.

use std::path::PathBuf;

use anyhow::Result;
use gpui_kit::App;

use super::library;
use crate::{
    session::{self, Source},
    transport::Transport,
};

pub fn run(transport: Result<Transport>, data_dir: PathBuf, cx: &mut App) {
    let dir = data_dir.clone();
    cx.spawn(
        async move |_| match blocking::unblock(move || library::scan(&dir)).await {
            Ok(found) => log::info!("Library: {found}"),
            Err(e) => log::error!("Library: {e:#}"),
        },
    )
    .detach();
    cx.spawn(async move |_| {
        if let Err(e) = connect(transport, data_dir.join("session.bin")).await {
            log::error!("Session: {e:#}");
        }
    })
    .detach();
}

async fn connect(transport: Result<Transport>, vault: PathBuf) -> Result<()> {
    let transport = transport?;
    let (session, source) = session::connect(&transport, vault).await?;
    let how = match source {
        Source::Restored => "restored",
        Source::Refreshed => "refreshed",
        Source::Wand => "new from Wand",
    };
    let days = (session.expires_in().as_secs() + 43_200) / 86_400;
    log::info!("Session: {how}, expires in {days} d");
    match session.check(&transport).await {
        Ok(n) => log::info!("API check: Enshrouded trainers: {n}"),
        Err(e) => log::error!("API check: {e:#}"),
    }
    Ok(())
}
