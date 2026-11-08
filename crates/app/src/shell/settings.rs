use gpui_kit::{
    ClickEvent, Context, Div, FontWeight, IntoElement, Window,
    component::{h_flex, v_flex},
    div,
    prelude::*,
    px, rgb,
};

use super::{
    Flash, Lead, Page,
    assets::Icon,
    theme::*,
    widgets::{flash_button, icon_button, mono, segment, segmented, square, toggle, tracked},
};

#[derive(Clone, Copy, PartialEq)]
pub(super) enum Tab {
    Appearance,
    App,
}

// The tabs without a page yet are shown but not clickable.
const TABS: [(&str, Option<Tab>); 6] = [
    ("ACCOUNT", None),
    ("APPEARANCE", Some(Tab::Appearance)),
    ("OVERLAY", None),
    ("HOTKEYS", None),
    ("APP", Some(Tab::App)),
    ("ABOUT", None),
];

impl Page {
    pub(super) fn settings_modal(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("settings-backdrop")
            .occlude()
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .flex()
            .justify_center()
            .items_start()
            .pt(px(96.))
            .bg(alpha(SUNKEN, 0.78))
            .on_click(cx.listener(Self::close_settings))
            .child(
                v_flex()
                    .id("settings")
                    .w(px(640.))
                    .border_1()
                    .border_color(line(0.14))
                    .bg(rgb(PANEL))
                    .on_click(|_, _, cx| cx.stop_propagation())
                    .child(
                        h_flex()
                            .h(px(44.))
                            .pl(px(16.))
                            .pr(px(6.))
                            .justify_between()
                            .border_b_1()
                            .border_color(line(0.09))
                            .child(tracked("SETTINGS", 13., 0.16).font_weight(FontWeight::BOLD))
                            .child(
                                icon_button("settings-close", Icon::Close, 32., 12.)
                                    .on_click(cx.listener(Self::close_settings)),
                            ),
                    )
                    .child(
                        h_flex()
                            .items_start()
                            .min_h(px(300.))
                            .child(self.tabs(cx))
                            .child(match self.settings_tab {
                                Tab::Appearance => self.appearance(cx).into_any_element(),
                                Tab::App => self.app(cx).into_any_element(),
                            }),
                    ),
            )
    }

    fn close_settings(&mut self, _: &ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
        self.settings = false;
        cx.notify();
    }

    fn tabs(&self, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .w(px(168.))
            .flex_none()
            .min_h(px(300.))
            .py(px(8.))
            .border_r_1()
            .border_color(line(0.07))
            .bg(rgb(SUNKEN))
            .children(TABS.map(|(name, tab)| {
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
                        item.cursor_pointer()
                            .on_click(cx.listener(move |page, _, _, cx| {
                                page.settings_tab = tab;
                                cx.notify();
                            }))
                    })
                    .child(
                        div()
                            .w(px(6.))
                            .when(current, |cell| cell.child(square(6., rgb(self.accent)))),
                    )
                    .child(tracked(name, 12., 0.12))
            }))
    }

    fn app(&self, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .flex_1()
            .min_w_0()
            .px(px(18.))
            .py(px(6.))
            .children(self.folder_rows(cx))
    }

    fn appearance(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let swatches = ACCENTS.map(|(name, color)| {
            let current = color == self.accent;
            v_flex()
                .id(name)
                .items_center()
                .gap(px(5.))
                .cursor_pointer()
                .on_click(cx.listener(move |page, _, _, cx| {
                    page.accent = color;
                    page.log.update(cx, |log, cx| {
                        log.accent = color;
                        cx.notify();
                    });
                    cx.notify();
                }))
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
        let lead = |id: &'static str, label: &'static str, lead: Lead| {
            let current = self.lead == lead;
            div()
                .id(id)
                .h_full()
                .cursor_pointer()
                .on_click(cx.listener(move |page, _, _, cx| {
                    page.lead = lead;
                    cx.notify();
                }))
                .child(
                    segment(
                        label,
                        current.then_some(self.accent),
                        if current { BG } else { SEGMENT_IDLE },
                    )
                    .px(px(12.)),
                )
        };
        v_flex()
            .flex_1()
            .px(px(18.))
            .py(px(6.))
            .child(
                row("ACCENT", "marks live state only", 72.)
                    .child(h_flex().gap(px(10.)).children(swatches)),
            )
            .child(
                row("SIDEBAR", "leading item of a game row", 64.).child(
                    segmented()
                        .h(px(26.))
                        .child(lead("lead-number", "NUMBER", Lead::Number))
                        .child(lead("lead-thumbnail", "THUMBNAIL", Lead::Thumbnail)),
                ),
            )
            .child(
                row("REDUCE MOTION", "no blinking LIVE, no transitions", 64.).child(
                    div()
                        .id("reduce-motion")
                        .cursor_pointer()
                        .on_click(cx.listener(|page, _, _, cx| {
                            page.reduce_motion = !page.reduce_motion;
                            cx.notify();
                        }))
                        .child(toggle(self.reduce_motion, self.accent)),
                ),
            )
            .child(
                row(
                    "CACHES",
                    "download the catalog, trainers and art again",
                    64.,
                )
                .border_b_0()
                .child(
                    flash_button(
                        "INVALIDATE",
                        "DONE",
                        self.is_flashed(Flash::Caches),
                        self.accent,
                    )
                    .id("invalidate-caches")
                    .on_click(cx.listener(|page, _, window, cx| {
                        page.invalidate_caches(window, cx);
                        page.flash(Flash::Caches, cx);
                    })),
                ),
            )
    }
}

/// A setting's name with what it does under it.
pub(super) fn label(title: &'static str, hint: &'static str) -> Div {
    v_flex()
        .gap(px(4.))
        .child(tracked(title, 13., 0.1).font_weight(FontWeight::SEMIBOLD))
        .child(
            mono(hint, 11.)
                .font_weight(FontWeight::NORMAL)
                .text_color(rgb(FAINT)),
        )
}

fn row(title: &'static str, hint: &'static str, height: f32) -> Div {
    h_flex()
        .min_h(px(height))
        .justify_between()
        .border_b_1()
        .border_color(line(0.07))
        .child(label(title, hint))
}
