//! Impulse's window: `Root` draws the page (`page/`) and the open map over it. Each folder is a slice with its
//! model entities and their views; the page composes them.

mod account;
mod bindings;
mod cheats;
mod crash;
mod layer;
mod library;
mod log_view;
mod map;
mod page;
mod remote;
mod resources;
mod root;
mod run;
mod settings;
mod systray;
mod trainer;
mod ui;

use std::{path::Path, sync::Arc};

use client::{Load, Registry, media::Media, storage::Paths};
use gpui_kit::{App, Window};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};

pub use remote::Serve;
pub use root::Root;
pub use ui::assets::Assets;

pub fn init(data_dir: &Path, registry: Arc<Registry>, cx: &mut App) {
    crash::install();
    ui::theme::init(cx);
    let cache = Media::Pictures.dir(&Paths::new(data_dir.to_owned()));
    cx.set_http_client(Arc::new(ui::art::CdnClient { cache, registry: registry.clone() }));
    ui::art::Router::set(registry, cx);
    log_view::bind_keys(cx);
    map::bind_keys(cx);
    layer::bind_keys(cx);
    ui::dialog::bind_keys(cx);
}

/// The window's Win32 handle.
fn hwnd(window: &Window) -> Option<isize> {
    match HasWindowHandle::window_handle(window).map(|handle| handle.as_raw()) {
        Ok(RawWindowHandle::Win32(handle)) => Some(handle.hwnd.get()),
        _ => None,
    }
}
