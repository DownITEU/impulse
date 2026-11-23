//! The SETTINGS dialog: its tabs, APPEARANCE and APP here; the others are views of their own slices.

use std::iter;

use gpui_kit::{
    AnyView, App, Context, Div, Entity, EventEmitter, FocusHandle, FontWeight, IntoElement, Render, SharedString, Window,
    component::{h_flex, input::InputState, scroll::ScrollableElement, v_flex},
    div,
    prelude::*,
    px, rgb,
};

use i18n::{Key, LANGUAGES, Language};

use super::{Folders, Lead, Panels, Prefs, Settings, Shown};
use crate::ui::{
    dialog::dialog,
    dropdown::{dropdown, trigger},
    key,
    text::t,
    theme::*,
    widgets::{badge, choice, list_row, mono, segmented, square, switch, tracked},
};

#[derive(Clone, Copy, PartialEq)]
enum Tab {
    Appearance,
    Overlay,
    Hotkeys,
    Sounds,
    App,
    Resources,
    Plugins,
    About,
}

const TABS: [(Key, Tab); 8] = [
    (key::SETTINGS_APPEARANCE, Tab::Appearance),
    (key::SETTINGS_OVERLAY, Tab::Overlay),
    (key::SETTINGS_HOTKEYS, Tab::Hotkeys),
    (key::SETTINGS_SOUNDS, Tab::Sounds),
    (key::SETTINGS_APP, Tab::App),
    (key::SETTINGS_RESOURCES, Tab::Resources),
    (key::SETTINGS_PLUGINS, Tab::Plugins),
    (key::SETTINGS_ABOUT, Tab::About),
];

/// The dialog's open list: a folder row's picks or the languages.
#[derive(Clone, Copy, PartialEq)]
pub(super) enum Menu {
    Folder(usize),
    Language,
}

/// The tabs other slices draw.
pub struct Tabs {
    pub overlay: AnyView,
    pub hotkeys: AnyView,
    pub sounds: AnyView,
    pub resources: AnyView,
    pub plugins: AnyView,
    pub about: AnyView,
}

/// The dialog opened or showed another tab: what it shows may be read again.
pub struct Looked;

pub struct SettingsView {
    settings: Entity<Settings>,
    pub(super) folders: Entity<Folders>,
    // The folder rows' path fields, one per row of `folders`.
    pub(super) fields: Vec<Entity<InputState>>,
    pub(super) menu: Option<Menu>,
    tabs: Tabs,
    tab: Tab,
    open: bool,
    focus: FocusHandle,
}

impl EventEmitter<Looked> for SettingsView {}

impl SettingsView {
    pub fn new(settings: Entity<Settings>, folders: Entity<Folders>, tabs: Tabs, window: &mut Window, cx: &mut Context<Self>) -> Self {
        cx.observe(&settings, |_, _, cx| cx.notify()).detach();
        cx.observe(&folders, |_, _, cx| cx.notify()).detach();
        cx.subscribe_in(&folders, window, |view, folders, &Shown(at), window, cx| view.folder_shown(folders, at, window, cx)).detach();
        Self { settings, folders, fields: Vec::new(), menu: None, tabs, tab: Tab::Appearance, open: false, focus: cx.focus_handle() }
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    pub fn open(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // A focused filter would keep taking keys behind the modal, and Escape closes it.
        self.focus.focus(window, cx);
        self.open = true;
        // An update may have installed a newer version since the last look.
        self.folders.update(cx, |folders, cx| folders.scan(cx));
        cx.emit(Looked);
        cx.notify();
    }

    pub fn close(&mut self, cx: &mut Context<Self>) {
        self.open = false;
        self.menu = None;
        cx.notify();
    }

    fn edit_prefs(&self, change: impl FnOnce(&mut Prefs), cx: &mut App) {
        self.settings.update(cx, |settings, cx| settings.edit(change, cx));
    }

    fn tabs(&self, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex().min_w(px(168.)).flex_none().py(px(8.)).border_r_1().border_color(line(0.07)).bg(rgb(SUNKEN)).children(
            TABS.into_iter().map(|(name, tab)| {
                let current = tab == self.tab;
                list_row(name.0, current)
                    .h(px(34.))
                    .px(px(14.))
                    .gap(px(10.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(rgb(if current { TEXT_BRIGHT } else { DIM }))
                    .on_click(cx.listener(move |view, _, _, cx| {
                        view.tab = tab;
                        cx.emit(Looked);
                        cx.notify();
                    }))
                    .child(div().w(px(6.)).when(current, |cell| cell.child(square(6., rgb(accent(cx))))))
                    .child(tracked(&t(cx, name), TYPE_SMALL, 0.12).flex_nowrap())
                    .when(tab == Tab::Plugins, |item| item.child(badge(t(cx, key::ALPHA), WARN)))
            }),
        )
    }

    fn app(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let folders = self.folder_rows(cx);
        let prefs = self.settings.read(cx).prefs();
        let (close_to_tray, hidden) = (!prefs.quit_on_close, prefs.hide_from_capture);
        let names: Vec<SharedString> =
            languages().map(|ours| ours.map_or_else(|| t(cx, key::LANGUAGE_SYSTEM), |ours| ours.name.into())).collect();
        let current = languages().position(|ours| ours == prefs.chosen()).unwrap_or_default();
        let width = px(160.);
        let shown = trigger("language", names[current].clone()).w(width);
        let open = self.menu == Some(Menu::Language);
        let language = dropdown(shown, Menu::Language, open_menu, open, names, current, width, cx, |view, at, _, cx| {
            let tag = languages().nth(at).flatten().map(|ours| ours.tag.to_owned());
            view.edit_prefs(|prefs| prefs.language = tag, cx);
        });
        v_flex()
            .flex_1()
            .min_w_0()
            .px(px(18.))
            .py(px(6.))
            .children(folders)
            .child(row(t(cx, key::LANGUAGE), t(cx, key::LANGUAGE_HINT), 64.).mt(px(12.)).border_t_1().child(language))
            .child(
                row(t(cx, key::CLOSE_TO_TRAY), t(cx, key::CLOSE_TO_TRAY_HINT), 64.).child(
                    switch("close-to-tray", close_to_tray, cx)
                        .on_click(cx.listener(move |view, _, _, cx| view.edit_prefs(|prefs| prefs.quit_on_close = close_to_tray, cx))),
                ),
            )
            .child(
                row(t(cx, key::HIDE_FROM_CAPTURE), t(cx, key::HIDE_FROM_CAPTURE_HINT), 64.).border_b_0().child(
                    switch("hide-from-capture", hidden, cx)
                        .on_click(cx.listener(move |view, _, _, cx| view.edit_prefs(|prefs| prefs.hide_from_capture = !hidden, cx))),
                ),
            )
    }

    fn appearance(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let prefs = self.settings.read(cx).prefs();
        let (accent, lead, panels, reduce_motion) = (prefs.accent(), prefs.lead, prefs.panels, prefs.reduce_motion);
        let swatches = ACCENTS.map(|(name, color, label)| {
            let current = color == accent;
            v_flex()
                .id(name)
                .items_center()
                .gap(px(5.))
                .cursor_pointer()
                .on_click(cx.listener(move |view, _, _, cx| view.edit_prefs(|prefs| prefs.set_accent(color), cx)))
                .child(
                    div()
                        .size(px(28.))
                        .p(px(3.))
                        .border_1()
                        .border_color(if current { rgb(color) } else { line(0.16) })
                        .child(div().size_full().bg(rgb(color))),
                )
                .child(mono(t(cx, label), TYPE_TAG).text_color(rgb(if current { TEXT } else { FAINT })))
        });
        let lead = |id: &'static str, label: Key, choose: Lead| {
            choice(id, t(cx, label), lead == choose, cx)
                .on_click(cx.listener(move |view, _, _, cx| view.edit_prefs(|prefs| prefs.lead = choose, cx)))
        };
        let panels = |id: &'static str, label: Key, choose: Panels| {
            choice(id, t(cx, label), panels == choose, cx)
                .on_click(cx.listener(move |view, _, _, cx| view.edit_prefs(|prefs| prefs.panels = choose, cx)))
        };
        v_flex()
            .flex_1()
            .px(px(18.))
            .py(px(6.))
            .child(row(t(cx, key::ACCENT), t(cx, key::ACCENT_HINT), 72.).child(h_flex().gap(px(10.)).children(swatches)))
            .child(row(t(cx, key::SIDEBAR), t(cx, key::SIDEBAR_HINT), 64.).child(
                segmented().h(px(26.)).child(lead("lead-number", key::SIDEBAR_NUMBER, Lead::Number)).child(lead(
                    "lead-thumbnail",
                    key::SIDEBAR_THUMBNAIL,
                    Lead::Thumbnail,
                )),
            ))
            .child(row(t(cx, key::PANELS), t(cx, key::PANELS_HINT), 64.).child(
                segmented().h(px(26.)).child(panels("panels-top", key::PANELS_TOP, Panels::Top)).child(panels(
                    "panels-side",
                    key::PANELS_SIDE,
                    Panels::Side,
                )),
            ))
            .child(
                row(t(cx, key::REDUCE_MOTION), t(cx, key::REDUCE_MOTION_HINT), 64.).border_b_0().child(
                    switch("reduce-motion", reduce_motion, cx)
                        .on_click(cx.listener(move |view, _, _, cx| view.edit_prefs(|prefs| prefs.reduce_motion = !reduce_motion, cx))),
                ),
            )
    }
}

impl Render for SettingsView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let body = h_flex().items_stretch().min_h(px(300.)).child(self.tabs(cx)).child(
            div().id("settings-page").flex_1().min_w_0().overflow_y_scrollbar().child(match self.tab {
                Tab::Appearance => self.appearance(cx).into_any_element(),
                Tab::Overlay => self.tabs.overlay.clone().into_any_element(),
                Tab::Hotkeys => self.tabs.hotkeys.clone().into_any_element(),
                Tab::Sounds => self.tabs.sounds.clone().into_any_element(),
                Tab::App => self.app(cx).into_any_element(),
                Tab::Resources => self.tabs.resources.clone().into_any_element(),
                Tab::Plugins => self.tabs.plugins.clone().into_any_element(),
                Tab::About => self.tabs.about.clone().into_any_element(),
            }),
        );
        dialog("settings", t(cx, key::SETTINGS), 640., &self.focus, |view: &mut Self, _, cx| view.close(cx), body, cx)
    }
}

/// The language menu: the system's (`None`), then each language we ship.
fn languages() -> impl Iterator<Item = Option<Language>> {
    iter::once(None).chain(LANGUAGES.iter().copied().map(Some))
}

pub(super) fn open_menu(view: &mut SettingsView) -> &mut Option<Menu> {
    &mut view.menu
}

pub fn label(title: impl Into<SharedString>, hint: impl Into<SharedString>) -> Div {
    v_flex()
        .gap(px(4.))
        .child(tracked(&title.into(), TYPE_BODY, 0.1).font_weight(FontWeight::SEMIBOLD))
        .child(mono(hint, TYPE_LABEL).font_weight(FontWeight::NORMAL).text_color(rgb(FAINT)))
}

/// The control the caller adds keeps its width; a long hint wraps.
pub fn row(title: impl Into<SharedString>, hint: impl Into<SharedString>, height: f32) -> Div {
    h_flex().min_h(px(height)).gap(px(16.)).border_b_1().border_color(line(0.07)).child(label(title, hint).flex_1().min_w_0())
}
