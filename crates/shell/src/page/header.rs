//! The custom title bar: gaps drag the window, controls occlude the drag area.

use gpui_kit::{
    App, Entity, FontWeight, IntoElement, SharedString, Window, WindowControlArea, component::h_flex, div, prelude::*, px, rgb,
};
use i18n::Arg;

use crate::{
    Load,
    account::Account,
    library::Library,
    settings::SettingsView,
    ui::{
        assets::Icon,
        key,
        text::{digits, t, tf},
        theme::*,
        widgets::{icon, icon_button, mono, square, tracked},
    },
};

pub fn header(library: &Library, account: &Account, settings: &Entity<SettingsView>, window: &Window, cx: &App) -> impl IntoElement {
    let accent = rgb(accent(cx));
    let maximize = if window.is_maximized() { Icon::WinRestore } else { Icon::WinMax };
    let search = match library.list() {
        Load::Ready(library) => {
            let count = library.trainers;
            let trainers = digits(cx, &format!("{},{:03}", count / 1000, count % 1000));
            tf(cx, key::PAGE_SEARCH, &[("trainers", Arg::Str(&trainers)), ("count", Arg::Num(count as f64))])
        }
        _ => t(cx, key::PAGE_SEARCH_NONE),
    };
    let name = match (account.user(), account.signed_out()) {
        (Some(user), _) => user.name.clone().into(),
        (None, true) => t(cx, key::PAGE_SIGNED_OUT),
        (None, false) => SharedString::default(),
    };
    let settings = settings.clone();
    h_flex()
        .h(px(52.))
        .flex_none()
        .pl(px(20.))
        .gap(px(28.))
        .border_b_1()
        .border_color(line(0.09))
        .bg(alpha(BG, 0.94))
        .window_control_area(WindowControlArea::Drag)
        .child(h_flex().gap(px(10.)).child(square(9., accent)).child(tracked("IMPULSE", 16., 0.22).font_weight(FontWeight::BOLD)))
        .child(
            h_flex()
                .id("search")
                .occlude()
                .w(px(380.))
                .min_w(px(220.))
                .h(px(34.))
                .pl(px(12.))
                .pr(px(8.))
                .gap(px(10.))
                .rounded(px(2.))
                .bg(rgb(SUNKEN))
                .border_1()
                .border_color(line(0.12))
                .cursor_pointer()
                .text_color(rgb(DIM))
                .child(mono(">", 13.).font_weight(FontWeight::BOLD).text_color(accent))
                .child(mono(search, 13.).font_weight(FontWeight::NORMAL).flex_1())
                .child(
                    mono(t(cx, key::PAGE_SEARCH_KEY), 10.5)
                        .px(px(6.))
                        .py(px(2.))
                        .rounded(px(2.))
                        .border_1()
                        .border_color(line(0.16))
                        .text_color(rgb(MUTED)),
                ),
        )
        .child(div().flex_1())
        .child(
            h_flex()
                .h_full()
                .gap(px(14.))
                .child(
                    h_flex()
                        .id("settings")
                        .group("settings")
                        .occlude()
                        .h(px(34.))
                        .px(px(9.))
                        .gap(px(10.))
                        .cursor_pointer()
                        .hover(|style| style.bg(rgb(RAISED_HOVER)))
                        .on_click(move |_, window, cx| settings.update(cx, |view, cx| view.open(window, cx)))
                        .child(icon(Icon::Settings, 16., rgb(DIM)).group_hover("settings", |style| style.text_color(rgb(TEXT))))
                        .when(!name.is_empty(), |button| {
                            button.child(div().text_size(px(13.)).font_weight(FontWeight::SEMIBOLD).child(name))
                        }),
                )
                .child(
                    h_flex()
                        .h_full()
                        .child(caption("minimize", Icon::WinMin, WindowControlArea::Min))
                        .child(caption("maximize", maximize, WindowControlArea::Max))
                        .child(caption("close", Icon::Close, WindowControlArea::Close)),
                ),
        )
}

fn caption(id: &'static str, glyph: Icon, area: WindowControlArea) -> impl IntoElement {
    icon_button(id, glyph, 12., 12.).w(px(44.)).h_full().occlude().window_control_area(area)
}
