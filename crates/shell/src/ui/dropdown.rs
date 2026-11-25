//! A trigger that opens a list of options under itself. Its view keeps which of its menus is open, one at a time.

use std::{cell::Cell, rc::Rc};

use gpui_kit::{
    Anchor, Bounds, Context, Div, ElementId, MouseButton, Pixels, SharedString, Stateful, Window, anchored,
    base::ElementExt,
    component::{h_flex, v_flex},
    deferred, point,
    prelude::*,
    px, rgb,
};

use super::{
    assets::Icon,
    theme::*,
    widgets::{icon, list_row, mono},
};

/// Where a view keeps its open menu.
pub type Slot<V, M> = fn(&mut V) -> &mut Option<M>;

/// A menu's marked options: the chosen ones, and the first of a second group, under a line.
#[derive(Clone, Copy, Default)]
pub struct Marked<'a> {
    pub chosen: &'a [usize],
    pub group: Option<usize>,
}

impl<'a> Marked<'a> {
    pub fn chosen(chosen: &'a [usize]) -> Self {
        Self { chosen, group: None }
    }
}

/// Makes `trigger` open `options` under its right edge, at least `width` wide, inside the window; `open`: `menu` is the open one;
/// `marks`: the chosen options and the lines between groups.
#[expect(clippy::too_many_arguments, reason = "a menu's trigger, its slot, options, width and pick")]
pub fn dropdown<V: 'static, M: Copy + PartialEq + 'static>(
    trigger: Stateful<Div>,
    menu: M,
    slot: Slot<V, M>,
    open: bool,
    options: Vec<SharedString>,
    marks: Marked<'_>,
    width: Pixels,
    cx: &Context<V>,
    pick: impl Fn(&mut V, usize, &mut Window, &mut Context<V>) + Clone + 'static,
) -> Stateful<Div> {
    trigger
        .relative()
        .on_click(cx.listener(move |view, _, _, cx| {
            let open = slot(view);
            *open = (*open != Some(menu)).then_some(menu);
            cx.notify();
        }))
        .on_mouse_down_out(cx.listener(move |view, _, _, cx| close(view, slot, menu, cx)))
        .when(open && !options.is_empty(), |trigger| {
            let under = Rc::new(Cell::new(Bounds::default()));
            let fitted = anchored()
                .attach_tracked(under.clone(), Anchor::BottomRight)
                .anchor(Anchor::TopRight)
                .offset(point(px(1.), px(4.)))
                .snap_to_window_with_margin(px(8.));
            let view = cx.weak_entity();
            trigger
                .on_prepaint(move |bounds, window, cx| {
                    under.set(bounds);
                    if !window.content_mask().bounds.intersects(&bounds) {
                        cx.defer(move |cx| drop(view.update(cx, |view, cx| close(view, slot, menu, cx))));
                    }
                })
                .child(deferred(fitted.child(picks(options, marks, slot, cx, pick).min_w(width))))
        })
}

/// A closed list: its current option and a chevron.
pub fn trigger(id: impl Into<ElementId>, current: impl Into<SharedString>) -> Stateful<Div> {
    h_flex()
        .id(id)
        .h(px(26.))
        .gap(px(6.))
        .justify_between()
        .pl(px(10.))
        .pr(px(8.))
        .border_1()
        .border_color(line(0.2))
        .bg(rgb(SUNKEN))
        .cursor_pointer()
        .child(mono(current, TYPE_SMALL).min_w_0().truncate())
        .child(icon(Icon::ChevronDown, 11., rgb(DIM)))
}

pub fn close<V: 'static, M: PartialEq>(view: &mut V, slot: Slot<V, M>, menu: M, cx: &mut Context<V>) {
    let open = slot(view);
    if *open == Some(menu) {
        *open = None;
        cx.notify();
    }
}

/// An open menu's list; the caller places it.
pub fn picks<V: 'static, M: 'static>(
    options: Vec<SharedString>,
    marks: Marked<'_>,
    slot: Slot<V, M>,
    cx: &Context<V>,
    pick: impl Fn(&mut V, usize, &mut Window, &mut Context<V>) + Clone + 'static,
) -> Stateful<Div> {
    let accent = rgb(accent(cx));
    v_flex()
        .id("picks")
        .occlude()
        .max_w(px(320.))
        .max_h(px(300.))
        .overflow_y_scroll()
        .py(px(4.))
        .border_1()
        .border_color(line(0.2))
        .bg(rgb(PANEL))
        .children(options.into_iter().enumerate().map(|(at, option)| {
            let pick = pick.clone();
            list_row(("option", at), false)
                .flex_none()
                .h(px(28.))
                .px(px(10.))
                .text_color(if marks.chosen.contains(&at) { accent } else { rgb(TEXT_2) })
                .when(at > 0 && marks.group == Some(at), |row| row.mt(px(4.)).border_t_1().border_color(line(0.12)))
                // Mouse down, not click: the outside-press close removes the list first.
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |view, _, window, cx| {
                        pick(view, at, window, cx);
                        *slot(view) = None;
                        cx.notify();
                    }),
                )
                .child(mono(option, TYPE_SMALL).min_w_0().truncate())
        }))
}
