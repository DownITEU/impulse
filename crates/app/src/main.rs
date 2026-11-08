#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod catalog;
mod library;
mod play;
mod preferences;
mod session;
mod shell;
mod trainer;
mod transport;
mod wand;

use std::{env, path::PathBuf};

use gpui_kit::{AppContext as _, Bounds, TitlebarOptions, WindowBounds, WindowOptions, px, size};

use crate::{shell::Page, transport::Transport};

fn main() {
    let data_dir =
        PathBuf::from(env::var_os("LOCALAPPDATA").expect("LOCALAPPDATA is set on Windows"))
            .join("Impulse");
    journal::init(&data_dir);
    let transport = Transport::spawn(data_dir.clone());
    gpui_kit::application()
        .with_assets(shell::Assets)
        .run(move |cx| {
            gpui_kit::init(cx);
            shell::init(&data_dir, cx);
            let options = WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                    None,
                    size(px(1280.), px(800.)),
                    cx,
                ))),
                titlebar: Some(TitlebarOptions {
                    title: Some("Impulse".into()),
                    appears_transparent: true,
                    ..Default::default()
                }),
                window_min_size: Some(size(px(1024.), px(640.))),
                ..Default::default()
            };
            gpui_kit::open_window(options, cx, |window, cx| {
                cx.new(|cx| Page::new(transport, data_dir, window, cx))
            })
            .expect("failed to open window");
            // gpui-pre-windows keeps running after the last window closes.
            cx.on_window_closed(|cx, _| {
                if cx.windows().is_empty() {
                    journal::closing();
                    cx.quit();
                }
            })
            .detach();
            cx.activate(true);
        });
}
