//! A titled box over the dimmed page: SETTINGS, REMOTE PANEL and the collection editor.

use gpui_kit::{
    App, Context, Div, ElementId, FocusHandle, FontWeight, IntoElement, KeyBinding, SharedString, Stateful, Window, actions,
    component::{FocusTrapElement, h_flex, v_flex},
    div,
    prelude::*,
    px, relative, rgb,
};

use super::{
    assets::Icon,
    theme::*,
    widgets::{icon_button, tracked},
};

actions!(dialog, [CloseDialog]);

const CONTEXT: &str = "Dialog";

pub fn bind_keys(cx: &mut App) {
    cx.bind_keys([KeyBinding::new("escape", CloseDialog, Some(CONTEXT))]);
}

type Close<V> = fn(&mut V, &mut Window, &mut Context<V>);

/// `close` runs on a press beside the box, on its × and on Escape while `focus` is in it; Tab stays in the box.
pub fn dialog<V: 'static>(
    id: &'static str,
    title: impl Into<SharedString>,
    width: f32,
    focus: &FocusHandle,
    close: Close<V>,
    body: impl IntoElement,
    cx: &Context<V>,
) -> impl IntoElement {
    frame(id, title, width, focus, Some(close), body, cx)
}

/// A `dialog` that only its body's buttons end: no ×, no press beside the box, Escape does nothing.
pub fn pledge<V: 'static>(
    id: &'static str,
    title: impl Into<SharedString>,
    width: f32,
    focus: &FocusHandle,
    body: impl IntoElement,
    cx: &Context<V>,
) -> impl IntoElement {
    frame(id, title, width, focus, None, body, cx)
}

/// A dialog's body: `text` at reading size over `buttons` pressed to the right.
pub fn message(text: impl IntoElement, buttons: impl IntoIterator<Item = Stateful<Div>>) -> Div {
    v_flex()
        .gap(px(20.))
        .p(px(20.))
        .child(div().text_size(px(TYPE_LEAD)).line_height(relative(1.5)).text_color(rgb(TEXT)).child(text))
        .child(h_flex().justify_end().gap(px(10.)).children(buttons))
}

/// The dimmed page under a dialog or the map: what lies under it gets no hover or click.
pub fn backdrop(id: impl Into<ElementId>) -> Stateful<Div> {
    div().id(id).occlude().absolute().top_0().left_0().size_full().bg(alpha(SUNKEN, 0.78))
}

fn frame<V: 'static>(
    id: &'static str,
    title: impl Into<SharedString>,
    width: f32,
    focus: &FocusHandle,
    close: Option<Close<V>>,
    body: impl IntoElement,
    cx: &Context<V>,
) -> impl IntoElement {
    backdrop((id, 0usize))
        .flex()
        .justify_center()
        .items_start()
        .pt(px(96.))
        .pb(px(40.))
        .when_some(close, |backdrop, close| backdrop.on_click(cx.listener(move |view, _, window, cx| close(view, window, cx))))
        .child(
            v_flex()
                .id(id)
                .key_context(CONTEXT)
                .track_focus(focus)
                // Handled even without a `close`, so Escape stops here.
                .on_action(cx.listener(move |view, _: &CloseDialog, window, cx| {
                    if let Some(close) = close {
                        close(view, window, cx);
                    }
                }))
                .w(px(width))
                // A long body scrolls inside a short window.
                .max_h_full()
                .border_1()
                .border_color(line(0.14))
                .bg(rgb(PANEL))
                .on_click(|_, _, cx| cx.stop_propagation())
                .child(
                    h_flex()
                        .h(px(44.))
                        .flex_none()
                        .pl(px(16.))
                        .pr(px(6.))
                        .justify_between()
                        .border_b_1()
                        .border_color(line(0.09))
                        .child(tracked(&title.into(), TYPE_BODY, 0.16).font_weight(FontWeight::BOLD))
                        .when_some(close, |bar, close| {
                            bar.child(
                                icon_button((id, 1usize), Icon::Close, 32., 12.)
                                    .on_click(cx.listener(move |view, _, window, cx| close(view, window, cx))),
                            )
                        }),
                )
                .child(body)
                .focus_trap((id, 2usize), focus),
        )
}
