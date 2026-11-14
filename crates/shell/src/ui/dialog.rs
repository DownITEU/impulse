//! A titled box over the dimmed page: Settings and the collection editor.

use gpui_kit::{
    App, Context, FocusHandle, FontWeight, IntoElement, KeyBinding, Window, actions,
    component::{h_flex, v_flex},
    div,
    prelude::*,
    px, rgb,
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

/// `close` runs on a press beside the box, on its × and on Escape while `focus` is in it.
pub fn dialog<V: 'static>(
    id: &'static str,
    title: &str,
    width: f32,
    focus: &FocusHandle,
    close: fn(&mut V, &mut Window, &mut Context<V>),
    body: impl IntoElement,
    cx: &Context<V>,
) -> impl IntoElement {
    div()
        .id((id, 0usize))
        .occlude()
        .absolute()
        .top_0()
        .left_0()
        .size_full()
        .flex()
        .justify_center()
        .items_start()
        .pt(px(96.))
        .pb(px(40.))
        .bg(alpha(SUNKEN, 0.78))
        .on_click(cx.listener(move |view, _, window, cx| close(view, window, cx)))
        .child(
            v_flex()
                .id(id)
                .key_context(CONTEXT)
                .track_focus(focus)
                .on_action(cx.listener(move |view, _: &CloseDialog, window, cx| close(view, window, cx)))
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
                        .child(tracked(title, 13., 0.16).font_weight(FontWeight::BOLD))
                        .child(
                            icon_button((id, 1usize), Icon::Close, 32., 12.)
                                .on_click(cx.listener(move |view, _, window, cx| close(view, window, cx))),
                        ),
                )
                .child(body),
        )
}
