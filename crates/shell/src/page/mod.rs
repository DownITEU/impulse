//! The combined page: the title bar, the installed games, the open game's pane and the status
//! bar. It makes the slices' entities and lays out their views; over a game the overlay's `Hud` takes its place.

mod disclaimer;
mod header;
mod hero;
mod signed_out;
mod status;
mod update_notice;

use std::sync::Arc;

use client::{
    Registry,
    library::{Game, GameId},
    storage::Paths,
};
use gpui_kit::{
    AnyElement, App, Context, Div, Entity, IntoElement, Render, Window,
    component::{h_flex, scroll::ScrollableElement, v_flex},
    div,
    prelude::*,
    px, rgb,
};

use crate::{
    about::AboutView,
    account::Account,
    bindings::{HotkeySettings, Hotkeys, hotkeys_scope},
    cheats::CheatList,
    crash,
    layer::{Entered, Hud, Layer, OverlaySettings},
    library::{GamePicker, Library, Marks, SIDEBAR_WIDTH, Sidebar, vacant},
    log_view::{LogEvent, LogView},
    map::{MapEvent, MapView, Maps, map_panel, paneled},
    plugins::{Plugins, PluginsView},
    remote::{Remote, RemoteView, Serve},
    resources::{Resources, ResourcesEvent, ResourcesView},
    run::{Runs, session_panel},
    settings::{Folders, Looked, Panels, Settings, SettingsView, Tabs},
    sounds::{Cues, SoundsView},
    sources::Sources,
    systray::TrayIcon,
    trainer::Trainers,
    ui::{theme::*, widgets::grid},
    update::{UpdatePanel, Updates},
};
use disclaimer::Disclaimer;
use header::header;
use hero::Hero;
use signed_out::SignedOut;
use status::status_bar;
use update_notice::{Opened, UpdateNotice};

// Below this pane width the side panels drop under the cheat list.
const WIDE_PANE: f32 = 900.;
const SIDE_WIDTH: f32 = 280.;
const SESSION_WIDTH: f32 = 300.;

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
    /// None in a build without the remote panel's server.
    remote_view: Option<Entity<RemoteView>>,
    /// Impulse runs as administrator.
    elevated: bool,
    _tray: Entity<TrayIcon>,
    layer: Entity<Layer>,
    // Drawn in place of the page while over a game.
    hud: Entity<Hud>,
    log: Entity<LogView>,
    maps: Entity<Maps>,
    map: Entity<MapView>,
    resources: Entity<Resources>,
    cheat_list: Entity<CheatList>,
    hero: Entity<Hero>,
    disclaimer: Entity<Disclaimer>,
    signed_out: Entity<SignedOut>,
    updates: Entity<Updates>,
    update_notice: Entity<UpdateNotice>,
    _cues: Entity<Cues>,
    _sources: Entity<Sources>,
}

impl Page {
    pub fn new(
        registry: Arc<Registry>,
        plugins: Arc<client::plugins::Plugins>,
        paths: Paths,
        serve: Option<Serve>,
        elevated: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let sources = cx.new(|cx| Sources::new(&registry, cx));
        let settings = cx.new(|cx| Settings::new(&paths, registry.clone(), &sources, cx));
        cx.observe(&settings, |_, _, cx| cx.notify()).detach();
        let disclaimer = cx.new(|cx| Disclaimer::new(settings.clone(), registry.clone(), window, cx));
        cx.observe(&disclaimer, |_, _, cx| cx.notify()).detach();
        let folders = cx.new(|cx| Folders::new(registry.clone(), cx));
        let library = cx.new(|cx| Library::new(registry.clone(), &paths, settings.clone(), cx));
        cx.observe(&library, |_, _, cx| cx.notify()).detach();
        let account = cx.new(|cx| Account::new(registry.clone(), &library, cx));
        cx.observe(&account, |_, _, cx| cx.notify()).detach();
        let signed_out = cx.new(|cx| SignedOut::new(account.clone(), disclaimer.clone(), registry.clone(), window, cx));
        cx.observe(&signed_out, |_, _, cx| cx.notify()).detach();
        let trainers = cx.new(|cx| Trainers::new(registry.clone(), &paths, library.clone(), &account, cx));
        cx.observe(&trainers, |_, _, cx| cx.notify()).detach();
        let runs = cx.new(|cx| Runs::new(library.clone(), trainers.clone(), cx));
        cx.observe(&runs, |_, _, cx| cx.notify()).detach();
        crash::watch(&runs, cx);
        let log = cx.new(|cx| LogView::new(window, cx));
        cx.subscribe(&log, |_, _, _: &LogEvent, cx| cx.notify()).detach();
        let maps = cx.new(|cx| Maps::new(registry.clone(), &paths, library.clone(), runs.clone(), &sources, cx));
        // The map panel reads the open game's offers and the titles to link.
        cx.observe(&maps, |_, _, cx| cx.notify()).detach();
        let map = cx.new(|cx| MapView::new(maps.clone(), &library, &paths, window, cx));
        cx.subscribe_in(&map, window, |page, map, _: &MapEvent, window, cx| {
            // Over a game the map is a dock, and Escape's last step leaves the overlay.
            if page.layer.read(cx).is_over() {
                return page.layer.read(cx).leave();
            }
            map.update(cx, |map, cx| map.hide(window, cx));
        })
        .detach();
        let resources = cx.new(|cx| Resources::new(registry.clone(), &paths, &sources, cx));
        let plugins = cx.new(|cx| Plugins::new(plugins, cx));
        // Only a source the page reads from reloads it: a plugin's answer cache must not stop a live trainer.
        cx.subscribe(&resources, move |page, _, event: &ResourcesEvent, cx| match event {
            ResourcesEvent::Cleared(sources) if sources.iter().any(|source| registry.plays(source)) => page.reload(cx),
            ResourcesEvent::Cleared(_) => {}
        })
        .detach();
        let picker = cx.new(|cx| GamePicker::new(library.clone(), cx));
        cx.observe(&picker, |_, _, cx| cx.notify()).detach();
        let marks = {
            let (trainers, runs) = (trainers.clone(), runs.clone());
            move |game: &GameId, cx: &App| Marks {
                favorite: trainers.read(cx).game_profile(game).favorite,
                live: runs.read(cx).is_live(game),
            }
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
        let cues = cx.new(|cx| Cues::new(&runs, &trainers, &settings, cx));
        let updates = cx.new(|cx| Updates::new(settings.clone(), runs.clone(), &paths, cx));
        cx.observe(&updates, |_, _, cx| cx.notify()).detach();
        let update_notice = cx.new(|cx| UpdateNotice::new(updates.clone(), disclaimer.clone(), signed_out.clone(), window, cx));
        cx.observe(&update_notice, |_, _, cx| cx.notify()).detach();
        let update_panel = cx.new(|cx| UpdatePanel::new(updates.clone(), settings.clone(), cx));
        let tabs = Tabs {
            overlay: cx.new(|cx| OverlaySettings::new(layer.clone(), hotkeys.clone(), trainers.clone(), settings.clone(), cx)).into(),
            hotkeys: cx.new(|cx| HotkeySettings::new(hotkeys.clone(), cx)).into(),
            sounds: cx.new(|cx| SoundsView::new(settings.clone(), cues.clone(), cx)).into(),
            resources: cx.new(|cx| ResourcesView::new(resources.clone(), cx)).into(),
            plugins: cx.new(|cx| PluginsView::new(plugins.clone(), window, cx)).into(),
            about: cx.new(|_| AboutView::new(update_panel.into())).into(),
        };
        let settings_view = cx.new(|cx| SettingsView::new(settings.clone(), folders, tabs, window, cx));
        cx.observe(&settings_view, |_, _, cx| cx.notify()).detach();
        let remote_view = serve.map(|serve| {
            let owners = (library.clone(), trainers.clone(), runs.clone());
            let remote = cx.new(|cx| Remote::new(serve, settings.clone(), owners, &paths, cx));
            let view = cx.new(|cx| RemoteView::new(remote, settings.clone(), cx));
            cx.observe(&view, |_, _, cx| cx.notify()).detach();
            view
        });
        // A dialog over Settings would leave it mounted beneath.
        cx.subscribe(&update_notice, |page, _, _: &Opened, cx| page.settings_view.update(cx, |view, cx| view.close(cx))).detach();
        cx.subscribe(&settings_view, move |page, _, _: &Looked, cx| {
            page.resources.update(cx, |resources, cx| resources.measure(cx));
            plugins.update(cx, |plugins, cx| plugins.refresh(cx));
        })
        .detach();
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
            if let Some(remote) = &page.remote_view {
                remote.update(cx, |view, cx| view.close(cx));
            }
            page.log.update(cx, |log, cx| log.close(cx));
            cx.notify();
        })
        .detach();
        let tray = cx.new(|cx| TrayIcon::new(library.clone(), runs.clone(), layer.clone(), settings.clone(), window, cx));
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
            remote_view,
            elevated,
            _tray: tray,
            layer,
            hud,
            log,
            maps,
            map,
            resources,
            cheat_list,
            hero,
            disclaimer,
            signed_out,
            updates,
            update_notice,
            _cues: cues,
            _sources: sources,
        }
    }

    /// The map `Root` shows over the page.
    pub fn map(&self) -> &Entity<MapView> {
        &self.map
    }

    fn game(&self, cx: &App) -> Option<Arc<Game>> {
        self.library.read(cx).opened().cloned()
    }

    /// The open map shows over the page; over a game it is in a dock.
    pub fn map_modal(&self, cx: &App) -> bool {
        !self.layer.read(cx).is_over() && self.maps.read(cx).is_open(cx)
    }

    /// Fills the page anew after a source's caches were cleared. The session stays: it is not a cache, but it
    /// is asked for again.
    fn reload(&mut self, cx: &mut Context<Self>) {
        log::info!("caches cleared");
        self.library.update(cx, |library, cx| library.reload(cx));
        self.account.update(cx, |account, cx| account.reload(cx));
        // The run keeps what it leaves on first, from the trainers about to go.
        self.runs.update(cx, |runs, cx| runs.stop(cx));
        self.trainers.update(cx, |trainers, cx| trainers.clear(cx));
        cx.notify();
    }

    fn pane(&self, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        let Some(game) = self.game(cx) else {
            return div().flex_1().p(px(24.)).child(vacant(self.library.read(cx), cx)).into_any_element();
        };
        let cheats = self.cheat_list.clone().into_any_element();
        let mapped = paneled(&self.map, cx);
        let body = div().flex().gap(px(20.)).px(px(24.)).pt(px(20.)).pb(px(28.));
        let body = match self.settings.read(cx).prefs().panels {
            // Without a map the session takes the row alone, as a strip.
            Panels::Top if !mapped => body.flex_col().child(self.session(&game, true, cx)).child(cheats),
            Panels::Top => {
                let session = self.session(&game, false, cx).w(px(SESSION_WIDTH)).flex_none();
                let map = map_panel(&self.map, &game, cx).flex_1().min_w_0();
                body.flex_col().child(h_flex().items_stretch().gap(px(16.)).child(map).child(session)).child(cheats)
            }
            Panels::Side => {
                let wide = window.viewport_size().width >= px(SIDEBAR_WIDTH + WIDE_PANE);
                let side = match wide {
                    true => v_flex().flex_none(),
                    false => h_flex().items_start(),
                };
                let session = self.session(&game, false, cx).when(!wide, |panel| panel.flex_1());
                body.when(!wide, |body| body.flex_col())
                    .when(wide, |body| body.items_start())
                    .child(cheats)
                    .child(side.gap(px(16.)).child(map_panel(&self.map, &game, cx).w(px(SIDE_WIDTH)).flex_none()).child(session))
            }
        };
        v_flex().id("pane").flex_1().min_w_0().h_full().child(self.hero.clone()).child(body).overflow_y_scrollbar().into_any_element()
    }

    fn session(&self, game: &Game, strip: bool, cx: &App) -> Div {
        session_panel(self.runs.read(cx), self.trainers.read(cx), game, strip, hotkeys_scope(&self.hotkeys, cx), cx)
    }
}

impl Render for Page {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.layer.read(cx).is_over() {
            // Entering the overlay first must not skip the disclaimer.
            return if self.disclaimer.read(cx).is_open(cx) {
                div().relative().size_full().child(self.hud.clone()).child(self.disclaimer.clone()).into_any_element()
            } else {
                self.hud.clone().into_any_element()
            };
        }
        v_flex()
            .relative()
            .size_full()
            .bg(rgb(BG))
            .child(grid(32., line(0.025)))
            .child(header(self.account.read(cx), &self.settings_view, self.remote_view.as_ref(), self.elevated, window, cx))
            .child(
                h_flex()
                    .flex_1()
                    .min_h_0()
                    .items_start()
                    .child(self.sidebar.clone())
                    .child(self.pane(window, cx))
                    .when(self.log.read(cx).is_open(), |body| {
                        body.child(div().occlude().absolute().top_0().left_0().size_full().child(self.log.clone()))
                    }),
            )
            .child(status_bar(
                &self.log,
                &self.updates,
                self.runs.read(cx),
                self.library.read(cx),
                self.trainers.read(cx),
                self.hotkeys.read(cx),
                cx,
            ))
            .when(self.settings_view.read(cx).is_open(), |page| page.child(self.settings_view.clone()))
            .children(self.remote_view.clone().filter(|view| view.read(cx).is_open()))
            .children(self.cheat_list.read(cx).editor().cloned())
            .when(self.picker.read(cx).is_open(), |page| page.child(self.picker.clone()))
            .when(self.disclaimer.read(cx).is_open(cx), |page| page.child(self.disclaimer.clone()))
            .when(self.signed_out.read(cx).is_open(cx), |page| page.child(self.signed_out.clone()))
            .when(self.update_notice.read(cx).is_open(cx), |page| page.child(self.update_notice.clone()))
            .into_any_element()
    }
}
