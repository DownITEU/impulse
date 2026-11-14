//! The control at the right of a cheat row, one per cheat kind.

use client::trainer::{Sheet, Span, Step, TOP_TICK};
use gpui_kit::{
    AnyElement, Context, Div, IntoElement, MouseButton, MouseMoveEvent, ScrollEvent, SharedString, Stateful, Window,
    component::{h_flex, input::Input, v_flex},
    deferred, div,
    prelude::*,
    px, relative, rgb,
};
use model::Kind;

use super::{
    Flash, Menu, Page,
    assets::Icon,
    theme::*,
    trainer::{Printed, number},
    widgets::{button, flash_button, icon, mono, toggle},
};

impl Page {
    /// A hotkey slot of the live trainer's cheat; a numeric field takes the new value too.
    pub(super) fn hotkey(&mut self, index: usize, slot: usize, cx: &mut Context<Self>) {
        let Some(game) = self.play.live().cloned() else { return };
        match self.sheet(&game, cx).and_then(|sheet| sheet.step(index, slot)) {
            Some(Step::Press(pressed)) => {
                self.push_value(index, pressed, cx);
                if self.selected(cx).as_ref() == Some(&game) {
                    self.flash(Flash::Cheat(index), cx);
                }
            }
            Some(Step::Set(next)) => {
                self.trainers.update(cx, |trainers, cx| trainers.set(&game, index, next, cx));
                self.push_value(index, next, cx);
            }
            None => {}
        }
    }

    fn set_value(&mut self, index: usize, value: f64, cx: &mut Context<Self>) {
        if let Some(game) = self.selected(cx) {
            self.trainers.update(cx, |trainers, cx| trainers.set(&game, index, value, cx));
        }
        self.push_value(index, value, cx);
    }

    pub(super) fn control(
        &self,
        index: usize,
        sheet: &Sheet,
        printed: &Printed,
        running: bool,
        live: bool,
        cx: &Context<Self>,
    ) -> AnyElement {
        let cheat = &sheet.cheats[index];
        let value = sheet.values[index];
        let value_color = if live { rgb(accent(cx)) } else { rgb(DIM) };
        match cheat.kind {
            Kind::Toggle => div()
                .id(("toggle", index))
                .cursor_pointer()
                .on_click(cx.listener(move |page, _, _, cx| {
                    if running {
                        page.set_value(index, if value == 0. { 1. } else { 0. }, cx);
                    }
                }))
                .child(toggle(live, cx))
                .into_any_element(),
            Kind::Slider => {
                let span = Span::of(&cheat.args);
                h_flex()
                    .gap(px(5.))
                    .child(self.ticks(index, span, value, running, live, cx))
                    .child(mono(number(value), 12.5).w(px(38.)).flex().justify_end().text_color(value_color))
                    .into_any_element()
            }
            Kind::Number if cheat.args.button => h_flex()
                .gap(px(8.))
                .when_some(printed.inputs.get(&index), |control, input| {
                    control.child(
                        value_box("").w(px(72.)).child(
                            Input::new(input)
                                .appearance(false)
                                .disabled(!running)
                                .px(px(8.))
                                .text_center()
                                .font_family(MONO)
                                .text_size(px(12.5)),
                        ),
                    )
                })
                .child(self.press(index, "SET", "OK", running, cx))
                .into_any_element(),
            Kind::Number => {
                let step = cheat.args.step.unwrap_or(1.);
                let nudge = |id: &'static str, label: &'static str, by: f64| {
                    button(label).id((id, index)).w(px(26.)).px_0().font_family(MONO).text_size(px(14.)).on_click(cx.listener(
                        move |page, _, _, cx| {
                            let game = page.selected(cx).filter(|_| running);
                            if let Some(next) = game.and_then(|game| Some(page.sheet(&game, cx)?.cheats[index].args.clamp(value + by))) {
                                page.set_value(index, next, cx);
                            }
                        },
                    ))
                };
                h_flex()
                    .gap(px(8.))
                    .child(nudge("decrease", "−", -step))
                    .child(value_box(number(value)).w(px(52.)).justify_center().text_color(value_color))
                    .child(nudge("increase", "+", step))
                    .into_any_element()
            }
            Kind::Button => self.press(index, "EXECUTE", "APPLIED", running, cx).px(px(14.)).into_any_element(),
            Kind::Selection => {
                let options = printed.rows[index].options.clone();
                let current = value as usize;
                let trigger = value_box(options.get(current).cloned().unwrap_or_default())
                    .id(("select", index))
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
                self.dropdown(trigger, Menu::Cheat(index), options, current, span, cx, move |page, option, _, cx| {
                    page.set_value(index, option as f64, cx)
                })
                .into_any_element()
            }
            Kind::Unknown => div().into_any_element(),
        }
    }

    /// A button with no state of its own: it confirms the press for a moment.
    fn press(&self, index: usize, label: &str, done: &str, running: bool, cx: &Context<Self>) -> Stateful<Div> {
        flash_button(label, done, self.is_flashed(Flash::Cheat(index)), cx).id(("press", index)).on_click(cx.listener(
            move |page, _, _, cx| {
                if !running {
                    return;
                }
                // A cheat with a text field takes its value from it; a button cheat sends its own and
                // keeps no state.
                let Some(game) = page.selected(cx) else { return };
                let (typed, fired) = page.sheet(&game, cx).map_or((None, None), |sheet| {
                    let cheat = &sheet.cheats[index];
                    let input = page.printed.as_ref().and_then(|printed| printed.inputs.get(&index));
                    let typed = input.and_then(|input| Some(cheat.args.clamp(input.read(cx).value().parse().ok()?)));
                    (typed, (cheat.kind == Kind::Button).then_some(cheat.args.pressed))
                });
                if let Some(value) = typed {
                    page.set_value(index, value, cx);
                }
                if let Some(value) = fired {
                    page.push_value(index, value, cx);
                }
                page.flash(Flash::Cheat(index), cx);
            },
        ))
    }

    /// Ten ticks: click or drag across them, or use the wheel.
    fn ticks(&self, index: usize, span: Span, value: f64, running: bool, live: bool, cx: &Context<Self>) -> impl IntoElement {
        let lit = if live { accent(cx) } else { TICK_IDLE };
        let level = span.tick(value);
        h_flex()
            .when(running, |ticks| {
                ticks.cursor_pointer().on_scroll(cx.listener(move |page, event: &ScrollEvent, window, cx| {
                    let up = event.delta.pixel_delta(window.line_height()).y > px(0.);
                    page.set_value(index, span.nudge(value, up), cx);
                    // Keeps the pane from scrolling under the slider.
                    cx.stop_propagation();
                }))
            })
            .children((0..=TOP_TICK).map(|tick| {
                // The 3 px gap belongs to the cell, so a drag never falls between ticks.
                div()
                    .pr(px(3.))
                    .when(running, |cell| {
                        cell.on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |page, _, _, cx| {
                                page.set_value(index, span.value(tick), cx);
                            }),
                        )
                        .on_mouse_move(cx.listener(move |page, event: &MouseMoveEvent, _, cx| {
                            if event.pressed_button == Some(MouseButton::Left) {
                                page.set_value(index, span.value(tick), cx);
                            }
                        }))
                    })
                    .child(div().w(px(6.)).h(px(15.)).bg(rgb(if tick <= level { lit } else { TICK })))
            }))
    }

    /// Makes `trigger` open a list of `options` under itself, set across by `place`.
    #[expect(clippy::too_many_arguments, reason = "a menu's trigger, options, placement and pick")]
    pub(super) fn dropdown(
        &self,
        trigger: Stateful<Div>,
        menu: Menu,
        options: Vec<SharedString>,
        current: usize,
        place: fn(Div) -> Div,
        cx: &Context<Self>,
        pick: impl Fn(&mut Page, usize, &mut Window, &mut Context<Page>) + Copy + 'static,
    ) -> Stateful<Div> {
        trigger
            .relative()
            .on_click(cx.listener(move |page, _, _, cx| {
                page.menu = (page.menu != Some(menu)).then_some(menu);
                cx.notify();
            }))
            .on_mouse_down_out(cx.listener(move |page, _, _, cx| {
                if page.menu == Some(menu) {
                    page.menu = None;
                    cx.notify();
                }
            }))
            .when(self.menu == Some(menu) && !options.is_empty(), |trigger| {
                trigger.child(deferred(place(self.picks(options, current, cx, pick))))
            })
    }

    /// An open menu's list, under what opened it; the caller places it across.
    pub(super) fn picks(
        &self,
        options: Vec<SharedString>,
        current: usize,
        cx: &Context<Self>,
        pick: impl Fn(&mut Page, usize, &mut Window, &mut Context<Page>) + Copy + 'static,
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
                        cx.listener(move |page, _, window, cx| {
                            pick(page, at, window, cx);
                            page.menu = None;
                            cx.notify();
                        }),
                    )
                    .child(mono(option, 12.))
            }),
        )
    }
}

/// A dropdown's list across its trigger, borders included.
pub(super) fn span(list: Div) -> Div {
    list.left(px(-1.)).right(px(-1.))
}

fn value_box(value: impl Into<SharedString>) -> Div {
    mono(value, 12.5).h(px(26.)).flex().items_center().border_1().border_color(line(0.14)).bg(rgb(SUNKEN))
}
