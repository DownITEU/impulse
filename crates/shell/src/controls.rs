//! The control at the right of a cheat row, one per cheat kind.

use gpui_kit::{
    AnyElement, Context, Div, IntoElement, MouseButton, MouseMoveEvent, ScrollEvent, SharedString,
    Stateful, Window,
    component::{h_flex, input::Input, v_flex},
    deferred, div,
    prelude::*,
    px, relative, rgb,
};
use model::{Args, Kind};

use super::{
    Flash, Load, Menu, Page,
    assets::Icon,
    loaded::{Loaded, number},
    theme::*,
    widgets::{button, flash_button, icon, mono, toggle},
};

const TOP_TICK: u8 = 9;

/// A slider's bounds; trainers may leave any of them out.
#[derive(Clone, Copy)]
struct Span {
    min: f64,
    max: f64,
    step: f64,
}

impl Span {
    fn of(args: &Args) -> Self {
        let min = args.min.unwrap_or(0.);
        Self {
            min,
            max: args.max.filter(|max| *max > min).unwrap_or(min + 100.),
            step: args.step.filter(|step| *step > 0.).unwrap_or(1.),
        }
    }

    fn tick(self, value: f64) -> u8 {
        let share = (value - self.min) / (self.max - self.min);
        (share * f64::from(TOP_TICK)).round() as u8
    }

    fn value(self, tick: u8) -> f64 {
        self.snap(self.min + (self.max - self.min) * f64::from(tick) / f64::from(TOP_TICK))
    }

    /// A wheel notch: at least one step, so a coarse step never rounds back to where it was.
    fn nudge(self, value: f64, up: bool) -> f64 {
        let by = self.step.max((self.max - self.min) / f64::from(TOP_TICK));
        self.snap(if up { value + by } else { value - by })
    }

    /// Steps count from `min`, not from zero.
    fn snap(self, value: f64) -> f64 {
        (self.min + ((value - self.min) / self.step).round() * self.step).clamp(self.min, self.max)
    }
}

/// Rounds to the decimals of the step and of `min`, so repeated steps do not drift (Wand rounds
/// to the step's alone, which turns 0.5 + 1 into 2).
fn round_to(value: f64, args: &Args, step: f64) -> f64 {
    let decimals = |x: f64| x.to_string().split_once('.').map_or(0, |(_, fraction)| fraction.len());
    let scale = 10_f64.powi(decimals(step).max(args.min.map_or(0, decimals)) as i32);
    let rounded = (value * scale).round() / scale;
    // Far past f64's range the scale overflows: the step is then too coarse to drift.
    if rounded.is_finite() { rounded } else { value }
}

impl Page {
    /// A hotkey slot of the live trainer's cheat, stepped as Wand steps it: a pair moves by the step
    /// and stops at the ends, a selection wraps, a numeric field takes the new value too.
    pub(super) fn hotkey(&mut self, index: usize, slot: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(game) = self.play.live().cloned() else { return };
        let Some(Load::Ready(loaded)) = self.trainers.get_mut(&game) else { return };
        let (cheat, value, up) = (&loaded.cheats[index], loaded.values[index], slot == 1);
        let next = match cheat.kind {
            Kind::Toggle => 1. - value.min(1.),
            Kind::Button => {
                let pressed = cheat.args.pressed;
                self.push_value(index, pressed, cx);
                if self.selected.as_ref() == Some(&game) {
                    self.flash(Flash::Cheat(index), cx);
                }
                return;
            }
            Kind::Number | Kind::Slider => {
                let step = cheat.args.step.filter(|step| *step > 0.).unwrap_or(1.);
                cheat.args.clamp(round_to(if up { value + step } else { value - step }, &cheat.args, step))
            }
            Kind::Selection => {
                let count = loaded.rows[index].options.len() as f64;
                if count == 0. {
                    return;
                }
                (value + if up { 1. } else { -1. }).rem_euclid(count)
            }
            Kind::Unknown => return,
        };
        if next == value {
            return;
        }
        loaded.values[index] = next;
        if let Some(input) = loaded.inputs.get(&index) {
            input.update(cx, |input, cx| input.set_value(number(next), window, cx));
        }
        self.push_value(index, next, cx);
        cx.notify();
    }

    fn set_value(&mut self, index: usize, value: f64, cx: &mut Context<Self>) {
        if let Some(loaded) = self.loaded_mut() {
            loaded.values[index] = value;
        }
        self.push_value(index, value, cx);
        cx.notify();
    }

    pub(super) fn control(
        &self,
        index: usize,
        loaded: &Loaded,
        running: bool,
        live: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let cheat = &loaded.cheats[index];
        let value = loaded.values[index];
        let value_color = if live { rgb(self.accent) } else { rgb(DIM) };
        match cheat.kind {
            Kind::Toggle => div()
                .id(("toggle", index))
                .cursor_pointer()
                .on_click(cx.listener(move |page, _, _, cx| {
                    if running {
                        page.set_value(index, if value == 0. { 1. } else { 0. }, cx);
                    }
                }))
                .child(toggle(live, self.accent))
                .into_any_element(),
            Kind::Slider => {
                let span = Span::of(&cheat.args);
                h_flex()
                    .gap(px(5.))
                    .child(self.ticks(index, span, value, running, live, cx))
                    .child(
                        mono(number(value), 12.5)
                            .w(px(38.))
                            .flex()
                            .justify_end()
                            .text_color(value_color),
                    )
                    .into_any_element()
            }
            Kind::Number if cheat.args.button => h_flex()
                .gap(px(8.))
                .when_some(loaded.inputs.get(&index), |control, input| {
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
                    button(label)
                        .id((id, index))
                        .w(px(26.))
                        .px_0()
                        .font_family(MONO)
                        .text_size(px(14.))
                        .on_click(cx.listener(move |page, _, _, cx| {
                            if running && let Some(loaded) = page.loaded_mut() {
                                let next = loaded.cheats[index].args.clamp(value + by);
                                page.set_value(index, next, cx);
                            }
                        }))
                };
                h_flex()
                    .gap(px(8.))
                    .child(nudge("decrease", "−", -step))
                    .child(
                        value_box(number(value))
                            .w(px(52.))
                            .justify_center()
                            .text_color(value_color),
                    )
                    .child(nudge("increase", "+", step))
                    .into_any_element()
            }
            Kind::Button => self
                .press(index, "EXECUTE", "APPLIED", running, cx)
                .px(px(14.))
                .into_any_element(),
            Kind::Selection => {
                let options = loaded.rows[index].options.clone();
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
                self.dropdown(
                    trigger,
                    Menu::Cheat(index),
                    options,
                    current,
                    cx,
                    move |page, option, _, cx| page.set_value(index, option as f64, cx),
                )
                .into_any_element()
            }
            Kind::Unknown => div().into_any_element(),
        }
    }

    /// A button with no state of its own: it confirms the press for a moment.
    fn press(
        &self,
        index: usize,
        label: &str,
        done: &str,
        running: bool,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        flash_button(
            label,
            done,
            self.is_flashed(Flash::Cheat(index)),
            self.accent,
        )
        .id(("press", index))
        .on_click(cx.listener(move |page, _, _, cx| {
            if !running {
                return;
            }
            // A cheat with a text field takes its value from it; a button cheat sends its own and
            // keeps no state.
            let (typed, fired) = page.trainer(page.selected.as_ref()).map_or((None, None), |loaded| {
                let cheat = &loaded.cheats[index];
                let typed = loaded.inputs.get(&index).and_then(|input| Some(cheat.args.clamp(input.read(cx).value().parse().ok()?)));
                (typed, (cheat.kind == Kind::Button).then_some(cheat.args.pressed))
            });
            if let Some(value) = typed {
                page.set_value(index, value, cx);
            }
            if let Some(value) = fired {
                page.push_value(index, value, cx);
            }
            page.flash(Flash::Cheat(index), cx);
        }))
    }

    /// Ten ticks: click or drag across them, or use the wheel.
    fn ticks(
        &self,
        index: usize,
        span: Span,
        value: f64,
        running: bool,
        live: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let lit = if live { self.accent } else { TICK_IDLE };
        let level = span.tick(value);
        h_flex()
            .when(running, |ticks| {
                ticks.cursor_pointer().on_scroll(cx.listener(
                    move |page, event: &ScrollEvent, window, cx| {
                        let up = event.delta.pixel_delta(window.line_height()).y > px(0.);
                        page.set_value(index, span.nudge(value, up), cx);
                        // Keeps the pane from scrolling under the slider.
                        cx.stop_propagation();
                    },
                ))
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
                        .on_mouse_move(cx.listener(
                            move |page, event: &MouseMoveEvent, _, cx| {
                                if event.pressed_button == Some(MouseButton::Left) {
                                    page.set_value(index, span.value(tick), cx);
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
        menu: Menu,
        options: Vec<SharedString>,
        current: usize,
        cx: &mut Context<Self>,
        pick: impl Fn(&mut Page, usize, &mut Window, &mut Context<Page>) + Copy + 'static,
    ) -> Stateful<Div> {
        let accent = rgb(self.accent);
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
                        .children(options.into_iter().enumerate().map(|(at, option)| {
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

#[cfg(test)]
mod tests {
    use model::Args;

    use super::{Span, round_to};

    #[test]
    fn hotkey_steps_keep_the_origin_and_do_not_drift() {
        let half = Args { min: Some(0.5), ..Args::default() };
        assert_eq!(round_to(0.5 + 1., &half, 1.), 1.5);
        assert_eq!(round_to(0.2 + 0.1, &Args::default(), 0.1), 0.3);
    }

    #[test]
    fn steps_count_from_min() {
        let span = Span {
            min: 3.,
            max: 21.,
            step: 2.,
        };
        assert_eq!(span.value(0), 3.);
        assert_eq!(span.value(super::TOP_TICK), 21.);
        assert!((0..=super::TOP_TICK).all(|tick| (span.value(tick) - 3.) % 2. == 0.));
    }

    #[test]
    fn wheel_moves_a_coarse_step() {
        let span = Span {
            min: 0.,
            max: 10.,
            step: 5.,
        };
        assert_eq!(span.nudge(0., true), 5.);
        assert_eq!(span.nudge(5., true), 10.);
        assert_eq!(span.nudge(10., true), 10.);
        assert_eq!(span.nudge(5., false), 0.);
    }
}
