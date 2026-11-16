//! A trigger that opens a list of options under itself. Its view keeps which of its menus is open, one at a time.

use gpui_kit::{
    Context, Div, ElementId, MouseButton, SharedString, Stateful, Window,
    component::{h_flex, v_flex},
    deferred,
    prelude::*,
    px, relative, rgb,
};

use super::{
    assets::Icon,
    theme::*,
    widgets::{icon, mono},
};

/// Where a view keeps its open menu.
pub type Slot<V, M> = fn(&mut V) -> &mut Option<M>;

/// Makes `trigger` open `options` under itself, set across by `place`; `open`: `menu` is the open one.
#[expect(clippy::too_many_arguments, reason = "a menu's trigger, its slot, options, placement and pick")]
pub fn dropdown<V: 'static, M: Copy + PartialEq + 'static>(
    trigger: Stateful<Div>,
    menu: M,
    slot: Slot<V, M>,
    open: bool,
    options: Vec<SharedString>,
    current: usize,
    place: fn(Div) -> Div,
    cx: &Context<V>,
    pick: impl Fn(&mut V, usize, &mut Window, &mut Context<V>) + Copy + 'static,
) -> Stateful<Div> {
    trigger
        .relative()
        .on_click(cx.listener(move |view, _, _, cx| {
            let open = slot(view);
            *open = (*open != Some(menu)).then_some(menu);
            cx.notify();
        }))
        .on_mouse_down_out(cx.listener(move |view, _, _, cx| close(view, slot, menu, cx)))
        .when(open && !options.is_empty(), |trigger| trigger.child(deferred(place(picks(options, current, slot, cx, pick)))))
}

/// A closed list: its current option and a chevron.
pub fn trigger(id: impl Into<ElementId>, current: impl Into<SharedString>) -> Stateful<Div> {
    mono(current, 12.)
        .id(id)
        .h(px(26.))
        .flex()
        .items_center()
        .justify_between()
        .pl(px(10.))
        .pr(px(8.))
        .border_1()
        .border_color(line(0.2))
        .bg(rgb(SUNKEN))
        .cursor_pointer()
        .child(icon(Icon::ChevronDown, 11., rgb(DIM)))
}

/// Closes `menu` when it is the open one.
pub fn close<V: 'static, M: PartialEq>(view: &mut V, slot: Slot<V, M>, menu: M, cx: &mut Context<V>) {
    let open = slot(view);
    if *open == Some(menu) {
        *open = None;
        cx.notify();
    }
}

/// An open menu's list, under what opened it; the caller places it across.
pub fn picks<V: 'static, M: 'static>(
    options: Vec<SharedString>,
    current: usize,
    slot: Slot<V, M>,
    cx: &Context<V>,
    pick: impl Fn(&mut V, usize, &mut Window, &mut Context<V>) + Copy + 'static,
) -> Div {
    let accent = rgb(accent(cx));
    v_flex().occlude().absolute().top(relative(1.)).mt(px(3.)).py(px(4.)).border_1().border_color(line(0.2)).bg(rgb(PANEL)).children(
        options.into_iter().enumerate().map(|(at, option)| {
            h_flex()
                .id(("option", at))
                .h(px(28.))
                .px(px(10.))
                .cursor_pointer()
                .text_color(if at == current { accent } else { rgb(TEXT_2) })
                .hover(|style| style.bg(rgb(ROW_HOVER)))
                // Mouse down, not click: the outside-press close removes the list first.
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |view, _, window, cx| {
                        pick(view, at, window, cx);
                        *slot(view) = None;
                        cx.notify();
                    }),
                )
                .child(mono(option, 12.))
        }),
    )
}

/// A dropdown's list across its trigger, borders included.
pub fn span(list: Div) -> Div {
    list.left(px(-1.)).right(px(-1.))
}
