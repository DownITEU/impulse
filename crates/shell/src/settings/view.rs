use gpui_kit::{
    App, Context, Div, FontWeight, IntoElement, Window,
    component::{h_flex, scroll::ScrollableElement, v_flex},
    div,
    prelude::*,
    px, rgb,
};

use super::{Lead, Panels, Prefs};
use crate::{
    Page,
    theme::*,
    widgets::{choice, mono, segmented, square, toggle, tracked},
};

#[derive(Clone, Copy, PartialEq)]
pub enum Tab {
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

impl Page {
    pub(crate) fn settings_modal(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let body = h_flex().items_stretch().min_h(px(300.)).child(self.tabs(cx)).child(
            div().id("settings-page").flex_1().min_w_0().overflow_y_scrollbar().child(match self.settings_tab {
                Tab::Appearance => self.appearance(cx).into_any_element(),
                Tab::Overlay => self.overlay_settings(cx).into_any_element(),
                Tab::Hotkeys => self.hotkey_settings(cx).into_any_element(),
                Tab::App => self.app(cx).into_any_element(),
                Tab::Resources => self.resources.clone().into_any_element(),
            }),
        );
        self.dialog("settings", "SETTINGS", 640., Self::close_settings, body, cx)
    }

    fn close_settings(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        self.settings_open = false;
        cx.notify();
    }

    fn edit_prefs(&self, change: impl FnOnce(&mut Prefs), cx: &mut App) {
        self.settings.update(cx, |settings, cx| settings.edit(change, cx));
    }

    fn tabs(&self, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex().w(px(168.)).flex_none().py(px(8.)).border_r_1().border_color(line(0.07)).bg(rgb(SUNKEN)).children(TABS.map(
            |(name, tab)| {
                let current = tab == Some(self.settings_tab);
                h_flex()
                    .id(name)
                    .h(px(34.))
                    .px(px(14.))
                    .gap(px(10.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(rgb(if current { TEXT_BRIGHT } else { DIM }))
                    .when(current, |tab| tab.bg(rgb(ROW_SELECTED)))
                    .when_some(tab, |item, tab| {
                        item.cursor_pointer().on_click(cx.listener(move |page, _, _, cx| {
                            page.settings_tab = tab;
                            page.resources.update(cx, |resources, cx| resources.measure(cx));
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
                        .on_click(cx.listener(move |page, _, _, cx| page.edit_prefs(|prefs| prefs.quit_on_close = close_to_tray, cx)))
                        .child(toggle(close_to_tray, cx)),
                ),
            )
            .child(
                row("HIDE FROM CAPTURE", "screenshots, recordings and streams show no Impulse window", 64.).border_b_0().child(
                    div()
                        .id("hide-from-capture")
                        .cursor_pointer()
                        .on_click(cx.listener(move |page, _, _, cx| page.edit_prefs(|prefs| prefs.hide_from_capture = !hidden, cx)))
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
                .on_click(cx.listener(move |page, _, _, cx| page.edit_prefs(|prefs| prefs.set_accent(color), cx)))
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
                .on_click(cx.listener(move |page, _, _, cx| page.edit_prefs(|prefs| prefs.lead = choose, cx)))
        };
        let panels = |id: &'static str, label: &'static str, choose: Panels| {
            choice(id, label, panels == choose, cx)
                .on_click(cx.listener(move |page, _, _, cx| page.edit_prefs(|prefs| prefs.panels = choose, cx)))
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
                        .on_click(cx.listener(move |page, _, _, cx| page.edit_prefs(|prefs| prefs.reduce_motion = !reduce_motion, cx)))
                        .child(toggle(reduce_motion, cx)),
                ),
            )
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
