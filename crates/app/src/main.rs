#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::{env, path::PathBuf, sync::Arc};

use client::{
    Registry,
    storage::{self, Paths},
};
use gpui_kit::{AppContext as _, Bounds, TitlebarOptions, WindowBounds, WindowOptions, px, size};
use model::SourceId;
use shell::Root;
use wand::Wand;

fn main() {
    // One Impulse at a time: a second start shows the first, which may sit in the tray.
    if !tray::claim() {
        return;
    }
    let data_dir = PathBuf::from(env::var_os("LOCALAPPDATA").expect("LOCALAPPDATA is set on Windows")).join("Impulse");
    journal::init(&data_dir);
    // Wand wrote every file before ids named their source.
    storage::migrate(&Paths::new(data_dir.clone()), &SourceId::new(Wand::SOURCE).expect("a valid source id"));
    // Built before the window: its browser bridge takes a moment to come up.
    let wand = Arc::new(Wand::new(data_dir.clone()));
    let registry = Arc::new(
        Registry::default()
            .with_accounts(wand.clone())
            .with_games(wand.clone())
            .with_trainers(wand.clone())
            .with_maps(wand.clone())
            .with_folders(wand.clone())
            .with_caches(wand),
    );
    gpui_kit::application().with_assets(shell::Assets).run(move |cx| {
        gpui_kit::init(cx);
        shell::init(&data_dir, cx);
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds::centered(None, size(px(1280.), px(800.)), cx))),
            titlebar: Some(TitlebarOptions { title: Some("Impulse".into()), appears_transparent: true, ..Default::default() }),
            window_min_size: Some(size(px(1024.), px(640.))),
            ..Default::default()
        };
        // The remote panel's server on the LAN address.
        #[cfg(feature = "remote")]
        let serve: Option<shell::Serve> = Some(|side| {
            let server = remote::Server::bind()?;
            Ok((server.address(), Box::pin(server.serve(side))))
        });
        #[cfg(not(feature = "remote"))]
        let serve = None;
        gpui_kit::open_window(options, cx, |window, cx| cx.new(|cx| Root::new(registry, data_dir, serve, window, cx)))
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
