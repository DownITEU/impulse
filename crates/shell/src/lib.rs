//! Impulse's window: `Root` draws the page (`page/`) and the open map over it. Each folder is a slice with its
//! model entities and their views; the page composes them.

mod account;
mod art;
mod assets;
mod bindings;
mod cheats;
mod crash;
mod dialog;
mod dropdown;
mod layer;
mod library;
mod log_view;
mod map;
mod page;
mod resources;
mod root;
mod run;
mod settings;
mod systray;
mod theme;
mod trainer;
mod widgets;

use std::{path::Path, sync::Arc};

use client::{Load, media::Media, storage::Paths};
use gpui_kit::{App, Window};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};

pub use assets::Assets;
pub use root::Root;

pub fn init(data_dir: &Path, cx: &mut App) {
    crash::install();
    theme::init(cx);
    cx.set_http_client(Arc::new(art::CdnClient { cache: Media::Pictures.dir(&Paths::new(data_dir.to_owned())) }));
    log_view::bind_keys(cx);
    map::bind_keys(cx);
    layer::bind_keys(cx);
    dialog::bind_keys(cx);
}

/// The window's Win32 handle.
fn hwnd(window: &Window) -> Option<isize> {
    match HasWindowHandle::window_handle(window).map(|handle| handle.as_raw()) {
        Ok(RawWindowHandle::Win32(handle)) => Some(handle.hwnd.get()),
        _ => None,
    }
}
