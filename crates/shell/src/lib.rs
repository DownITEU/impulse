//! The combined page (`docs/design.md`): the installed games, the open game's trainer and its controls.
//! LAUNCH + ATTACH runs the open game's trainer (`run.rs`); its cheat controls drive the live session.

mod art;
mod assets;
mod bindings;
mod boot;
mod cheats;
mod collections;
mod controls;
mod crash;
mod dialog;
mod docks;
mod folders;
mod glance;
mod header;
mod hero;
mod hud;
mod layer;
mod library;
mod log_view;
mod map;
mod pane;
mod resources;
mod root;
mod run;
mod saved;
mod settings;
mod side;
mod sidebar;
mod sources;
mod status;
mod systray;
mod theme;
mod trainer;
mod widgets;

use std::{path::Path, sync::Arc};

use gpui_kit::{
    App, Context, Entity, FocusHandle, IntoElement, Render, Task, Window,
    component::{
        h_flex,
        input::{InputEvent, InputState},
        v_flex,
    },
    div,
    prelude::*,
    rgb,
};

pub use assets::Assets;
use bindings::Hotkeys;
use collections::Editor;
use docks::Docks;
use folders::FolderRow;
use glance::Glance;
use layer::{Layer, MinimapKey};
use library::{Library, LibraryEvent};
use log_view::{LogEvent, LogView};
use map::{MapEvent, MapView, Minimap};
use resources::{Resources, ResourcesEvent};
pub use root::Root;
use run::{RunEvent, Runs};
use settings::{Settings, Tab};
use sources::Picker;
use theme::*;
use trainer::{Printed, Trainers};
use widgets::grid;

use client::{Load, library::Game, media::Media, storage::Paths, trainer::Sheet};
use model::{Account, Provider};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};

const SIDEBAR_WIDTH: f32 = 248.;

pub fn init(data_dir: &Path, cx: &mut App) {
    crash::install();
    theme::init(cx);
    cx.set_http_client(Arc::new(art::CdnClient { cache: Media::Pictures.dir(&Paths::new(data_dir.to_owned())) }));
    log_view::bind_keys(cx);
    map::bind_keys(cx);
    hud::bind_keys(cx);
    dialog::bind_keys(cx);
}

/// The window's Win32 handle.
fn hwnd(window: &Window) -> Option<isize> {
    match HasWindowHandle::window_handle(window).map(|handle| handle.as_raw()) {
        Ok(RawWindowHandle::Win32(handle)) => Some(handle.hwnd.get()),
        _ => None,
    }
}

/// The open dropdown.
#[derive(Clone, Copy, PartialEq)]
enum Menu {
    Cheat(usize),
    /// A folder row's picks.
    Choices(usize),
    /// A collection chip's actions.
    Collection(usize),
    /// The open game's sources.
    Source,
}

#[derive(Clone, Copy, PartialEq)]
enum Flash {
    Cheat(usize),
}

pub struct Page {
    provider: Arc<dyn Provider>,
    account: Option<Account>,
    // No session: the header says so when no account is known.
    signed_out: bool,
    library: Entity<Library>,
    trainers: Entity<Trainers>,
    // The open game's cheat list as printed; the cheat list's own until it is a view.
    printed: Option<Printed>,
    runs: Entity<Runs>,
    hotkeys: Entity<Hotkeys>,
    // `+ ADD`'s choice of game.
    picker: Option<Picker>,
    settings: Entity<Settings>,
    // The collection being named.
    editor: Option<Editor>,
    settings_open: bool,
    settings_tab: Tab,
    folders: Vec<FolderRow>,
    // The window is the overlay over a game: the HUD replaces the page.
    over: bool,
    tray: Option<tray::Tray>,
    // Impulse ends once the trainer is stopped.
    quitting: bool,
    layer: Entity<Layer>,
    docks: Entity<Docks>,
    // The HUD window over the game; it lives as long as the page.
    _glance: Entity<Glance>,
    // The minimap in its dock over the game.
    minimap: Entity<Minimap>,
    hud_focus: FocusHandle,
    dialog_focus: FocusHandle,
    menu: Option<Menu>,
    // The press-only button showing its confirmation.
    flashed: Option<(Flash, Task<()>)>,
    log: Entity<LogView>,
    log_open: bool,
    map: Entity<MapView>,
    resources: Entity<Resources>,
    game_filter: Entity<InputState>,
    mod_filter: Entity<InputState>,
}

impl Page {
    pub fn new(provider: Arc<dyn Provider>, paths: Paths, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let settings = cx.new(|cx| Settings::new(&paths, cx));
        let hidden = settings.read(cx).prefs().hide_from_capture;
        let mut excluded = hidden;
        cx.observe_in(&settings, window, move |_, settings, window, cx| {
            let hidden = settings.read(cx).prefs().hide_from_capture;
            if hidden != excluded
                && let Some(hwnd) = hwnd(window)
            {
                overlay::exclude_from_capture(hwnd, hidden);
            }
            excluded = hidden;
            cx.notify();
        })
        .detach();
        let library = cx.new(|cx| Library::new(provider.clone(), &paths, settings.clone(), cx));
        cx.observe(&library, |_, _, cx| cx.notify()).detach();
        cx.subscribe_in(&library, window, Self::library_changed).detach();
        let trainers = cx.new(|cx| Trainers::new(provider.clone(), &paths, library.clone(), cx));
        cx.observe(&trainers, |_, _, cx| cx.notify()).detach();
        cx.subscribe_in(&trainers, window, Self::trainer_changed).detach();
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
        let map = cx.new(|cx| MapView::new(provider.clone(), &paths, window, cx));
        cx.subscribe_in(&map, window, |page, _, _: &MapEvent, window, cx| page.close_map(window, cx)).detach();
        let resources = cx.new(|cx| Resources::new(provider.clone(), &paths, cx));
        cx.subscribe_in(&resources, window, |page, _, event: &ResourcesEvent, window, cx| match event {
            ResourcesEvent::Cleared => page.reload(window, cx),
        })
        .detach();
        let mut filter = |placeholder: &'static str| {
            let state = cx.new(|cx| InputState::new(window, cx).placeholder(placeholder));
            cx.subscribe(&state, |_, _, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    cx.notify();
                }
            })
            .detach();
            state
        };
        let (game_filter, mod_filter) = (filter("filter"), filter("filter mods"));
        let layer = cx.new(|cx| Layer::new(&paths, &runs, window, cx));
        // The overlay's and the minimap's hotkeys are not the trainer's to take.
        let reserved = layer.downgrade();
        let reserved = move |chord, cx: &App| reserved.upgrade().is_some_and(|layer| layer.read(cx).takes(chord));
        let hotkeys = cx.new(|cx| Hotkeys::new(&paths, &library, trainers.clone(), runs.clone(), reserved, window, cx));
        cx.observe(&hotkeys, |_, _, cx| cx.notify()).detach();
        cx.observe_in(&layer, window, Self::overlay_changed).detach();
        cx.subscribe(&layer, |page, _, _: &MinimapKey, cx| page.minimap_key(cx)).detach();
        let docks = cx.new(|_| Docks::load(&paths, &hud::DOCKS));
        let live_map = map.read(cx).live_map().clone();
        let glance = cx.new(|cx| Glance::new(&layer, docks.clone(), live_map.clone(), &settings, cx));
        if hidden && let Some(hwnd) = hwnd(window) {
            overlay::exclude_from_capture(hwnd, true);
        }
        let minimap = cx.new(|cx| Minimap::new(live_map.clone(), cx));
        cx.observe_in(&docks, window, |page, _, window, cx| {
            page.sync_map_dock(window, cx);
            page.sync_minimap(cx);
            page.sync_minimap_dock(window, cx);
            cx.notify();
        })
        .detach();
        cx.observe_in(&live_map, window, |page, _, window, cx| page.sync_minimap_dock(window, cx)).detach();
        // Quitting skips drops: the icon would stay until a hover.
        cx.on_app_quit(|page, _| {
            page.tray = None;
            async {}
        })
        .detach();
        let page = Self {
            hotkeys,
            picker: None,
            provider,
            account: None,
            signed_out: false,
            folders: Vec::new(),
            library,
            trainers,
            printed: None,
            runs,
            settings,
            editor: None,
            settings_open: false,
            settings_tab: Tab::Appearance,
            over: false,
            tray: Self::start_tray(window, cx),
            quitting: false,
            layer,
            docks,
            _glance: glance,
            minimap,
            hud_focus: cx.focus_handle(),
            dialog_focus: cx.focus_handle(),
            menu: None,
            flashed: None,
            log,
            log_open: false,
            map,
            resources,
            game_filter,
            mod_filter,
        };
        page.sign_in(window, cx);
        page.scan_folders(window, cx);
        page
    }

    /// The open game, held while drawing.
    fn game(&self, cx: &App) -> Option<Arc<Game>> {
        self.library.read(cx).opened().cloned()
    }

    fn selected(&self, cx: &App) -> Option<Arc<str>> {
        self.library.read(cx).open_id().cloned()
    }

    fn sheet<'a>(&self, game: &str, cx: &'a App) -> Option<&'a Sheet> {
        self.trainers.read(cx).sheet(game)
    }

    fn is_running(&self, cx: &App) -> bool {
        self.runs.read(cx).live().is_some_and(|game| self.library.read(cx).open_id() == Some(game))
    }

    fn select(&self, id: Arc<str>, cx: &mut App) {
        self.library.update(cx, |library, cx| library.open(id, cx));
    }

    fn library_changed(&mut self, _: &Entity<Library>, event: &LibraryEvent, window: &mut Window, cx: &mut Context<Self>) {
        match event {
            LibraryEvent::Opened => {
                self.map.update(cx, |map, cx| map.hide(window, cx));
                // Another trainer's names: the old filter would hide them.
                self.mod_filter.update(cx, |filter, cx| filter.set_value("", window, cx));
                self.drop_cheat_state();
                self.print(window, cx);
                // Over a game, its dock opens the new game's map.
                self.sync_map_dock(window, cx);
            }
            LibraryEvent::Listed | LibraryEvent::Switched => {}
        }
        cx.notify();
    }

    fn run_changed(&mut self, _: Entity<Runs>, event: &RunEvent, cx: &mut Context<Self>) {
        match event {
            RunEvent::Pressed(index) => {
                if self.is_running(cx) {
                    self.flash(Flash::Cheat(*index), cx);
                }
            }
            // The tray's quit waited for the trainer to stop.
            RunEvent::Ended if self.quitting => cx.quit(),
            RunEvent::Live | RunEvent::Stopping | RunEvent::Ended => self.sync_map(cx),
        }
        cx.notify();
    }

    /// The open menu and flash are keyed by cheat index, which means another cheat in another trainer.
    fn drop_cheat_state(&mut self) {
        if matches!(self.menu, Some(Menu::Cheat(_) | Menu::Collection(_))) {
            self.menu = None;
        }
        self.editor = None;
        if matches!(self.flashed, Some((Flash::Cheat(_), _))) {
            self.flashed = None;
        }
    }

    fn flash(&mut self, button: Flash, cx: &mut Context<Self>) {
        let timer = widgets::flash(cx, |page: &mut Self| page.flashed = None);
        self.flashed = Some((button, timer));
        cx.notify();
    }

    fn is_flashed(&self, button: Flash) -> bool {
        self.flashed.as_ref().is_some_and(|(at, _)| *at == button)
    }
}

impl Render for Page {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.over {
            return self.hud(window, cx);
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
                    .child(self.sidebar(cx))
                    .child(self.pane(window, cx))
                    .when(self.log_open, |body| {
                        body.child(div().occlude().absolute().top_0().left_0().size_full().child(self.log.clone()))
                    }),
            )
            .child(self.status_bar(cx))
            .when(self.settings_open, |page| page.child(self.settings_modal(cx)))
            .children(self.collection_editor(cx))
            .children(self.game_picker(cx))
            .into_any_element()
    }
}
