//! The control at the right of a cheat row, one per cheat kind, and the saved bar over the rows.

use client::{
    library::GameId,
    trainer::{Sheet, Span, TOP_TICK},
};
use gpui_kit::{
    AnyElement, Context, Div, IntoElement, MouseButton, MouseMoveEvent, ScrollEvent, SharedString, Stateful,
    component::{h_flex, input::Input},
    div,
    prelude::*,
    px, rgb,
};
use i18n::{Arg, Key};
use model::Kind;

use super::{CheatList, Menu, Printed, number, open_menu};
use crate::ui::{
    dropdown::{dropdown, span, trigger},
    key,
    text::{t, tf},
    theme::*,
    widgets::{button, flash_button, mono, tip, toggle},
};

impl CheatList {
    fn set_value(&mut self, index: usize, value: f64, cx: &mut Context<Self>) {
        self.runs.update(cx, |runs, cx| runs.set(index, value, cx));
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
                .on_click(cx.listener(move |list, _, _, cx| {
                    if running {
                        list.set_value(index, if value == 0. { 1. } else { 0. }, cx);
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
                .child(self.press(index, key::CHEATS_SET, key::CHEATS_SET_DONE, running, cx))
                .into_any_element(),
            Kind::Number => {
                let step = cheat.args.stride();
                let nudge = |id: &'static str, label: &'static str, by: f64| {
                    button(label).id((id, index)).w(px(26.)).px_0().font_family(MONO).text_size(px(14.)).on_click(cx.listener(
                        move |list, _, _, cx| {
                            let game = list.selected(cx).filter(|_| running);
                            let next =
                                game.and_then(|game| Some(list.trainers.read(cx).sheet(&game)?.cheats[index].args.clamp(value + by)));
                            if let Some(next) = next {
                                list.set_value(index, next, cx);
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
            Kind::Button => self.press(index, key::CHEATS_EXECUTE, key::CHEATS_EXECUTE_DONE, running, cx).px(px(14.)).into_any_element(),
            Kind::Selection => {
                let options = printed.rows[index].options.clone();
                let current = value as usize;
                let trigger = trigger(("select", index), options.get(current).cloned().unwrap_or_default()).w(px(112.));
                if !running {
                    return trigger.into_any_element();
                }
                let menu = Menu::Cheat(index);
                dropdown(trigger, menu, open_menu, self.menu == Some(menu), options, current, span, cx, move |list, option, _, cx| {
                    list.set_value(index, option as f64, cx)
                })
                .into_any_element()
            }
            Kind::Unknown => div().into_any_element(),
        }
    }

    /// A button with no state of its own: it confirms the press for a moment.
    fn press(&self, index: usize, label: Key, done: Key, running: bool, cx: &Context<Self>) -> Stateful<Div> {
        flash_button(&t(cx, label), &t(cx, done), self.is_flashed(index), cx).id(("press", index)).on_click(cx.listener(
            move |list, _, _, cx| {
                if !running {
                    return;
                }
                // A cheat with a text field takes its value from it; a button cheat sends its own and
                // keeps no state.
                let Some(game) = list.selected(cx) else { return };
                let (typed, button) = list.trainers.read(cx).sheet(&game).map_or((None, false), |sheet| {
                    let cheat = &sheet.cheats[index];
                    let input = list.printed.as_ref().and_then(|printed| printed.inputs.get(&index));
                    let typed = input.and_then(|input| Some(cheat.args.clamp(input.read(cx).value().parse().ok()?)));
                    (typed, cheat.kind == Kind::Button)
                });
                if let Some(value) = typed {
                    list.set_value(index, value, cx);
                }
                if button {
                    list.runs.update(cx, |runs, cx| runs.press(index, cx));
                }
                list.flash(index, cx);
            },
        ))
    }

    /// Ten ticks: click or drag across them, or use the wheel.
    fn ticks(&self, index: usize, span: Span, value: f64, running: bool, live: bool, cx: &Context<Self>) -> impl IntoElement {
        let lit = if live { accent(cx) } else { TICK_IDLE };
        let level = span.tick(value);
        h_flex()
            .when(running, |ticks| {
                ticks.cursor_pointer().on_scroll(cx.listener(move |list, event: &ScrollEvent, window, cx| {
                    let up = event.delta.pixel_delta(window.line_height()).y > px(0.);
                    list.set_value(index, span.nudge(value, up), cx);
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
                            cx.listener(move |list, _, _, cx| {
                                list.set_value(index, span.value(tick), cx);
                            }),
                        )
                        .on_mouse_move(cx.listener(move |list, event: &MouseMoveEvent, _, cx| {
                            if event.pressed_button == Some(MouseButton::Left) {
                                list.set_value(index, span.value(tick), cx);
                            }
                        }))
                    })
                    .child(div().w(px(6.)).h(px(15.)).bg(rgb(if tick <= level { lit } else { TICK })))
            }))
    }

    fn reset_mods(&mut self, cx: &mut Context<Self>) {
        let Some(game) = self.selected(cx) else { return };
        self.runs.update(cx, |runs, cx| runs.reset(&game, cx));
    }

    /// The right of the filter row: the game's save switch and the reset.
    pub(super) fn saved_bar(&self, game: &GameId, sheet: &Sheet, running: bool, cx: &Context<Self>) -> impl IntoElement {
        let profile = self.trainers.read(cx).trainer_profile(game);
        let (save, kept) = (profile.save_mods, profile.values.len());
        let resettable = if running { sheet.active() > 0 } else { kept > 0 };
        let label = match kept {
            0 => t(cx, key::CHEATS_SAVE),
            _ if running => t(cx, key::CHEATS_SAVE),
            kept => {
                let padded = format!("{kept:02}");
                tf(cx, key::CHEATS_SAVED, &[("saved", Arg::Str(&padded)), ("count", Arg::Num(kept as f64))])
            }
        };
        let save_tip = t(cx, key::CHEATS_SAVE_TIP);
        h_flex()
            .flex_none()
            .gap(px(10.))
            .child(div().w(px(1.)).h(px(16.)).bg(line(0.12)))
            .child(
                h_flex()
                    .id("save-mods")
                    .gap(px(8.))
                    .cursor_pointer()
                    .tooltip(move |window, cx| tip(save_tip.clone(), window, cx))
                    .on_click(cx.listener(move |list, _, _, cx| {
                        let Some(game) = list.selected(cx) else { return };
                        list.trainers.update(cx, |trainers, cx| trainers.set_save_mods(&game, !save, cx));
                    }))
                    .child(mono(label, 10.5).text_color(rgb(if save { MUTED } else { GHOST })))
                    .child(toggle(save, cx)),
            )
            .child(
                button(&t(cx, key::CHEATS_RESET))
                    .id("reset-mods")
                    .h(px(24.))
                    .when(!resettable, |button| button.opacity(0.4).cursor_default())
                    .when(resettable, |button| button.on_click(cx.listener(|list, _, _, cx| list.reset_mods(cx)))),
            )
    }
}

fn value_box(value: impl Into<SharedString>) -> Div {
    mono(value, 12.5).h(px(26.)).flex().items_center().border_1().border_color(line(0.14)).bg(rgb(SUNKEN))
}
