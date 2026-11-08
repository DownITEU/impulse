//! The combined page (`docs/design.md`): the installed games, the open game's trainer and its controls.
//! LAUNCH + ATTACH runs the open game's trainer (`run.rs`); its cheat controls drive the live session.

mod art;
mod assets;
mod boot;
mod cheats;
mod controls;
mod header;
mod hero;
mod keys;
mod library;
mod loaded;
mod log_view;
mod run;
mod settings;
mod side;
mod sidebar;
mod theme;
mod wand_dir;
mod widgets;

use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    sync::Arc,
};

use anyhow::Result;
use gpui_kit::{
    AnyElement, App, Context, Entity, IntoElement, Render, Task, Window,
    component::{
        h_flex,
        input::{InputEvent, InputState},
        scroll::ScrollableElement,
        v_flex,
    },
    div,
    prelude::*,
    px, rgb,
};

pub use assets::Assets;
use library::{Game, Library};
use loaded::Loaded;
use log_view::{LogEvent, LogView};
use run::Play;
use settings::Tab;
use theme::*;
use wand_dir::WandDir;
use widgets::{grid, mono, notice, square};

use crate::{
    play::tophat::Server,
    preferences::{self, Preferences},
    session::{Account, Session},
    transport::Transport,
};

const SIDEBAR_WIDTH: f32 = 248.;
// Below this pane width the map and session panels drop under the cheat list.
const WIDE_PANE: f32 = 900.;

pub fn init(data_dir: &Path, cx: &mut App) {
    theme::init(cx);
    cx.set_http_client(Arc::new(art::CdnClient {
        cache: art::cache_dir(data_dir),
    }));
    log_view::bind_keys(cx);
}

/// Something that arrives after the window opens.
enum Load<T> {
    Pending,
    Ready(T),
    Failed(String),
}

#[derive(Clone, Copy, PartialEq)]
enum Lead {
    Number,
    Thumbnail,
}

/// The open dropdown.
#[derive(Clone, Copy, PartialEq)]
enum Menu {
    Overlay,
    Cheat(usize),
    WandVersions,
}

#[derive(Clone, Copy, PartialEq)]
enum Flash {
    Cheat(usize),
    Caches,
}

pub struct Page {
    data_dir: PathBuf,
    transport: Option<Transport>,
    session: Load<Arc<Session>>,
    account: Option<Account>,
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
    // Started with the first run.
    tophat: Option<Arc<Server>>,
    favorites: HashSet<Arc<str>>,
    accent: u32,
    lead: Lead,
    reduce_motion: bool,
    settings: bool,
    settings_tab: Tab,
    preferences: Preferences,
    wand: WandDir,
    // Index into `hero::OVERLAYS`.
    overlay: usize,
    menu: Option<Menu>,
    // The press-only button showing its confirmation.
    flashed: Option<(Flash, Task<()>)>,
    log: Entity<LogView>,
    log_open: bool,
    game_filter: Entity<InputState>,
    mod_filter: Entity<InputState>,
}

impl Page {
    pub fn new(
        transport: Result<Transport>,
        data_dir: PathBuf,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let log = cx.new(|cx| LogView::new(ACCENTS[0].1, window, cx));
        cx.subscribe(&log, |page, _, event: &LogEvent, cx| {
            if matches!(event, LogEvent::Close) {
                page.log_open = false;
            }
            cx.notify();
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
        let (transport, session) = match transport {
            Ok(transport) => (Some(transport), Load::Pending),
            Err(e) => {
                log::error!("Transport: {e:#}");
                (None, Load::Failed(format!("{e:#}")))
            }
        };
        let page = Self {
            // A few bytes from the local data directory: not worth a loading state.
            preferences: preferences::load(&data_dir),
            wand: WandDir::new(window, cx),
            data_dir,
            transport,
            session,
            account: None,
            library: Load::Pending,
            trainers: HashMap::new(),
            loading: HashSet::new(),
            deferred: None,
            selected: None,
            play: Play::Idle,
            tophat: None,
            favorites: HashSet::new(),
            accent: ACCENTS[0].1,
            lead: Lead::Number,
            reduce_motion: false,
            settings: false,
            settings_tab: Tab::Appearance,
            overlay: 0,
            menu: None,
            flashed: None,
            log,
            log_open: false,
            game_filter,
            mod_filter,
        };
        page.boot(window, cx);
        page.scan_wand(window, cx);
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
        self.selected = Some(id);
        self.drop_cheat_state();
        self.open_trainer(window, cx);
        cx.notify();
    }

    /// The open menu and flash are keyed by cheat index, which means another cheat in another trainer.
    fn drop_cheat_state(&mut self) {
        if matches!(self.menu, Some(Menu::Cheat(_))) {
            self.menu = None;
        }
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

    fn pane(&self, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        let Some(game) = self.game() else {
            return div()
                .flex_1()
                .p(px(24.))
                .child(self.vacant())
                .into_any_element();
        };
        let running = self.is_running();
        let wide = window.viewport_size().width >= px(SIDEBAR_WIDTH + WIDE_PANE);
        let side = match wide {
            true => v_flex().flex_none(),
            false => h_flex().items_start(),
        };
        v_flex()
            .id("pane")
            .flex_1()
            .min_w_0()
            .h_full()
            .child(self.hero(game, cx))
            .child(
                div()
                    .flex()
                    .when(!wide, |body| body.flex_col())
                    .when(wide, |body| body.items_start())
                    .gap(px(20.))
                    .px(px(24.))
                    .pt(px(20.))
                    .pb(px(28.))
                    .child(self.cheats(game, cx))
                    .child(
                        side.gap(px(16.))
                            .child(self.map_panel(game, running))
                            .child(
                                div()
                                    .when(!wide, |panel| panel.flex_1())
                                    .child(self.session_panel(game, running)),
                            ),
                    ),
            )
            .overflow_y_scrollbar()
            .into_any_element()
    }

    // The pane before there is a game to open.
    fn vacant(&self) -> impl IntoElement {
        match &self.library {
            Load::Pending => notice(
                "[ SCANNING ]",
                "LOOKING FOR INSTALLED GAMES",
                "Reading the game catalog and your launchers.",
            ),
            Load::Failed(why) => notice(
                "[ NO CATALOG ]",
                "THE GAME CATALOG IS UNAVAILABLE",
                &format!(
                    "It could not be downloaded: {why}. Impulse tries again on the next start."
                ),
            ),
            Load::Ready(_) => notice(
                "[ NO GAMES ]",
                "NO INSTALLED GAMES FOUND",
                "Impulse looks in Steam, Epic, GOG, Ubisoft, Xbox, Rockstar and EA for games from the Wand catalog.",
            ),
        }
    }

    fn status_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let (errors, warnings) = self.log.read(cx).problems();
        let problem = |count: usize, color: u32| {
            h_flex()
                .gap(px(6.))
                .child(square(7., rgb(color)))
                .child(mono(count.to_string(), 11.).text_color(rgb(color)))
        };
        let accent = rgb(self.accent);
        let running = self
            .games()
            .iter()
            .find(|game| Some(&game.id) == self.play.live());
        h_flex()
            .h(px(30.))
            .flex_none()
            .px(px(20.))
            .gap(px(22.))
            .border_t_1()
            .border_color(line(0.09))
            .bg(rgb(STATUSBAR))
            .text_color(rgb(DIM))
            .map(|bar| match running {
                Some(game) => {
                    let (active, total) = self
                        .trainer(Some(&game.id))
                        .map_or((0, 0), |loaded| (loaded.active(), loaded.cheats.len()));
                    bar.child(
                        h_flex()
                            .gap(px(8.))
                            .child(square(7., accent))
                            .child(mono("LIVE", 11.).text_color(accent)),
                    )
                    .child(mono(game.name.clone(), 11.))
                    .child(mono(format!("{active}/{total} ACTIVE"), 11.))
                }
                None => bar.child(mono("IDLE", 11.)),
            })
            .child(div().flex_1())
            .child(
                h_flex()
                    .id("log")
                    .h_full()
                    .gap(px(10.))
                    .cursor_pointer()
                    .hover(|style| style.text_color(rgb(TEXT)))
                    .when(self.log_open, |item| item.text_color(rgb(TEXT)))
                    .on_click(cx.listener(|page, _, window, cx| {
                        page.log_open = !page.log_open;
                        if page.log_open {
                            page.log.update(cx, |log, cx| log.focus(window, cx));
                        }
                        cx.notify();
                    }))
                    .child(mono("LOG", 11.))
                    .when(errors > 0, |item| item.child(problem(errors, DANGER)))
                    .when(warnings > 0, |item| item.child(problem(warnings, WARN))),
            )
            .child(mono("HOTKEYS ON", 11.))
            .child(mono("IMPULSE 0.1", 11.).text_color(rgb(GHOST)))
    }
}

impl Render for Page {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .relative()
            .size_full()
            .bg(rgb(BG))
            .text_color(rgb(TEXT))
            .font_family(UI)
            .text_size(px(14.))
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
    }
}
