#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod session;
mod shell;
mod transport;
mod wand;

use std::{env, path::PathBuf};

use gpui_kit::{AppContext as _, Bounds, TitlebarOptions, WindowBounds, WindowOptions, px, size};

use crate::{shell::Shell, transport::Transport};

fn main() {
    let data_dir =
        PathBuf::from(env::var_os("LOCALAPPDATA").expect("LOCALAPPDATA is set on Windows"))
            .join("Impulse");
    let vault = data_dir.join("session.bin");
    let transport = Transport::spawn(data_dir);
    gpui_kit::application().run(move |cx| {
        gpui_kit::init(cx);
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                None,
                size(px(480.), px(320.)),
                cx,
            ))),
            titlebar: Some(TitlebarOptions {
                title: Some("Impulse".into()),
                ..Default::default()
            }),
            ..Default::default()
        };
        gpui_kit::open_window(options, cx, |_, cx| {
            cx.new(|cx| Shell::new(transport, vault, cx))
        })
        .expect("failed to open window");
        // gpui-pre-windows keeps running after the last window closes.
        cx.on_window_closed(|cx, _| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();
        cx.activate(true);
    });
}
