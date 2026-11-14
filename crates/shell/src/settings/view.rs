//! The SETTINGS dialog: its tabs, APPEARANCE and APP here; the others are views of their own slices.

use gpui_kit::{
    AnyView, App, Context, Div, Entity, EventEmitter, FocusHandle, FontWeight, IntoElement, Render, Window,
    component::{h_flex, input::InputState, scroll::ScrollableElement, v_flex},
    div,
    prelude::*,
    px, rgb,
};

use super::{Folders, Lead, Panels, Prefs, Settings, Shown};
use crate::{
    dialog::dialog,
    theme::*,
    widgets::{choice, mono, segmented, square, toggle, tracked},
};

#[derive(Clone, Copy, PartialEq)]
enum Tab {
    Appearance,
    Overlay,
    Hotkeys,
    App,
    Resources,
}

// The tabs without a page yet are shown but not clickable.
const TABS: [(&str, Option<Tab>); 6] = [
    ("APPEARANCE", Some(Tab::Appearance)),
    ("OVERLAY", Some(Tab::Overlay)),
    ("HOTKEYS", Some(Tab::Hotkeys)),
    ("APP", Some(Tab::App)),
    ("RESOURCES", Some(Tab::Resources)),
    ("ABOUT", None),
];

/// The tabs other slices draw.
pub struct Tabs {
    pub overlay: AnyView,
    pub hotkeys: AnyView,
    pub resources: AnyView,
}

/// The dialog opened or showed another tab: what it shows may be read again.
pub struct Looked;

pub struct SettingsView {
    settings: Entity<Settings>,
    pub(super) folders: Entity<Folders>,
    // The folder rows' path fields, one per row of `folders`.
    pub(super) fields: Vec<Entity<InputState>>,
    // The folder row whose picks are open.
    pub(super) menu: Option<usize>,
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
        v_flex().w(px(168.)).flex_none().py(px(8.)).border_r_1().border_color(line(0.07)).bg(rgb(SUNKEN)).children(TABS.map(
            |(name, tab)| {
                let current = tab == Some(self.tab);
                h_flex()
                    .id(name)
                    .h(px(34.))
                    .px(px(14.))
                    .gap(px(10.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(rgb(if current { TEXT_BRIGHT } else { DIM }))
                    .when(current, |tab| tab.bg(rgb(ROW_SELECTED)))
                    .when_some(tab, |item, tab| {
                        item.cursor_pointer().on_click(cx.listener(move |view, _, _, cx| {
                            view.tab = tab;
                            cx.emit(Looked);
                            cx.notify();
                        }))
                    })
                    .child(div().w(px(6.)).when(current, |cell| cell.child(square(6., rgb(accent(cx))))))
                    .child(tracked(name, 12., 0.12))
            },
        ))
    }

    fn app(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let prefs = self.settings.read(cx).prefs();
        let (close_to_tray, hidden) = (!prefs.quit_on_close, prefs.hide_from_capture);
        v_flex()
            .flex_1()
            .min_w_0()
            .px(px(18.))
            .py(px(6.))
            .children(self.folder_rows(cx))
            .child(
                row("CLOSE TO TRAY", "the close button hides Impulse; quit from the tray icon", 64.).mt(px(12.)).border_t_1().child(
                    div()
                        .id("close-to-tray")
                        .cursor_pointer()
                        .on_click(cx.listener(move |view, _, _, cx| view.edit_prefs(|prefs| prefs.quit_on_close = close_to_tray, cx)))
                        .child(toggle(close_to_tray, cx)),
                ),
            )
            .child(
                row("HIDE FROM CAPTURE", "screenshots, recordings and streams show no Impulse window", 64.).border_b_0().child(
                    div()
                        .id("hide-from-capture")
                        .cursor_pointer()
                        .on_click(cx.listener(move |view, _, _, cx| view.edit_prefs(|prefs| prefs.hide_from_capture = !hidden, cx)))
                        .child(toggle(hidden, cx)),
                ),
            )
    }

    fn appearance(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let prefs = self.settings.read(cx).prefs();
        let (accent, lead, panels, reduce_motion) = (prefs.accent(), prefs.lead, prefs.panels, prefs.reduce_motion);
        let swatches = ACCENTS.map(|(name, color)| {
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
                .child(mono(name, 9.5).text_color(rgb(if current { TEXT } else { FAINT })))
        });
        let lead = |id: &'static str, label: &'static str, choose: Lead| {
            choice(id, label, lead == choose, cx)
                .on_click(cx.listener(move |view, _, _, cx| view.edit_prefs(|prefs| prefs.lead = choose, cx)))
        };
        let panels = |id: &'static str, label: &'static str, choose: Panels| {
            choice(id, label, panels == choose, cx)
                .on_click(cx.listener(move |view, _, _, cx| view.edit_prefs(|prefs| prefs.panels = choose, cx)))
        };
        v_flex()
            .flex_1()
            .px(px(18.))
            .py(px(6.))
            .child(row("ACCENT", "marks live state only", 72.).child(h_flex().gap(px(10.)).children(swatches)))
            .child(row("SIDEBAR", "leading item of a game row", 64.).child(
                segmented().h(px(26.)).child(lead("lead-number", "NUMBER", Lead::Number)).child(lead(
                    "lead-thumbnail",
                    "THUMBNAIL",
                    Lead::Thumbnail,
                )),
            ))
            .child(row("PANELS", "map and session over the mods, or beside them", 64.).child(
                segmented().h(px(26.)).child(panels("panels-top", "TOP", Panels::Top)).child(panels("panels-side", "SIDE", Panels::Side)),
            ))
            .child(
                row("REDUCE MOTION", "no blinking LIVE, no transitions", 64.).border_b_0().child(
                    div()
                        .id("reduce-motion")
                        .cursor_pointer()
                        .on_click(cx.listener(move |view, _, _, cx| view.edit_prefs(|prefs| prefs.reduce_motion = !reduce_motion, cx)))
                        .child(toggle(reduce_motion, cx)),
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
                Tab::App => self.app(cx).into_any_element(),
                Tab::Resources => self.tabs.resources.clone().into_any_element(),
            }),
        );
        dialog("settings", "SETTINGS", 640., &self.focus, |view: &mut Self, _, cx| view.close(cx), body, cx)
    }
}

/// A setting's name with what it does under it.
pub fn label(title: &'static str, hint: &'static str) -> Div {
    v_flex()
        .gap(px(4.))
        .child(tracked(title, 13., 0.1).font_weight(FontWeight::SEMIBOLD))
        .child(mono(hint, 11.).font_weight(FontWeight::NORMAL).text_color(rgb(FAINT)))
}

/// The control the caller adds keeps its width; a long hint wraps.
pub fn row(title: &'static str, hint: &'static str, height: f32) -> Div {
    h_flex().min_h(px(height)).gap(px(16.)).border_b_1().border_color(line(0.07)).child(label(title, hint).flex_1().min_w_0())
}
