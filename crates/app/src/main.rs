#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod plugins;

use std::{env, path::PathBuf, sync::Arc};

use client::{
    Capabilities, Registry,
    storage::{self, Paths},
};
use gpui_kit::{AppContext as _, Bounds, TitlebarOptions, WindowBounds, WindowOptions, px, size};
use model::SourceId;
use shell::Root;
use wand::Wand;

use self::plugins::Host;

fn main() {
    // A copy started elevated for a game run as administrator: it loads the DLL and exits.
    if let Some(code) = agent::serve_elevated(Wand::PUBLISHERS) {
        std::process::exit(code);
    }
    // One Impulse at a time: a second start shows the first, which may sit in the tray. An update's restart
    // waits for the one it replaces instead.
    if !tray::claim(updater::restarted()) {
        return;
    }
    let data_dir = PathBuf::from(env::var_os("LOCALAPPDATA").expect("LOCALAPPDATA is set on Windows")).join("Impulse");
    journal::init(&data_dir);
    if let Ok(exe) = env::current_exe() {
        updater::cleanup(&exe);
    }
    let paths = Paths::new(data_dir.clone());
    // The v1 files have unqualified ids from the original single source.
    storage::migrate(&paths, &SourceId::new(Wand::SOURCE).expect("a valid source id"));
    // Built before the window: its browser bridge takes a moment to come up.
    let wand = Arc::new(Wand::new(data_dir.clone()));
    let registry = Arc::new(Registry::default());
    // The title bar marks it.
    let elevated = agent::elevated();
    registry.add(Capabilities {
        accounts: Some(wand.clone()),
        games: Some(wand.clone()),
        trainers: Some(wand.clone()),
        maps: Some(wand.clone()),
        folders: Some(wand.clone()),
        caches: Some(wand),
        ..Capabilities::default()
    });
    // The crate, not this module of the same name.
    let store = ::plugins::Store::new(paths.plugins(), paths.plugin_lock(), paths.plugin_cache());
    let plugins = Arc::new(client::plugins::Plugins::new(Arc::new(Host(store)), registry.clone()));
    plugins.start();
    gpui_kit::application().with_assets(shell::Assets).run(move |cx| {
        gpui_kit::init(cx);
        shell::init(&data_dir, registry.clone(), cx);
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
        gpui_kit::open_window(options, cx, |window, cx| cx.new(|cx| Root::new(registry, plugins, data_dir, serve, elevated, window, cx)))
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
