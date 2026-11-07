use gpui_kit::{
    Bounds, Div, Entity, FontWeight, Hsla, IntoElement, Rgba, SharedString, Stateful, Svg, canvas,
    component::{
        h_flex,
        input::{Input, InputState},
    },
    div, fill, point,
    prelude::*,
    px, rgb, size, svg,
};

use super::{assets::Icon, theme::*};

pub fn icon(icon: Icon, size: f32, color: impl Into<Hsla>) -> Svg {
    svg()
        .path(icon.path())
        .size(px(size))
        .flex_none()
        .text_color(color)
}

pub fn mono(text: impl Into<SharedString>, size: f32) -> Div {
    div()
        .font_family(MONO)
        .font_weight(FontWeight::MEDIUM)
        .text_size(px(size))
        .child(text.into())
}

/// Letter-spaced label: GPUI has no tracking, so glyphs sit in a flex row with a gap.
pub fn tracked(text: &str, size: f32, em: f32) -> Div {
    let gap = px(size * em);
    div()
        .flex()
        .flex_wrap()
        .gap_x(px(size * 0.28) + gap)
        .text_size(px(size))
        .children(text.split(' ').map(|word| {
            div()
                .flex()
                .gap(gap)
                .children(word.chars().map(|c| SharedString::from(c.to_string())))
        }))
}

pub fn square(side: f32, color: impl Into<Hsla>) -> Div {
    div().size(px(side)).flex_none().bg(color.into())
}

pub fn keycap(text: &'static str) -> Div {
    mono(text, 10.5)
        .px(px(6.))
        .py(px(3.))
        .border_1()
        .border_color(line(0.16))
        .bg(rgb(RAISED))
        .text_color(rgb(MUTED))
        .whitespace_nowrap()
}

pub fn segment(label: &'static str, fill: Option<u32>, color: u32) -> Div {
    mono(label, 10.)
        .font_weight(FontWeight::BOLD)
        .h_full()
        .flex()
        .items_center()
        .justify_center()
        .text_color(rgb(color))
        .when_some(fill, |cell, fill| cell.bg(rgb(fill)))
}

pub fn segmented() -> Div {
    h_flex().flex_none().border_1().border_color(line(0.16))
}

pub fn toggle(on: bool, accent: u32) -> Div {
    let (off_color, on_color) = if on {
        (SEGMENT_IDLE, BG)
    } else {
        (TEXT, SEGMENT_IDLE)
    };
    segmented()
        .h(px(24.))
        .child(segment("OFF", (!on).then_some(SEGMENT_OFF), off_color).w(px(34.)))
        .child(segment("ON", on.then_some(accent), on_color).w(px(34.)))
}

pub fn icon_button(id: &'static str, glyph: Icon, side: f32, glyph_size: f32) -> Stateful<Div> {
    div()
        .id(id)
        .group(id)
        .size(px(side))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .hover(|style| style.bg(line(0.06)))
        .child(
            icon(glyph, glyph_size, rgb(DIM)).group_hover(id, |style| style.text_color(rgb(TEXT))),
        )
}

pub fn button(label: &str) -> Div {
    h_flex()
        .h(px(26.))
        .px(px(12.))
        .gap(px(8.))
        .flex_none()
        .justify_center()
        .border_1()
        .border_color(line(0.2))
        .bg(rgb(RAISED))
        .font_weight(FontWeight::BOLD)
        .cursor_pointer()
        .hover(|style| style.bg(rgb(RAISED_HOVER)))
        .child(tracked(label, 11., 0.12))
}

/// `/ filter` field: the accent slash and a bare mono input.
pub fn filter(state: &Entity<InputState>, accent: u32) -> Div {
    h_flex()
        .gap(px(8.))
        .bg(rgb(SUNKEN))
        .child(
            mono("/", 12.)
                .font_weight(FontWeight::BOLD)
                .text_color(rgb(accent)),
        )
        .child(
            div().flex_1().min_w_0().child(
                Input::new(state)
                    .appearance(false)
                    .px_0()
                    .font_family(MONO)
                    .text_size(px(12.5)),
            ),
        )
}

/// 1 px background grid, painted because GPUI has no repeating backgrounds.
pub fn grid(step: f32, color: Rgba) -> impl IntoElement {
    canvas(
        |_, _, _| (),
        move |bounds: Bounds<gpui_kit::Pixels>, _, window, _| {
            let mut x = bounds.left();
            while x < bounds.right() {
                let column = Bounds::new(point(x, bounds.top()), size(px(1.), bounds.size.height));
                window.paint_quad(fill(column, color));
                x += px(step);
            }
            let mut y = bounds.top();
            while y < bounds.bottom() {
                let row = Bounds::new(point(bounds.left(), y), size(bounds.size.width, px(1.)));
                window.paint_quad(fill(row, color));
                y += px(step);
            }
        },
    )
    .absolute()
    .top_0()
    .left_0()
    .size_full()
}
