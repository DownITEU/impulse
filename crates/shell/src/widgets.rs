use std::time::Duration;

use gpui_kit::{
    AnyView, App, Bounds, Context, Div, ElementId, Entity, FontWeight, Hsla, InteractiveText, IntoElement, Rgba, SharedString, Stateful,
    StyledText, Svg, Task, Window, canvas,
    component::{
        h_flex,
        input::{Input, InputState},
        tooltip::Tooltip,
        v_flex,
    },
    div, fill, point,
    prelude::*,
    px, rgb, size, svg,
};

use super::{assets::Icon, theme::*};

const FLASH: Duration = Duration::from_millis(900);
const TIP_WIDTH: f32 = 320.;

pub fn icon(icon: Icon, size: f32, color: impl Into<Hsla>) -> Svg {
    svg().path(icon.path()).size(px(size)).flex_none().text_color(color)
}

pub fn mono(text: impl Into<SharedString>, size: f32) -> Div {
    div().font_family(MONO).font_weight(FontWeight::MEDIUM).text_size(px(size)).child(text.into())
}

/// Letter-spaced label: GPUI has no tracking, so glyphs sit in a flex row with a gap.
pub fn tracked(text: &str, size: f32, em: f32) -> Div {
    let gap = px(size * em);
    div()
        .flex()
        .flex_wrap()
        .gap_x(px(size * 0.28) + gap)
        .text_size(px(size))
        .children(text.split(' ').map(|word| div().flex().gap(gap).children(word.chars().map(|c| SharedString::from(c.to_string())))))
}

pub fn square(side: f32, color: impl Into<Hsla>) -> Div {
    div().size(px(side)).flex_none().bg(color.into())
}

/// Over the corners of a `relative` box with a 1 px border.
pub fn brackets(cx: &App) -> [Div; 4] {
    let accent = accent(cx);
    let bracket = || div().absolute().size(px(10.)).border_color(rgb(accent));
    [
        bracket().left(px(-1.)).top(px(-1.)).border_l_2().border_t_2(),
        bracket().right(px(-1.)).top(px(-1.)).border_r_2().border_t_2(),
        bracket().left(px(-1.)).bottom(px(-1.)).border_l_2().border_b_2(),
        bracket().right(px(-1.)).bottom(px(-1.)).border_r_2().border_b_2(),
    ]
}

/// Shrinks with its row: the text is cut, and shown whole in a tooltip.
pub fn keycap(id: impl Into<ElementId>, text: SharedString) -> Stateful<Div> {
    div()
        .id(id)
        .min_w_0()
        .px(px(6.))
        .py(px(3.))
        .border_1()
        .border_color(line(0.16))
        .bg(rgb(RAISED))
        .text_color(rgb(MUTED))
        .font_family(MONO)
        .font_weight(FontWeight::MEDIUM)
        .text_size(px(10.5))
        .truncate()
        .child(fitted("text", text))
}

/// Text for a parent that truncates: a tooltip holds the whole of it when it was cut.
pub fn fitted(id: impl Into<ElementId>, text: SharedString) -> InteractiveText {
    let styled = StyledText::new(text.clone());
    let layout = styled.layout().clone();
    InteractiveText::new(id, styled).tooltip(move |_, window, cx| (layout.text() != *text).then(|| tip(text.clone(), window, cx)))
}

/// A tooltip that wraps instead of running across the window.
pub fn tip(text: SharedString, window: &mut Window, cx: &mut App) -> AnyView {
    Tooltip::element(move |_, _| div().max_w(px(TIP_WIDTH)).child(text.clone())).build(window, cx)
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

/// One option of a `segmented` choice; the caller adds the click.
pub fn choice(id: &'static str, label: &'static str, current: bool, cx: &App) -> Stateful<Div> {
    let accent = accent(cx);
    div()
        .id(id)
        .h_full()
        .cursor_pointer()
        .child(segment(label, current.then_some(accent), if current { BG } else { SEGMENT_IDLE }).px(px(12.)))
}

pub fn toggle(on: bool, cx: &App) -> Div {
    let accent = accent(cx);
    let (off_color, on_color) = if on { (SEGMENT_IDLE, BG) } else { (TEXT, SEGMENT_IDLE) };
    segmented()
        .h(px(24.))
        .child(segment("OFF", (!on).then_some(SEGMENT_OFF), off_color).w(px(34.)))
        .child(segment("ON", on.then_some(accent), on_color).w(px(34.)))
}

pub fn icon_button(id: impl Into<ElementId>, glyph: Icon, side: f32, glyph_size: f32) -> Stateful<Div> {
    const GROUP: &str = "icon-button";
    div()
        .id(id)
        .group(GROUP)
        .size(px(side))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .hover(|style| style.bg(line(0.06)))
        .child(icon(glyph, glyph_size, rgb(DIM)).group_hover(GROUP, |style| style.text_color(rgb(TEXT))))
}

/// A dashed card that stands in for content that is not there.
pub fn notice(tag: &'static str, title: &str, text: &str) -> Div {
    v_flex()
        .items_center()
        .gap(px(14.))
        .px(px(24.))
        .py(px(56.))
        .border_1()
        .border_dashed()
        .border_color(line(0.2))
        .bg(alpha(BG, 0.7))
        .child(mono(tag, 11.).text_color(rgb(DIM)))
        .child(tracked(title, 22., 0.1).justify_center().font_weight(FontWeight::BOLD))
        .child(div().max_w(px(420.)).text_center().text_color(rgb(MUTED)).child(text.to_owned()))
}

pub fn button(label: &str) -> Div {
    button_frame().child(tracked(label, 11., 0.12))
}

/// A press-only button's confirmation: `done` in the accent color while `flashed`.
pub fn flash_button(label: &str, done: &str, flashed: bool, cx: &App) -> Div {
    if !flashed {
        return button(label);
    }
    let overlay = h_flex().absolute().top_0().left_0().size_full();
    button_frame()
        .relative()
        .border_color(rgb(accent(cx)))
        .text_color(rgb(accent(cx)))
        // The hidden label keeps the button's width.
        .child(tracked(label, 11., 0.12).invisible())
        .child(overlay.justify_center().child(tracked(done, 11., 0.12)))
}

/// Runs `clear` once the confirmation has been shown; dropping the task cancels it.
pub fn flash<T: 'static>(cx: &mut Context<T>, clear: impl FnOnce(&mut T) + 'static) -> Task<()> {
    cx.spawn(async move |this, cx| {
        cx.background_executor().timer(FLASH).await;
        this.update(cx, |this, cx| {
            clear(this);
            cx.notify();
        })
        .ok();
    })
}

fn button_frame() -> Div {
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
}

/// `/ filter` field: the accent slash and a bare mono input.
pub fn filter(state: &Entity<InputState>, cx: &App) -> Div {
    h_flex()
        .gap(px(8.))
        .bg(rgb(SUNKEN))
        .child(mono("/", 12.).font_weight(FontWeight::BOLD).text_color(rgb(accent(cx))))
        .child(div().flex_1().min_w_0().child(Input::new(state).appearance(false).px_0().font_family(MONO).text_size(px(12.5))))
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
