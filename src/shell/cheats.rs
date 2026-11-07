use gpui_kit::{
    AnyElement, Context, Div, FontWeight, IntoElement,
    component::{h_flex, v_flex},
    div,
    prelude::*,
    px, rgb,
};

use super::{
    Page,
    assets::Icon,
    mock::{CATEGORIES, Control, Game, MODS, Mod, Trainer},
    theme::*,
    widgets::{filter, icon, icon_button, keycap, mono, toggle, tracked},
};

const PINNED: &str = "PINNED";

impl Page {
    pub(super) fn is_live(&self, cheat: &Mod) -> bool {
        match cheat.control {
            Control::Toggle => self.enabled.contains(cheat.id),
            Control::Slider { percent, .. } => percent > 0,
            Control::Incremental => self.crit != 0,
            _ => false,
        }
    }

    pub(super) fn cheats(&self, game: &Game, cx: &mut Context<Self>) -> impl IntoElement {
        let running = self.running == Some(self.selected);
        let column = v_flex().flex_1().min_w_0().gap(px(12.));
        if game.trainer == Trainer::None {
            return column.child(no_trainer(game));
        }
        column
            .when(game.trainer == Trainer::UpdatePending, |column| {
                column.child(update_callout())
            })
            .when(!running, |column| {
                column.child(
                    mono("TRAINER IDLE · CONTROLS UNLOCK AFTER LAUNCH", 11.).text_color(rgb(DIM)),
                )
            })
            .child(self.cheat_list(running, cx))
    }

    fn cheat_list(&self, running: bool, cx: &mut Context<Self>) -> impl IntoElement {
        let query = self.mod_filter.read(cx).value().to_lowercase();
        let mut number = 0;
        let mut shown = 0;
        let mut rows: Vec<AnyElement> = Vec::new();
        for title in [PINNED].into_iter().chain(CATEGORIES) {
            // Numbers are assigned before filtering and folding, so they stay stable.
            let cheats: Vec<_> = MODS
                .iter()
                .filter(|cheat| match self.pinned.contains(cheat.id) {
                    true => title == PINNED,
                    false => title == cheat.category,
                })
                .map(|cheat| {
                    number += 1;
                    (number, cheat)
                })
                .filter(|(_, cheat)| cheat.name.to_lowercase().contains(&query))
                .collect();
            if cheats.is_empty() {
                continue;
            }
            shown += cheats.len();
            let folded = self.folded.contains(&(self.selected, title));
            rows.push(
                self.section(title, cheats.len(), folded, !rows.is_empty(), cx)
                    .into_any_element(),
            );
            if !folded {
                rows.extend(cheats.into_iter().map(|(number, cheat)| {
                    self.cheat_row(number, cheat, running, cx)
                        .into_any_element()
                }));
            }
        }
        v_flex()
            .border_1()
            .border_color(line(0.09))
            .bg(alpha(BG, 0.78))
            .child(
                filter(&self.mod_filter, self.accent)
                    .h(px(38.))
                    .px(px(14.))
                    .border_b_1()
                    .border_color(line(0.09))
                    .child(mono(format!("{shown:02} MODS"), 10.5).text_color(rgb(GHOST))),
            )
            .children(rows)
    }

    fn section(
        &self,
        title: &'static str,
        count: usize,
        folded: bool,
        divided: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let chevron = if folded {
            Icon::ChevronRight
        } else {
            Icon::ChevronDown
        };
        h_flex()
            .h(px(36.))
            .flex_none()
            .px(px(14.))
            .gap(px(10.))
            .border_b_1()
            .when(divided, |header| header.border_t_1())
            .border_color(line(0.08))
            .when(title == PINNED, |header| {
                header.child(icon(Icon::Pin, 13., rgb(self.accent)))
            })
            .child(tracked(title, 13., 0.16).font_weight(FontWeight::BOLD))
            .child(mono(format!("{count:02}"), 11.).text_color(rgb(FAINT)))
            .child(div().flex_1())
            .child(icon_button(title, chevron, 26., 13.).on_click(cx.listener(
                move |page, _, _, cx| {
                    let section = (page.selected, title);
                    if !page.folded.remove(&section) {
                        page.folded.insert(section);
                    }
                    cx.notify();
                },
            )))
    }

    fn cheat_row(
        &self,
        number: usize,
        cheat: &'static Mod,
        running: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let accent = rgb(self.accent);
        let live = running && self.is_live(cheat);
        let pinned = self.pinned.contains(cheat.id);
        let name_color = match (live, running) {
            (true, _) => TEXT_BRIGHT,
            (false, true) => TEXT_2,
            (false, false) => DIM,
        };
        let pin = ("pin", number);
        h_flex()
            .h(px(42.))
            .flex_none()
            .px(px(14.))
            .gap(px(12.))
            .border_b_1()
            .border_color(line(0.045))
            .map(|row| match live {
                true => row.bg(alpha(self.accent, 0.05)),
                false => row.hover(|style| style.bg(rgb(ROW_HOVER))),
            })
            .child(
                h_flex()
                    .w(px(48.))
                    .flex_none()
                    .gap(px(8.))
                    .child(div().size(px(7.)).border_1().map(|dot| match live {
                        true => dot.bg(accent).border_color(accent),
                        false => dot.border_color(line(0.25)),
                    }))
                    .child(mono(format!("{number:02}"), 11.5).text_color(if live {
                        accent
                    } else {
                        rgb(FAINT)
                    })),
            )
            .child(
                h_flex()
                    .flex_1()
                    .min_w_0()
                    .gap(px(8.))
                    .child(
                        div()
                            .min_w_0()
                            .truncate()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_size(px(13.5))
                            .text_color(rgb(name_color))
                            .child(cheat.name.to_uppercase()),
                    )
                    .when(cheat.instructions, |name| {
                        name.child(icon(Icon::Info, 13., rgb(FAINT)))
                    })
                    .child(
                        div()
                            .id(pin)
                            .group("pin")
                            .size(px(22.))
                            .flex_none()
                            .flex()
                            .items_center()
                            .justify_center()
                            .cursor_pointer()
                            .on_click(cx.listener(move |page, _, _, cx| {
                                if !page.pinned.remove(cheat.id) {
                                    page.pinned.insert(cheat.id);
                                }
                                cx.notify();
                            }))
                            .child(match pinned {
                                true => icon(Icon::Pin, 12., accent),
                                false => icon(Icon::Pin, 12., rgb(GHOST))
                                    .group_hover("pin", |style| style.text_color(rgb(TEXT))),
                            }),
                    ),
            )
            .child(
                h_flex()
                    .w(px(196.))
                    .flex_none()
                    .justify_end()
                    .gap(px(4.))
                    .children(cheat.keys.iter().copied().map(keycap)),
            )
            .child(
                h_flex()
                    .w(px(186.))
                    .flex_none()
                    .justify_end()
                    .gap(px(8.))
                    .opacity(if running { 1. } else { 0.4 })
                    .child(self.control(number, cheat, running, live, cx)),
            )
    }

    fn control(
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
            Control::Slider { value, percent } => {
                let percent = if running { percent } else { 0 };
                let filled = usize::from(percent).div_ceil(10) + 1;
                let lit = if live { self.accent } else { TICK_IDLE };
                h_flex()
                    .gap(px(8.))
                    .child(h_flex().gap(px(3.)).children((0..10).map(|tick| {
                        div()
                            .w(px(6.))
                            .h(px(15.))
                            .bg(rgb(if tick < filled { lit } else { TICK }))
                    })))
                    .child(
                        mono(if running { value } else { "1.0×" }, 12.5)
                            .w(px(38.))
                            .flex()
                            .justify_end()
                            .text_color(value_color),
                    )
                    .into_any_element()
            }
            Control::Incremental => {
                let step = |id: &'static str, label: &'static str, delta: i32| {
                    small_button(label)
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
            Control::Number(value) => h_flex()
                .gap(px(8.))
                .child(value_box(value).w(px(72.)).px(px(8.)).justify_end())
                .child(small_button("SET"))
                .into_any_element(),
            Control::Button => small_button("EXECUTE").px(px(14.)).into_any_element(),
            Control::Selection(value) => value_box(value)
                .w(px(112.))
                .pl(px(10.))
                .pr(px(8.))
                .justify_between()
                .border_color(line(0.2))
                .text_size(px(12.))
                .child(icon(Icon::ChevronDown, 11., rgb(DIM)))
                .into_any_element(),
        }
    }
}

fn small_button(label: &'static str) -> Div {
    h_flex()
        .h(px(26.))
        .px(px(12.))
        .justify_center()
        .border_1()
        .border_color(line(0.2))
        .bg(rgb(RAISED))
        .font_weight(FontWeight::BOLD)
        .cursor_pointer()
        .hover(|style| style.bg(rgb(RAISED_HOVER)))
        .child(tracked(label, 11., 0.12))
}

fn value_box(value: impl Into<gpui_kit::SharedString>) -> Div {
    mono(value, 12.5)
        .h(px(26.))
        .flex()
        .items_center()
        .border_1()
        .border_color(line(0.14))
        .bg(rgb(SUNKEN))
}

fn update_callout() -> impl IntoElement {
    h_flex()
        .items_start()
        .gap(px(12.))
        .px(px(14.))
        .py(px(12.))
        .border_1()
        .border_color(alpha(WARN, 0.45))
        .bg(alpha(WARN, 0.06))
        .child(icon(Icon::Warn, 18., rgb(WARN)))
        .child(
            v_flex()
                .gap(px(3.))
                .child(
                    tracked("GAME UPDATED", 12.5, 0.14)
                        .font_weight(FontWeight::BOLD)
                        .text_color(rgb(WARN)),
                )
                .child(div().text_size(px(13.5)).text_color(rgb(TEXT_2)).child(
                    "This build is newer than the trainer supports. Mods may not work until Wand ships an update.",
                )),
        )
}

fn no_trainer(game: &Game) -> impl IntoElement {
    v_flex()
        .items_center()
        .gap(px(14.))
        .px(px(24.))
        .py(px(56.))
        .border_1()
        .border_dashed()
        .border_color(line(0.2))
        .bg(alpha(BG, 0.7))
        .child(mono("[ NO TRAINER ]", 11.).text_color(rgb(DIM)))
        .child(
            tracked(
                &format!("NO TRAINER FOR {} YET", game.name.to_uppercase()),
                22.,
                0.1,
            )
            .justify_center()
            .font_weight(FontWeight::BOLD),
        )
        .child(
            div()
                .max_w(px(420.))
                .text_center()
                .text_color(rgb(MUTED))
                .child("Wand has no trainer for this game. Impulse will flag it in your library as soon as one ships."),
        )
}
