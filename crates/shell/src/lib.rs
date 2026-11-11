//! The combined page (`docs/design.md`): the installed games, the open game's trainer and its controls.
//! LAUNCH + ATTACH runs the open game's trainer (`run.rs`); its cheat controls drive the live session.

mod art;
mod assets;
mod bindings;
mod boot;
mod cheats;
mod collections;
mod controls;
mod dialog;
mod docks;
mod folders;
mod header;
mod hero;
mod hud;
mod layer;
mod library;
mod loaded;
mod log_view;
mod map;
mod pane;
mod prefs;
mod profile;
mod resources;
mod root;
mod run;
mod saved;
mod settings;
mod side;
mod sidebar;
mod status;
mod store;
mod theme;
mod widgets;

use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    sync::Arc,
};

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
use bindings::Bindings;
use collections::Editor;
use docks::Docks;
use folders::FolderRow;
use layer::Layer;
use library::{Game, Library};
use loaded::Loaded;
use log_view::{LogEvent, LogView};
use map::{MapEvent, MapView};
use prefs::{Lead, Panels, Prefs};
use profile::Profiles;
use resources::{Resources, ResourcesEvent};
pub use root::Root;
use run::Play;
use settings::Tab;
use theme::*;
use widgets::grid;

use model::{Account, Provider};

const SIDEBAR_WIDTH: f32 = 248.;

pub fn init(data_dir: &Path, cx: &mut App) {
    theme::init(cx);
    cx.set_http_client(Arc::new(art::CdnClient {
        cache: art::Media::Pictures.dir(data_dir),
    }));
    log_view::bind_keys(cx);
    map::bind_keys(cx);
    hud::bind_keys(cx);
    dialog::bind_keys(cx);
}

/// Something that arrives after the window opens.
enum Load<T> {
    Pending,
    Ready(T),
    Failed(String),
}

/// The open dropdown.
#[derive(Clone, Copy, PartialEq)]
enum Menu {
    Cheat(usize),
    /// A folder row's picks.
    Choices(usize),
    /// A collection chip's actions.
    Collection(usize),
}

#[derive(Clone, Copy, PartialEq)]
enum Flash {
    Cheat(usize),
}

pub struct Page {
    data_dir: PathBuf,
    provider: Arc<dyn Provider>,
    account: Option<Account>,
    // No session: the header says so when no account is known.
    signed_out: bool,
    library: Load<Library>,
    // By game id, for the games opened in this run.
    // `Pending` while a first download runs.
    trainers: HashMap<Arc<str>, Load<Loaded>>,
    // Games whose trainer is being read or downloaded right now.
    loading: HashSet<Arc<str>>,
    // A newer copy of the running game's trainer: installed when the run ends.
    deferred: Option<(Arc<str>, Loaded)>,
    selected: Option<Arc<str>>,
    play: Play,
    bindings: Bindings,
    profiles: Profiles,
    accent: u32,
    lead: Lead,
    panels: Panels,
    reduce_motion: bool,
    // The collection being named.
    editor: Option<Editor>,
    settings: bool,
    settings_tab: Tab,
    folders: Vec<FolderRow>,
    // The window is the overlay over a game: the HUD replaces the page.
    over: bool,
    layer: Entity<Layer>,
    docks: Entity<Docks>,
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
    pub fn new(
        provider: Arc<dyn Provider>,
        data_dir: PathBuf,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let prefs = Prefs::load(&data_dir);
        let accent = prefs.accent();
        let log = cx.new(|cx| LogView::new(accent, window, cx));
        cx.subscribe(&log, |page, _, event: &LogEvent, cx| {
            if matches!(event, LogEvent::Close) {
                page.log_open = false;
            }
            cx.notify();
        })
        .detach();
        let map = cx.new(|cx| MapView::new(provider.clone(), &data_dir, accent, window, cx));
        cx.subscribe_in(&map, window, |page, _, _: &MapEvent, window, cx| page.close_map(window, cx)).detach();
        let resources = cx.new(|cx| Resources::new(provider.clone(), data_dir.clone(), accent, cx));
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
        // A capture hides keys from every app: it must not outlive the user's attention.
        cx.observe_window_activation(window, |page, window, cx| {
            if !window.is_window_active() {
                page.cancel_capture(cx);
            }
        })
        .detach();
        let page = cx.weak_entity();
        let layer = cx.new(|cx| Layer::new(&data_dir, move |cx| page.upgrade().and_then(|page| page.read(cx).play.pid()), window, cx));
        cx.observe_in(&layer, window, Self::overlay_changed).detach();
        let docks = cx.new(|_| Docks::load(&data_dir, &hud::DOCKS));
        cx.observe_in(&docks, window, |page, _, window, cx| {
            page.sync_map_dock(window, cx);
            cx.notify();
        })
        .detach();
        // A run's time and mods are kept when it ends, and so does closing the window or quitting.
        cx.on_app_quit(|page, _| {
            page.close_session();
            async {}
        })
        .detach();
        cx.on_release(|page, _| page.close_session()).detach();
        let page = Self {
            bindings: Bindings::load(&data_dir),
            profiles: Profiles::load(&data_dir),
            data_dir,
            provider,
            account: None,
            signed_out: false,
            folders: Vec::new(),
            library: Load::Pending,
            trainers: HashMap::new(),
            loading: HashSet::new(),
            deferred: None,
            // Opens once the library lists it.
            selected: prefs.game.as_deref().map(Arc::from),
            play: Play::Idle,
            accent,
            lead: prefs.lead,
            panels: prefs.panels,
            reduce_motion: prefs.reduce_motion,
            editor: None,
            settings: false,
            settings_tab: Tab::Appearance,
            over: false,
            layer,
            docks,
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
        page.boot(window, cx);
        page.scan_folders(window, cx);
        page
    }

    fn games(&self) -> &[Game] {
        match &self.library {
            Load::Ready(library) => &library.games,
            _ => &[],
        }
    }

    fn game(&self) -> Option<&Game> {
        let id = self.selected.as_ref()?;
        self.games().iter().find(|game| game.id == *id)
    }

    fn trainer(&self, id: Option<&Arc<str>>) -> Option<&Loaded> {
        match self.trainers.get(id?)? {
            Load::Ready(loaded) => Some(loaded),
            _ => None,
        }
    }

    /// The open game's trainer.
    fn loaded_mut(&mut self) -> Option<&mut Loaded> {
        match self.trainers.get_mut(self.selected.as_ref()?)? {
            Load::Ready(loaded) => Some(loaded),
            _ => None,
        }
    }

    fn is_running(&self) -> bool {
        self.play.live().is_some_and(|game| Some(game) == self.selected.as_ref())
    }

    fn select(&mut self, id: Arc<str>, window: &mut Window, cx: &mut Context<Self>) {
        self.cancel_capture(cx);
        self.map.update(cx, |map, cx| map.hide(window, cx));
        self.selected = Some(id);
        self.save_prefs();
        // Another trainer's names: the old filter would hide them.
        self.mod_filter.update(cx, |filter, cx| filter.set_value("", window, cx));
        self.drop_cheat_state();
        self.open_trainer(window, cx);
        // Over a game, its dock opens the new game's map.
        self.sync_map_dock(window, cx);
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
                        body.child(
                            div()
                                .occlude()
                                .absolute()
                                .top_0()
                                .left_0()
                                .size_full()
                                .child(self.log.clone()),
                        )
                    }),
            )
            .child(self.status_bar(cx))
            .when(self.settings, |page| page.child(self.settings_modal(cx)))
            .children(self.collection_editor(cx))
            .into_any_element()
    }
}
