use gpui_kit::{
    ClickEvent, Context, FontWeight, IntoElement, Window, WindowControlArea, component::h_flex,
    div, prelude::*, px, rgb,
};

use super::{
    Page,
    assets::Icon,
    theme::*,
    widgets::{icon_button, mono, square, tracked},
};

impl Page {
    /// The custom title bar: gaps drag the window, controls occlude the drag area.
    pub(super) fn header(&self, window: &Window, cx: &mut Context<Self>) -> impl IntoElement {
        let accent = rgb(self.accent);
        let maximize = if window.is_maximized() {
            Icon::WinRestore
        } else {
            Icon::WinMax
        };
        h_flex()
            .h(px(52.))
            .flex_none()
            .pl(px(20.))
            .gap(px(28.))
            .border_b_1()
            .border_color(line(0.09))
            .bg(alpha(BG, 0.94))
            .window_control_area(WindowControlArea::Drag)
            .child(
                h_flex()
                    .gap(px(10.))
                    .child(square(9., accent))
                    .child(tracked("IMPULSE", 16., 0.22).font_weight(FontWeight::BOLD)),
            )
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
                    .child(
                        mono(">", 13.)
                            .font_weight(FontWeight::BOLD)
                            .text_color(accent),
                    )
                    .child(
                        mono("search 3,634 trainers", 13.)
                            .font_weight(FontWeight::NORMAL)
                            .flex_1(),
                    )
                    .child(
                        mono("CTRL K", 10.5)
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
                        icon_button("settings", Icon::Settings, 34., 16.)
                            .occlude()
                            .on_click(cx.listener(Self::open_settings)),
                    )
                    .child(
                        h_flex()
                            .id("account")
                            .occlude()
                            .h(px(34.))
                            .pl(px(4.))
                            .pr(px(10.))
                            .gap(px(10.))
                            .cursor_pointer()
                            .hover(|style| style.bg(rgb(RAISED_HOVER)))
                            .on_click(cx.listener(Self::open_settings))
                            .child(
                                div()
                                    .size(px(26.))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .rounded(px(2.))
                                    .bg(rgb(AVATAR))
                                    .border_1()
                                    .border_color(line(0.14))
                                    .text_size(px(12.))
                                    .font_weight(FontWeight::BOLD)
                                    .child("W"),
                            )
                            .child(
                                div()
                                    .text_size(px(13.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child("[username]"),
                            ),
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

    fn open_settings(&mut self, _: &ClickEvent, window: &mut Window, cx: &mut Context<Self>) {
        // A focused filter would keep taking keys behind the modal.
        window.blur(cx);
        self.settings = true;
        cx.notify();
    }
}

fn caption(id: &'static str, glyph: Icon, area: WindowControlArea) -> impl IntoElement {
    icon_button(id, glyph, 12., 12.)
        .w(px(44.))
        .h_full()
        .occlude()
        .window_control_area(area)
}
