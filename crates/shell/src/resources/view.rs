//! The RESOURCES tab: what takes how much, each cache's CLEAR, the media limit, the user's own data.

use gpui_kit::{
    App, Context, Entity, FontWeight, IntoElement, Render, SharedString, Window,
    component::{h_flex, v_flex},
    div,
    prelude::*,
    px, relative, rgb,
};
use i18n::{Arg, Key};
use model::{Clear, Ref};

use super::Resources;
use crate::{
    settings::{label, row},
    ui::{
        key,
        text::{digits, t, text, tf},
        theme::*,
        widgets::{button, choice, mono, segmented, square, tracked},
    },
};

// One per cache, in the bar and beside its row.
const COLORS: [u32; 8] = [0x5B8DEF, 0x10BDDB, 0xB28DFF, 0x7BD88F, 0xF5C542, 0xFF9F43, 0xFF6B6B, 0x8A8F98];
const LIMITS: [(&str, Key, Option<u64>); 5] = [
    ("limit-500m", key::RESOURCES_LIMIT_500M, Some(500 << 20)),
    ("limit-1g", key::RESOURCES_LIMIT_1G, Some(1 << 30)),
    ("limit-2g", key::RESOURCES_LIMIT_2G, Some(2 << 30)),
    ("limit-5g", key::RESOURCES_LIMIT_5G, Some(5 << 30)),
    ("limit-none", key::RESOURCES_LIMIT_NONE, None),
];
const UNITS: [Key; 4] = [key::RESOURCES_BYTES, key::RESOURCES_KILOBYTES, key::RESOURCES_MEGABYTES, key::RESOURCES_GIGABYTES];

pub struct ResourcesView {
    resources: Entity<Resources>,
}

impl ResourcesView {
    pub fn new(resources: Entity<Resources>, cx: &mut Context<Self>) -> Self {
        cx.observe(&resources, |_, _, cx| cx.notify()).detach();
        Self { resources }
    }
}

impl Render for ResourcesView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let body = v_flex().px(px(18.)).py(px(10.));
        let resources = self.resources.read(cx);
        let Some(usage) = &resources.usage else {
            return body.child(mono(t(cx, key::RESOURCES_READING), 11.).text_color(rgb(FAINT))).into_any_element();
        };
        let color = |index: usize| COLORS[index % COLORS.len()];
        let total: u64 = usage.caches.iter().map(|store| store.bytes).sum::<u64>() + usage.progress + usage.hotkeys + usage.logs;
        let share = |bytes: u64| if total == 0 { 0. } else { bytes as f32 / total as f32 };
        let bar = h_flex().h(px(8.)).w_full().bg(line(0.08)).children(
            usage.caches.iter().enumerate().map(|(index, store)| div().h_full().w(relative(share(store.bytes))).bg(rgb(color(index)))),
        );
        let summary = v_flex()
            .gap(px(10.))
            .pb(px(14.))
            .border_b_1()
            .border_color(line(0.07))
            .child(
                row(t(cx, key::RESOURCES_IN_USE), t(cx, key::RESOURCES_IN_USE_HINT), 52.)
                    .border_b_0()
                    .child(mono(sized(cx, total), 18.).font_weight(FontWeight::BOLD).text_color(rgb(TEXT_BRIGHT))),
            )
            .child(bar);
        let caches = usage.caches.iter().enumerate().map(|(index, store)| {
            let id = store.id.clone();
            let action = match store.clear {
                _ if store.bytes == 0 => None,
                Clear::Now => Some(
                    button(&t(cx, key::RESOURCES_CLEAR))
                        .id(("clear", index))
                        .on_click(cx.listener(move |view, _, _, cx| view.clear(vec![id.clone()], cx)))
                        .into_any_element(),
                ),
                Clear::NextStart { asked: false } => Some(
                    button(&t(cx, key::RESOURCES_ON_RESTART))
                        .id(("clear", index))
                        .on_click(cx.listener(move |view, _, _, cx| view.clear(vec![id.clone()], cx)))
                        .into_any_element(),
                ),
                Clear::NextStart { asked: true } => {
                    Some(mono(t(cx, key::RESOURCES_CLEARS_ON_RESTART), 10.5).text_color(rgb(accent(cx))).into_any_element())
                }
            };
            entry(Some(color(index)), text(cx, &store.name), text(cx, &store.hint), sized(cx, store.bytes)).children(action)
        });
        let clearable: Vec<Ref> = usage
            .caches
            .iter()
            .filter(|store| store.bytes > 0 && store.clear != Clear::NextStart { asked: true })
            .map(|store| store.id.clone())
            .collect();
        let clear_all = row(t(cx, key::RESOURCES_CLEAR_CACHES), t(cx, key::RESOURCES_CLEAR_CACHES_HINT), 64.).child(
            button(&t(cx, key::RESOURCES_CLEAR_ALL))
                .id("clear-all")
                .on_click(cx.listener(move |view, _, _, cx| view.clear(clearable.clone(), cx))),
        );
        let limits = segmented().w_full().h(px(28.)).children(LIMITS.map(|(id, name, limit)| {
            choice(id, t(cx, name), resources.limit() == limit, cx)
                .flex_1()
                .on_click(cx.listener(move |view, _, _, cx| view.set_limit(limit, cx)))
        }));
        let limit = v_flex()
            .gap(px(10.))
            .py(px(14.))
            .border_b_1()
            .border_color(line(0.07))
            .child(label(t(cx, key::RESOURCES_LIMIT), t(cx, key::RESOURCES_LIMIT_HINT)))
            .child(limits);
        let kept = [
            (key::RESOURCES_PROGRESS, key::RESOURCES_PROGRESS_HINT, usage.progress),
            (key::RESOURCES_HOTKEYS, key::RESOURCES_HOTKEYS_HINT, usage.hotkeys),
            (key::RESOURCES_LOGS, key::RESOURCES_LOGS_HINT, usage.logs),
        ];
        body.child(summary)
            .children(caches)
            .child(clear_all)
            .child(limit)
            .child(tracked(&t(cx, key::RESOURCES_KEPT), 11., 0.14).pt(px(18.)).pb(px(4.)).text_color(rgb(DIM)))
            .children(kept.map(|(name, hint, bytes)| entry(None, t(cx, name), t(cx, hint), sized(cx, bytes))))
            .into_any_element()
    }
}

impl ResourcesView {
    fn clear(&mut self, ids: Vec<Ref>, cx: &mut Context<Self>) {
        self.resources.update(cx, |resources, cx| resources.clear(ids, cx));
    }

    fn set_limit(&mut self, limit: Option<u64>, cx: &mut Context<Self>) {
        self.resources.update(cx, |resources, cx| resources.set_limit(limit, cx));
    }
}

/// A store's row: its color, name, what it holds and its size; the caller adds the action.
fn entry(color: Option<u32>, name: SharedString, hint: SharedString, size: SharedString) -> gpui_kit::Div {
    h_flex()
        .min_h(px(52.))
        .gap(px(12.))
        .border_b_1()
        .border_color(line(0.07))
        .children(color.map(|color| square(10., rgb(color))))
        .child(label(name, hint).flex_1().min_w_0())
        .child(mono(size, 12.).text_color(rgb(TEXT)))
}

/// A size worded: the number with the language's marks, the unit from the catalog.
pub fn sized(cx: &App, bytes: u64) -> SharedString {
    let (number, unit) = size(bytes);
    tf(cx, UNITS[unit], &[("size", Arg::Str(&digits(cx, &number)))])
}

fn size(bytes: u64) -> (String, usize) {
    let (mut value, mut unit) = (bytes as f64, 0);
    while value >= 1024. && unit < UNITS.len() - 1 {
        value /= 1024.;
        unit += 1;
    }
    let number = if unit == 0 || value >= 10. { format!("{value:.0}") } else { format!("{value:.1}") };
    (number, unit)
}

#[cfg(test)]
mod tests {
    #[test]
    fn sizes_read_like_a_file_manager() {
        let sizes = [0, 900, 1536, 5 << 20, 121 << 20, 3 << 30].map(super::size);
        assert_eq!(
            sizes.map(|(number, unit)| (number, unit)),
            [("0", 0), ("900", 0), ("1.5", 1), ("5.0", 2), ("121", 2), ("3.0", 3)].map(|(n, u)| (n.to_owned(), u))
        );
    }
}
