use client::{
    Load,
    library::{Game, Support},
    trainer::Sheet,
};
use gpui_kit::{
    AnyElement, Context, Entity, FontWeight, IntoElement, SharedString, Window,
    component::{h_flex, v_flex},
    div,
    prelude::*,
    px, rgb,
};

use super::{
    Page,
    assets::Icon,
    theme::*,
    trainer::{Printed, TrainerEvent, Trainers},
    widgets::{filter, fitted, icon, icon_button, mono, notice, tip, tracked},
};

const PINNED: &str = "PINNED";
const KEPT: &str = "It is kept on disk after the first download.";

impl Page {
    pub(super) fn cheats(&self, game: &Game, cx: &Context<Self>) -> impl IntoElement {
        let running = self.is_running(cx);
        let column = v_flex().flex_1().min_w_0().gap(px(12.));
        if game.support == Support::NoTrainer {
            return column.child(notice(
                "[ NO TRAINER ]",
                &format!("NO TRAINER FOR {} YET", game.name),
                "There is no trainer for this game yet. Impulse will flag it in your library as soon as one ships.",
            ));
        }
        let trainers = self.trainers.read(cx);
        let sheet = match trainers.get(&game.id) {
            Some(Load::Ready(sheet)) => sheet,
            Some(Load::Failed(why)) => {
                return column.child(notice(
                    "[ NO DATA ]",
                    "THE CHEAT LIST IS NOT DOWNLOADED",
                    &format!("{why}. Impulse tries again when you open the game."),
                ));
            }
            // The disk answers within a frame or two: a card would only flash.
            None if trainers.is_loading(&game.id) => return column,
            _ => {
                return column.child(notice("[ LOADING ]", "LOADING THE CHEAT LIST", KEPT));
            }
        };
        // Printed as the sheet shows, before a frame draws.
        let Some(printed) = self.printed.as_ref().filter(|printed| printed.game == game.id) else { return column };
        column
            .when(game.support == Support::UpdatePending, |column| column.child(update_callout()))
            .when(!running, |column| column.child(self.run_note(&game.id, cx)))
            .child(self.cheat_list(sheet, printed, running, cx))
    }

    /// The open game's cheat list as printed, made anew when another game opens or another copy shows.
    pub(super) fn print(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let game = self.selected(cx);
        self.printed = self.trainers.update(cx, |trainers, cx| {
            let game = game?;
            Some(Printed::new(game.clone(), trainers.sheet(&game)?, window, cx))
        });
    }

    pub(super) fn trainer_changed(
        &mut self,
        trainers: &Entity<Trainers>,
        event: &TrainerEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match event {
            TrainerEvent::Installed(game) => {
                // The user's hotkey choices over the provider's.
                let slots = trainers.read(cx).sheet(game).map(|sheet| self.bindings.slots(game, &sheet.cheats));
                if let Some(slots) = slots {
                    trainers.update(cx, |trainers, cx| trainers.bind(game, slots, cx));
                }
                if self.selected(cx).as_ref() == Some(game) {
                    self.drop_cheat_state();
                    // Its cap may be gone, and with it the click that ends the capture.
                    self.bindings.cancel();
                    self.print(window, cx);
                }
            }
            TrainerEvent::Rebound(game) => {
                if let Some(printed) = self.printed.as_mut().filter(|printed| printed.game == *game)
                    && let Some(sheet) = trainers.read(cx).sheet(game)
                {
                    printed.recap(sheet);
                }
            }
            TrainerEvent::Set { game, index, value } => {
                if let Some(printed) = self.printed.as_ref().filter(|printed| printed.game == *game) {
                    printed.put(*index, *value, window, cx);
                }
            }
        }
    }

    fn cheat_list(&self, sheet: &Sheet, printed: &Printed, running: bool, cx: &Context<Self>) -> impl IntoElement {
        let query = self.mod_filter.read(cx).value().to_lowercase();
        // Pinned first, then categories in the trainer's own order.
        let mut sections: Vec<(SharedString, Vec<usize>)> = vec![(PINNED.into(), Vec::new())];
        for (index, row) in printed.rows.iter().enumerate() {
            let at = match sheet.pinned.contains(&index) {
                true => 0,
                false => {
                    sections.iter().skip(1).position(|(title, _)| *title == row.category).map(|found| found + 1).unwrap_or_else(|| {
                        sections.push((row.category.clone(), Vec::new()));
                        sections.len() - 1
                    })
                }
            };
            sections[at].1.push(index);
        }
        let mut number = 0;
        let mut shown = 0;
        let mut rows: Vec<AnyElement> = Vec::new();
        for (at, (title, cheats)) in sections.into_iter().enumerate() {
            // Numbers are assigned before filtering and folding, so they stay stable.
            let cheats: Vec<_> = cheats
                .into_iter()
                .map(|index| {
                    number += 1;
                    (number, index)
                })
                .filter(|(_, index)| printed.rows[*index].search.contains(&query))
                .collect();
            if cheats.is_empty() {
                continue;
            }
            shown += cheats.len();
            let folded = sheet.folded.contains(&*title);
            let divided = !rows.is_empty();
            rows.push(self.section(at, title, cheats.len(), folded, divided, cx).into_any_element());
            if !folded {
                rows.extend(
                    cheats.into_iter().map(|(number, index)| self.cheat_row(number, index, sheet, printed, running, cx).into_any_element()),
                );
            }
        }
        v_flex()
            .border_1()
            .border_color(line(0.09))
            .bg(alpha(BG, 0.78))
            .child(self.collections_bar(sheet, running, cx))
            .child(
                filter(&self.mod_filter, cx)
                    .h(px(38.))
                    .pl(px(14.))
                    .pr(px(8.))
                    .gap(px(10.))
                    .border_b_1()
                    .border_color(line(0.09))
                    .child(mono(format!("{shown:02} MODS"), 10.5).flex_none().text_color(rgb(GHOST)))
                    .child(self.saved_bar(sheet, running, cx)),
            )
            .children(rows)
            .when(shown == 0, |list| list.child(mono("NO MODS MATCH THE FILTER", 11.).px(px(14.)).py(px(16.)).text_color(rgb(FAINT))))
    }

    fn toggle_fold(&mut self, title: SharedString, cx: &mut Context<Self>) {
        let Some(game) = self.selected(cx) else { return };
        self.trainers.update(cx, |trainers, cx| trainers.toggle_fold(&game, &title, cx));
    }

    fn toggle_pin(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(game) = self.selected(cx) else { return };
        self.trainers.update(cx, |trainers, cx| trainers.toggle_pin(&game, index, cx));
    }

    fn section(&self, at: usize, title: SharedString, count: usize, folded: bool, divided: bool, cx: &Context<Self>) -> impl IntoElement {
        let chevron = if folded { Icon::ChevronRight } else { Icon::ChevronDown };
        h_flex()
            .h(px(36.))
            .flex_none()
            .px(px(14.))
            .gap(px(10.))
            .border_b_1()
            .when(divided, |header| header.border_t_1())
            .border_color(line(0.08))
            .when(at == 0, |header| header.child(icon(Icon::Pin, 13., rgb(accent(cx)))))
            .child(tracked(&title, 13., 0.16).font_weight(FontWeight::BOLD))
            .child(mono(format!("{count:02}"), 11.).text_color(rgb(FAINT)))
            .child(div().flex_1())
            .child(
                icon_button(("fold", at), chevron, 26., 13.)
                    .on_click(cx.listener(move |page, _, _, cx| page.toggle_fold(title.clone(), cx))),
            )
    }

    fn cheat_row(
        &self,
        number: usize,
        index: usize,
        sheet: &Sheet,
        printed: &Printed,
        running: bool,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let lit = accent(cx);
        let accent = rgb(lit);
        let row = &printed.rows[index];
        let live = running && sheet.is_live(index);
        let pinned = sheet.pinned.contains(&index);
        let name_color = match (live, running) {
            (true, _) => TEXT_BRIGHT,
            (false, true) => TEXT_2,
            (false, false) => DIM,
        };
        // GPUI repaints a hover style on its own only for an element with an id.
        h_flex()
            .id(("cheat", index))
            .h(px(42.))
            .flex_none()
            .px(px(14.))
            .gap(px(12.))
            .border_b_1()
            .border_color(line(0.045))
            .map(|row| match live {
                true => row.bg(alpha(lit, 0.05)),
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
                    .child(mono(format!("{number:02}"), 11.5).text_color(if live { accent } else { rgb(FAINT) })),
            )
            .child(
                // Shrinks with the caps, in proportion to their widths.
                h_flex()
                    .flex_auto()
                    .min_w_0()
                    .gap(px(8.))
                    .child(
                        div()
                            .min_w_0()
                            .truncate()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_size(px(13.5))
                            .text_color(rgb(name_color))
                            .child(fitted("name", row.name.clone())),
                    )
                    .when_some(row.tip.clone(), |name, text| {
                        name.child(
                            div()
                                .id(("info", index))
                                .child(icon(Icon::Info, 13., rgb(FAINT)))
                                .tooltip(move |window, cx| tip(text.clone(), window, cx)),
                        )
                    })
                    .child(
                        div()
                            .id(("pin", index))
                            .group("pin")
                            .size(px(22.))
                            .flex_none()
                            .flex()
                            .items_center()
                            .justify_center()
                            .cursor_pointer()
                            .on_click(cx.listener(move |page, _, _, cx| page.toggle_pin(index, cx)))
                            .child(
                                icon(Icon::Pin, 12., if pinned { accent } else { rgb(GHOST) })
                                    .id(("pin-icon", index))
                                    .when(!pinned, |pin| pin.group_hover("pin", |style| style.text_color(rgb(TEXT)))),
                            ),
                    ),
            )
            .child(self.caps(index, sheet, printed, cx))
            .child(
                h_flex()
                    .w(px(printed.control_width))
                    .flex_none()
                    .gap(px(8.))
                    .opacity(if running { 1. } else { 0.4 })
                    .child(self.control(index, sheet, printed, running, live, cx)),
            )
    }
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
            v_flex().gap(px(3.)).child(tracked("GAME UPDATED", 12.5, 0.14).font_weight(FontWeight::BOLD).text_color(rgb(WARN))).child(
                div()
                    .text_size(px(13.5))
                    .text_color(rgb(TEXT_2))
                    .child("This build is newer than the trainer supports. Mods may not work until the trainer is updated."),
            ),
        )
}
