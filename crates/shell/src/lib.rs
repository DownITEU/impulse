//! The combined page (`docs/design.md`): the installed games, the open game's trainer and its controls.
//! LAUNCH + ATTACH runs the open game's trainer (`run.rs`); its cheat controls drive the live session.

mod account;
mod art;
mod assets;
mod bindings;
mod boot;
mod cheats;
mod crash;
mod dialog;
mod dropdown;
mod header;
mod hero;
mod layer;
mod library;
mod log_view;
mod map;
mod pane;
mod resources;
mod root;
mod run;
mod settings;
mod status;
mod systray;
mod theme;
mod trainer;
mod widgets;

use std::{path::Path, sync::Arc};

use gpui_kit::{
    App, Context, Entity, IntoElement, Render, Window,
    component::{h_flex, v_flex},
    div,
    prelude::*,
    rgb,
};

use account::Account;
pub use assets::Assets;
use bindings::HotkeySettings;
use bindings::Hotkeys;
use cheats::CheatList;
use hero::Hero;
use layer::{Entered, Hud, Layer, OverlaySettings};
use library::{GamePicker, Library, Marks, Sidebar};
use log_view::{LogEvent, LogView};
use map::{MapEvent, MapView, Maps};
use resources::{Resources, ResourcesEvent, ResourcesView};
pub use root::Root;
use run::{RunEvent, Runs};
use settings::{Folders, Looked, Settings, SettingsView, Tabs};
use theme::*;
use trainer::Trainers;
use widgets::grid;

use client::{Load, library::Game, media::Media, storage::Paths};
use model::Provider;
use raw_window_handle::{HasWindowHandle, RawWindowHandle};

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

pub struct Page {
    account: Entity<Account>,
    library: Entity<Library>,
    trainers: Entity<Trainers>,
    runs: Entity<Runs>,
    hotkeys: Entity<Hotkeys>,
    sidebar: Entity<Sidebar>,
    picker: Entity<GamePicker>,
    settings: Entity<Settings>,
    settings_view: Entity<SettingsView>,
    tray: Option<tray::Tray>,
    // Impulse ends once the trainer is stopped.
    quitting: bool,
    layer: Entity<Layer>,
    // Drawn in place of the page while over a game.
    hud: Entity<Hud>,
    log: Entity<LogView>,
    log_open: bool,
    maps: Entity<Maps>,
    map: Entity<MapView>,
    resources: Entity<Resources>,
    cheat_list: Entity<CheatList>,
    hero: Entity<Hero>,
}

impl Page {
    pub fn new(provider: Arc<dyn Provider>, paths: Paths, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let settings = cx.new(|cx| Settings::new(&paths, cx));
        cx.observe(&settings, |_, _, cx| cx.notify()).detach();
        let folders = cx.new(|cx| Folders::new(provider.clone(), cx));
        let account = cx.new(|cx| Account::new(provider.clone(), cx));
        cx.observe(&account, |_, _, cx| cx.notify()).detach();
        let library = cx.new(|cx| Library::new(provider.clone(), &paths, settings.clone(), cx));
        cx.observe(&library, |_, _, cx| cx.notify()).detach();
        let trainers = cx.new(|cx| Trainers::new(provider.clone(), &paths, library.clone(), cx));
        cx.observe(&trainers, |_, _, cx| cx.notify()).detach();
        let runs = cx.new(|cx| Runs::new(library.clone(), trainers.clone(), cx));
        cx.observe(&runs, |_, _, cx| cx.notify()).detach();
        cx.subscribe(&runs, Self::run_changed).detach();
        crash::watch(&runs, cx);
        let log = cx.new(|cx| LogView::new(window, cx));
        cx.subscribe(&log, |page, _, event: &LogEvent, cx| {
            if matches!(event, LogEvent::Close) {
                page.log_open = false;
            }
            cx.notify();
        })
        .detach();
        let maps = cx.new(|cx| Maps::new(provider.clone(), &paths, library.clone(), runs.clone(), cx));
        let map = cx.new(|cx| MapView::new(maps.clone(), &library, &paths, window, cx));
        cx.subscribe_in(&map, window, |page, map, _: &MapEvent, window, cx| {
            // Over a game the map is a dock, and Escape's last step leaves the overlay.
            if page.layer.read(cx).is_over() {
                return page.layer.read(cx).leave();
            }
            map.update(cx, |map, cx| map.hide(window, cx));
        })
        .detach();
        let resources = cx.new(|cx| Resources::new(provider, &paths, cx));
        cx.subscribe(&resources, |page, _, event: &ResourcesEvent, cx| match event {
            ResourcesEvent::Cleared => page.reload(cx),
        })
        .detach();
        let picker = cx.new(|cx| GamePicker::new(library.clone(), cx));
        cx.observe(&picker, |_, _, cx| cx.notify()).detach();
        let marks = {
            let (trainers, runs) = (trainers.clone(), runs.clone());
            move |game: &str, cx: &App| Marks { favorite: trainers.read(cx).profile(game).favorite, live: runs.read(cx).is_live(game) }
        };
        let sidebar = cx.new(|cx| {
            // What `marks` reads.
            cx.observe(&trainers, |_, _, cx| cx.notify()).detach();
            cx.observe(&runs, |_, _, cx| cx.notify()).detach();
            Sidebar::new(library.clone(), settings.clone(), picker.clone(), marks, window, cx)
        });
        let layer = cx.new(|cx| Layer::new(&paths, &runs, &settings, window, cx));
        let hero = cx.new(|cx| Hero::new(library.clone(), trainers.clone(), runs.clone(), layer.clone(), cx));
        // The overlay's and the minimap's hotkeys are not the trainer's to take.
        let reserved = layer.downgrade();
        let reserved = move |chord, cx: &App| reserved.upgrade().is_some_and(|layer| layer.read(cx).takes(chord));
        let hotkeys = cx.new(|cx| Hotkeys::new(&paths, &library, trainers.clone(), runs.clone(), reserved, window, cx));
        cx.observe(&hotkeys, |_, _, cx| cx.notify()).detach();
        let cheat_list = cx.new(|cx| CheatList::new(library.clone(), trainers.clone(), runs.clone(), hotkeys.clone(), window, cx));
        cx.observe(&cheat_list, |_, _, cx| cx.notify()).detach();
        let tabs = Tabs {
            overlay: cx.new(|cx| OverlaySettings::new(layer.clone(), hotkeys.clone(), trainers.clone(), cx)).into(),
            hotkeys: cx.new(|cx| HotkeySettings::new(hotkeys.clone(), cx)).into(),
            resources: cx.new(|cx| ResourcesView::new(resources.clone(), cx)).into(),
        };
        let settings_view = cx.new(|cx| SettingsView::new(settings.clone(), folders, tabs, window, cx));
        cx.observe(&settings_view, |_, _, cx| cx.notify()).detach();
        cx.subscribe(&settings_view, |page, _, _: &Looked, cx| page.resources.update(cx, |resources, cx| resources.measure(cx))).detach();
        let hud = cx.new(|cx| {
            let (library, trainers, runs, hotkeys) = (library.clone(), trainers.clone(), runs.clone(), hotkeys.clone());
            Hud::new(
                layer.clone(),
                library,
                trainers,
                runs,
                hotkeys,
                maps.clone(),
                map.clone(),
                cheat_list.clone(),
                &settings,
                &paths,
                window,
                cx,
            )
        });
        cx.observe(&layer, |_, _, cx| cx.notify()).detach();
        cx.subscribe(&hud, |page, _, _: &Entered, cx| {
            page.hero.update(cx, |hero, cx| hero.close(cx));
            page.settings_view.update(cx, |view, cx| view.close(cx));
            page.log_open = false;
            cx.notify();
        })
        .detach();
        // Quitting skips drops: the icon would stay until a hover.
        cx.on_app_quit(|page, _| {
            page.tray = None;
            async {}
        })
        .detach();
        Self {
            hotkeys,
            sidebar,
            picker,
            account,
            library,
            trainers,
            runs,
            settings,
            settings_view,
            tray: Self::start_tray(window, cx),
            quitting: false,
            layer,
            hud,
            log,
            log_open: false,
            maps,
            map,
            resources,
            cheat_list,
            hero,
        }
    }

    /// The open game, held while drawing.
    fn game(&self, cx: &App) -> Option<Arc<Game>> {
        self.library.read(cx).opened().cloned()
    }

    /// The open map shows over the page; over a game it is in a dock.
    fn map_modal(&self, cx: &App) -> bool {
        !self.layer.read(cx).is_over() && self.maps.read(cx).is_open(cx)
    }

    fn run_changed(&mut self, _: Entity<Runs>, event: &RunEvent, cx: &mut Context<Self>) {
        match event {
            // The tray's quit waited for the trainer to stop.
            RunEvent::Ended if self.quitting => cx.quit(),
            _ => {}
        }
        cx.notify();
    }
}

impl Render for Page {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.layer.read(cx).is_over() {
            return self.hud.clone().into_any_element();
        }
        v_flex()
            .relative()
            .size_full()
            .bg(rgb(BG))
            .child(grid(32., line(0.025)))
            .child(self.header(window, cx))
            .child(
                h_flex()
                    .flex_1()
                    .min_h_0()
                    .items_start()
                    .child(self.sidebar.clone())
                    .child(self.pane(window, cx))
                    .when(self.log_open, |body| {
                        body.child(div().occlude().absolute().top_0().left_0().size_full().child(self.log.clone()))
                    }),
            )
            .child(self.status_bar(cx))
            .when(self.settings_view.read(cx).is_open(), |page| page.child(self.settings_view.clone()))
            .children(self.cheat_list.read(cx).editor().cloned())
            .when(self.picker.read(cx).is_open(), |page| page.child(self.picker.clone()))
            .into_any_element()
    }
}
