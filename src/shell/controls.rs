//! The control at the right of a cheat row, one per `Control` kind.

use gpui_kit::{
    AnyElement, Context, Div, IntoElement, MouseButton, MouseMoveEvent, ScrollEvent, SharedString,
    Stateful,
    component::{h_flex, input::Input, v_flex},
    deferred, div,
    prelude::*,
    px, relative, rgb,
};

use super::{
    Page,
    assets::Icon,
    mock::{Control, Mod},
    theme::*,
    widgets::{button, icon, mono, toggle},
};

const TOP_LEVEL: u8 = 9;

impl Page {
    pub(super) fn level(&self, cheat: &Mod) -> u8 {
        let initial = match cheat.control {
            Control::Slider(level) => level,
            _ => 0,
        };
        self.levels.get(cheat.id).copied().unwrap_or(initial)
    }

    pub(super) fn control(
        &self,
        number: usize,
        cheat: &'static Mod,
        running: bool,
        live: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let value_color = if live { rgb(self.accent) } else { rgb(DIM) };
        match cheat.control {
            Control::Toggle => div()
                .id(("toggle", number))
                .cursor_pointer()
                .on_click(cx.listener(move |page, _, _, cx| {
                    if running && !page.enabled.remove(cheat.id) {
                        page.enabled.insert(cheat.id);
                    }
                    cx.notify();
                }))
                .child(toggle(live, self.accent))
                .into_any_element(),
            Control::Slider(_) => {
                let level = if running { self.level(cheat) } else { 0 };
                h_flex()
                    .gap(px(5.))
                    .child(self.ticks(cheat, level, running, live, cx))
                    .child(
                        mono(format!("{:.1}×", 1. + 0.15 * f32::from(level)), 12.5)
                            .w(px(38.))
                            .flex()
                            .justify_end()
                            .text_color(value_color),
                    )
                    .into_any_element()
            }
            Control::Incremental => {
                let step = |id: &'static str, label: &'static str, delta: i32| {
                    button(label)
                        .id(id)
                        .w(px(26.))
                        .px_0()
                        .font_family(MONO)
                        .text_size(px(14.))
                        .on_click(cx.listener(move |page, _, _, cx| {
                            if running {
                                page.crit = (page.crit + delta).clamp(0, 100);
                            }
                            cx.notify();
                        }))
                };
                let crit = if running { self.crit } else { 0 };
                h_flex()
                    .gap(px(8.))
                    .child(step("decrease", "−", -5))
                    .child(
                        value_box(crit.to_string())
                            .w(px(44.))
                            .justify_center()
                            .text_color(value_color),
                    )
                    .child(step("increase", "+", 5))
                    .into_any_element()
            }
            Control::Number => h_flex()
                .gap(px(8.))
                .child(
                    value_box("").w(px(72.)).child(
                        Input::new(&self.amount)
                            .appearance(false)
                            .disabled(!running)
                            .px(px(8.))
                            .text_center()
                            .font_family(MONO)
                            .text_size(px(12.5)),
                    ),
                )
                .child(button("SET"))
                .into_any_element(),
            Control::Button => button("EXECUTE").px(px(14.)).into_any_element(),
            Control::Selection(options) => {
                let current = self.choices.get(cheat.id).copied().unwrap_or(options[0]);
                let trigger = value_box(current)
                    .id(("select", number))
                    .w(px(112.))
                    .pl(px(10.))
                    .pr(px(8.))
                    .justify_between()
                    .border_color(line(0.2))
                    .text_size(px(12.))
                    .cursor_pointer()
                    .child(icon(Icon::ChevronDown, 11., rgb(DIM)));
                if !running {
                    return trigger.into_any_element();
                }
                self.dropdown(
                    trigger,
                    cheat.id,
                    options,
                    current,
                    cx,
                    move |page, option| {
                        page.choices.insert(cheat.id, option);
                    },
                )
                .into_any_element()
            }
        }
    }

    /// Ten ticks: click or drag across them, or use the wheel.
    fn ticks(
        &self,
        cheat: &'static Mod,
        level: u8,
        running: bool,
        live: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let lit = if live { self.accent } else { TICK_IDLE };
        let set = move |page: &mut Page, level: u8, cx: &mut Context<Page>| {
            page.levels.insert(cheat.id, level);
            cx.notify();
        };
        h_flex()
            .when(running, |ticks| {
                ticks.cursor_pointer().on_scroll(cx.listener(
                    move |page, event: &ScrollEvent, window, cx| {
                        let up = event.delta.pixel_delta(window.line_height()).y > px(0.);
                        let level = page.level(cheat);
                        set(
                            page,
                            if up {
                                (level + 1).min(TOP_LEVEL)
                            } else {
                                level.saturating_sub(1)
                            },
                            cx,
                        );
                        // Keeps the pane from scrolling under the slider.
                        cx.stop_propagation();
                    },
                ))
            })
            .children((0..=TOP_LEVEL).map(|tick| {
                // The 3 px gap belongs to the cell, so a drag never falls between ticks.
                div()
                    .pr(px(3.))
                    .when(running, |cell| {
                        cell.on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |page, _, _, cx| set(page, tick, cx)),
                        )
                        .on_mouse_move(cx.listener(
                            move |page, event: &MouseMoveEvent, _, cx| {
                                if event.pressed_button == Some(MouseButton::Left) {
                                    set(page, tick, cx);
                                }
                            },
                        ))
                    })
                    .child(div().w(px(6.)).h(px(15.)).bg(rgb(if tick <= level {
                        lit
                    } else {
                        TICK
                    })))
            }))
    }

    /// Makes `trigger` open a list of `options` under itself.
    pub(super) fn dropdown(
        &self,
        trigger: Stateful<Div>,
        id: &'static str,
        options: &'static [&'static str],
        current: &'static str,
        cx: &mut Context<Self>,
        pick: impl Fn(&mut Page, &'static str) + Copy + 'static,
    ) -> Stateful<Div> {
        let accent = rgb(self.accent);
        trigger
            .relative()
            .on_click(cx.listener(move |page, _, _, cx| {
                page.menu = (page.menu != Some(id)).then_some(id);
                cx.notify();
            }))
            .on_mouse_down_out(cx.listener(move |page, _, _, cx| {
                if page.menu == Some(id) {
                    page.menu = None;
                    cx.notify();
                }
            }))
            .when(self.menu == Some(id), |trigger| {
                trigger.child(deferred(
                    v_flex()
                        .occlude()
                        .absolute()
                        .top(relative(1.))
                        // Spans the trigger, borders included.
                        .left(px(-1.))
                        .right(px(-1.))
                        .mt(px(3.))
                        .py(px(4.))
                        .border_1()
                        .border_color(line(0.2))
                        .bg(rgb(PANEL))
                        .children(options.iter().map(|&option| {
                            let chosen = option == current;
                            h_flex()
                                .h(px(28.))
                                .px(px(10.))
                                .cursor_pointer()
                                .text_color(if chosen { accent } else { rgb(TEXT_2) })
                                .hover(|style| style.bg(rgb(ROW_HOVER)))
                                // Mouse down, not click: the outside-press close removes the list first.
                                .on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(move |page, _, _, cx| {
                                        pick(page, option);
                                        page.menu = None;
                                        cx.notify();
                                    }),
                                )
                                .child(mono(option, 12.))
                        })),
                ))
            })
    }
}

fn value_box(value: impl Into<SharedString>) -> Div {
    mono(value, 12.5)
        .h(px(26.))
        .flex()
        .items_center()
        .border_1()
        .border_color(line(0.14))
        .bg(rgb(SUNKEN))
}
