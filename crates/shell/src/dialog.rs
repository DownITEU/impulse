//! A titled box over the dimmed page: Settings and the collection editor.

use gpui_kit::{
    App, Context, FontWeight, IntoElement, KeyBinding, Window, actions,
    component::{h_flex, v_flex},
    div,
    prelude::*,
    px, rgb,
};

use super::{
    Page,
    assets::Icon,
    theme::*,
    widgets::{icon_button, tracked},
};

actions!(dialog, [CloseDialog]);

const CONTEXT: &str = "Dialog";

pub fn bind_keys(cx: &mut App) {
    cx.bind_keys([KeyBinding::new("escape", CloseDialog, Some(CONTEXT))]);
}

type Close = fn(&mut Page, &mut Window, &mut Context<Page>);

impl Page {
    /// `close` runs on a press beside the box, on its × and on Escape while the focus is in it.
    pub(super) fn dialog(
        &self,
        id: &'static str,
        title: &str,
        width: f32,
        close: Close,
        body: impl IntoElement,
        cx: &mut Context<Self>,
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
            .on_click(cx.listener(move |page, _, window, cx| close(page, window, cx)))
            .child(
                v_flex()
                    .id(id)
                    .key_context(CONTEXT)
                    .track_focus(&self.dialog_focus)
                    .on_action(cx.listener(move |page, _: &CloseDialog, window, cx| close(page, window, cx)))
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
                                    .on_click(cx.listener(move |page, _, window, cx| close(page, window, cx))),
                            ),
                    )
                    .child(body),
            )
    }
}
