//! The custom title bar: gaps drag the window, controls occlude the drag area.

use crate::{
    account::Account,
    library::{GamePicker, Library},
    remote::RemoteView,
    settings::SettingsView,
    ui::{
        assets::Icon,
        key,
        text::{digits, t, tf},
        theme::*,
        widgets::{icon, icon_button, mono, square, tip, tracked},
    },
};
use client::Load;
use gpui_kit::{
    App, Div, Entity, FontWeight, IntoElement, SharedString, Stateful, Window, WindowControlArea, component::h_flex, div, prelude::*, px,
    rgb,
};
use i18n::Arg;

pub fn header(
    search: impl IntoElement,
    account: &Account,
    settings: &Entity<SettingsView>,
    remote: Option<&Entity<RemoteView>>,
    elevated: bool,
    window: &Window,
    cx: &App,
) -> impl IntoElement {
    let accent = rgb(accent(cx));
    let maximize = if window.is_maximized() { Icon::WinRestore } else { Icon::WinMax };
    // A kept name is no session.
    let name = match (account.signed_out(), account.user()) {
        (true, _) => t(cx, key::PAGE_SIGNED_OUT),
        (false, Some(user)) => user.name.clone().into(),
        (false, None) => SharedString::default(),
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
        .child(h_flex().gap(px(10.)).child(square(9., accent)).child(tracked("IMPULSE", TYPE_TITLE, 0.22).font_weight(FontWeight::BOLD)))
        .child(search)
        .child(div().flex_1())
        .child(
            h_flex()
                .h_full()
                .gap(px(14.))
                .when(elevated, |row| row.child(elevated_mark(cx)))
                .children(remote.map(|remote| remote_button(remote, cx)))
                .child(
                    control("settings", Icon::Settings)
                        .gap(px(10.))
                        .on_click(move |_, window, cx| settings.update(cx, |view, cx| view.open(window, cx)))
                        .when(!name.is_empty(), |button| {
                            button.child(div().text_size(px(TYPE_BODY)).font_weight(FontWeight::SEMIBOLD).child(name))
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

/// `> SEARCH 3,654 TRAINERS  CTRL K`: opens the picker's search.
pub fn search_box(library: &Library, picker: &Entity<GamePicker>, cx: &App) -> impl IntoElement {
    let search = match library.list() {
        Load::Ready(library) => {
            let count = library.trainers;
            let trainers = digits(cx, &format!("{},{:03}", count / 1000, count % 1000));
            tf(cx, key::PAGE_SEARCH, &[("trainers", Arg::Str(&trainers)), ("count", Arg::Num(count as f64))])
        }
        _ => t(cx, key::PAGE_SEARCH_NONE),
    };
    let picker = picker.clone();
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
        .on_click(move |_, window, cx| picker.update(cx, |picker, cx| picker.find(window, cx)))
        .child(mono(">", TYPE_BODY).font_weight(FontWeight::BOLD).text_color(rgb(accent(cx))))
        .child(mono(search, TYPE_BODY).font_weight(FontWeight::NORMAL).flex_1())
        .child(
            mono(t(cx, key::PAGE_SEARCH_KEY), TYPE_CAPTION)
                .px(px(6.))
                .py(px(2.))
                .rounded(px(2.))
                .border_1()
                .border_color(line(0.16))
                .text_color(rgb(MUTED)),
        )
}

/// A shield while Impulse runs as administrator.
fn elevated_mark(cx: &App) -> impl IntoElement {
    let text = t(cx, key::PAGE_ELEVATED);
    h_flex().id("elevated").occlude().h(px(34.)).px(px(9.)).tooltip(move |window, cx| tip(text.clone(), window, cx)).child(icon(
        Icon::Shield,
        16.,
        rgb(WARN),
    ))
}

/// The phone opens the REMOTE PANEL dialog; the mark shows the server is on.
fn remote_button(remote: &Entity<RemoteView>, cx: &App) -> impl IntoElement {
    let (remote, text) = (remote.clone(), t(cx, key::REMOTE_OPEN));
    control("remote", Icon::Phone)
        .gap(px(7.))
        .tooltip(move |window, cx| tip(text.clone(), window, cx))
        .when(remote.read(cx).serving(cx), |button| button.child(square(6., rgb(accent(cx)))))
        .on_click(move |_, window, cx| remote.update(cx, |view, cx| view.open(window, cx)))
}

/// A title bar button: its glyph lights on hover; the caller adds the rest.
fn control(id: &'static str, glyph: Icon) -> Stateful<Div> {
    h_flex()
        .id(id)
        .group(id)
        .occlude()
        .h(px(34.))
        .px(px(9.))
        .cursor_pointer()
        .hover(|style| style.bg(rgb(RAISED_HOVER)))
        .child(icon(glyph, 16., rgb(DIM)).group_hover(id, |style| style.text_color(rgb(TEXT))))
}

fn caption(id: &'static str, glyph: Icon, area: WindowControlArea) -> impl IntoElement {
    icon_button(id, glyph, 12., 12.).w(px(44.)).h_full().occlude().window_control_area(area)
}
