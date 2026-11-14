//! How docks look: the frame around a dock's content, the toolbar of toggles, and the window's mouse while
//! one is moved or sized.

use gpui_kit::{
    AnyElement, App, Bounds, CursorStyle, DispatchPhase, Entity, FontWeight, IntoElement, MouseButton, MouseDownEvent, MouseMoveEvent,
    MouseUpEvent, Pixels, Window, canvas, component::h_flex, div, prelude::*, px, rgb,
};

use super::{Docks, Spec};
use crate::ui::{
    assets::Icon,
    theme::*,
    widgets::{icon, icon_button, square, tip, tracked},
};

pub(super) const HEADER: f32 = 32.;
const GRIP: f32 = 14.;
const TOGGLE: f32 = 40.;

/// The frames around `docked`, the shown docks back to front with their content.
pub fn frame(docks: &Entity<Docks>, docked: Vec<(&'static Spec, AnyElement)>, window: &Window, cx: &App) -> Vec<AnyElement> {
    let state = docks.read(cx);
    let screen = window.viewport_size();
    let grab = |spec: &'static Spec, resize| {
        let docks = docks.clone();
        move |event: &MouseDownEvent, window: &mut Window, cx: &mut App| {
            let screen = window.viewport_size();
            docks.update(cx, |docks, cx| docks.grab(spec, resize, event.position, screen, cx));
        }
    };
    docked
        .into_iter()
        .map(|(spec, content)| {
            let bounds = state.bounds(spec, screen);
            let hide = docks.clone();
            div()
                .id(spec.id)
                .occlude()
                .absolute()
                .left(bounds.origin.x)
                .top(bounds.origin.y)
                .w(bounds.size.width)
                .h(bounds.size.height)
                .flex()
                .flex_col()
                .bg(rgb(BG))
                .border_1()
                .border_color(line(0.14))
                .child(
                    h_flex()
                        .id("header")
                        .h(px(HEADER))
                        .flex_none()
                        .pl(px(12.))
                        .pr(px(2.))
                        .gap(px(10.))
                        .border_b_1()
                        .border_color(line(0.09))
                        .on_mouse_down(MouseButton::Left, grab(spec, false))
                        .child(square(7., rgb(accent(cx))))
                        .child(tracked(spec.title, 11., 0.2).font_weight(FontWeight::BOLD))
                        .child(div().flex_1())
                        // Occluding, it keeps its press from the header's drag.
                        .child(
                            icon_button("close", Icon::Close, 28., 11.)
                                .occlude()
                                .on_click(move |_, _, cx| hide.update(cx, |docks, cx| docks.hide(spec, cx))),
                        ),
                )
                .child(
                    div()
                        .flex_1()
                        .min_h_0()
                        .overflow_hidden()
                        .when(spec.body_moves, |body| body.on_mouse_down(MouseButton::Left, grab(spec, false)))
                        .child(content),
                )
                .when(spec.smallest.is_some(), |frame| {
                    frame.child(
                        // Occluding, it keeps its press from a body that moves the dock.
                        div()
                            .id("grip")
                            .occlude()
                            .absolute()
                            .right_0()
                            .bottom_0()
                            .size(px(GRIP))
                            .cursor(CursorStyle::ResizeUpLeftDownRight)
                            .on_mouse_down(MouseButton::Left, grab(spec, true)),
                    )
                })
                .into_any_element()
        })
        .collect()
}

/// A dock's frame below its header, the border it shares with the header included.
pub fn below_header(mut bounds: Bounds<Pixels>) -> Bounds<Pixels> {
    bounds.origin.y += px(HEADER);
    bounds.size.height -= px(HEADER);
    bounds
}

/// A toggle per dock; `usable` says which may show for what is open.
pub fn toolbar(docks: &Entity<Docks>, cx: &App, usable: impl Fn(&Spec) -> bool) -> impl IntoElement {
    let accent = accent(cx);
    let state = docks.read(cx);
    h_flex().gap(px(4.)).children(state.specs.iter().map(|spec| {
        let (usable, shown) = (usable(spec), state.is_shown(spec));
        let docks = docks.clone();
        let color = match (usable, shown) {
            (false, _) => rgb(GHOST),
            (true, true) => rgb(accent),
            (true, false) => rgb(DIM),
        };
        div()
            .id(spec.id)
            .size(px(TOGGLE))
            .flex()
            .items_center()
            .justify_center()
            .border_1()
            .border_color(if usable && shown { alpha(accent, 0.6) } else { line(0.12) })
            .bg(alpha(BG, 0.85))
            .tooltip(move |window, cx| tip(spec.title.into(), window, cx))
            .child(icon(spec.icon, 17., color))
            .when(usable, |toggle| {
                toggle
                    .cursor_pointer()
                    .hover(|style| style.bg(rgb(RAISED_HOVER)))
                    .on_click(move |_, _, cx| docks.update(cx, |docks, cx| docks.toggle(spec, cx)))
            })
    }))
}

/// On the window, not an element: a press over any dock raises it, and a moved or sized one follows the
/// pointer past its own edges.
pub fn input(docks: &Entity<Docks>) -> impl IntoElement {
    let docks = docks.clone();
    canvas(
        |_, _, _| {},
        move |_, _, window, _| {
            let pressed = docks.clone();
            window.on_mouse_event(move |event: &MouseDownEvent, phase, window, cx| {
                if phase == DispatchPhase::Capture {
                    let screen = window.viewport_size();
                    pressed.update(cx, |docks, cx| docks.press(event.position, screen, cx));
                }
            });
            let moved = docks.clone();
            window.on_mouse_event(move |event: &MouseMoveEvent, phase, window, cx| {
                if phase == DispatchPhase::Bubble && moved.read(cx).grab.is_some() {
                    let screen = window.viewport_size();
                    moved.update(cx, |docks, cx| docks.follow(event, screen, cx));
                }
            });
            let released = docks.clone();
            window.on_mouse_event(move |_: &MouseUpEvent, phase, _, cx| {
                if phase == DispatchPhase::Bubble {
                    released.update(cx, |docks, cx| docks.release(cx));
                }
            });
        },
    )
    .absolute()
    .size_full()
}
