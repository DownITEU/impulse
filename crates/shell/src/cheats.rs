use gpui_kit::{
    AnyElement, Context, FontWeight, IntoElement, SharedString,
    component::{h_flex, v_flex},
    div,
    prelude::*,
    px, rgb,
};

use super::{
    Load, Page,
    assets::Icon,
    library::{Game, Support},
    loaded::Loaded,
    theme::*,
    widgets::{filter, fitted, icon, icon_button, mono, notice, tip, tracked},
};

const PINNED: &str = "PINNED";
const KEPT: &str = "It is kept on disk after the first download.";

impl Page {
    pub(super) fn cheats(&self, game: &Game, cx: &mut Context<Self>) -> impl IntoElement {
        let running = self.is_running();
        let column = v_flex().flex_1().min_w_0().gap(px(12.));
        if game.support == Support::NoTrainer {
            return column.child(notice(
                "[ NO TRAINER ]",
                &format!("NO TRAINER FOR {} YET", game.name),
                "There is no trainer for this game yet. Impulse will flag it in your library as soon as one ships.",
            ));
        }
        let loaded = match self.trainers.get(&game.id) {
            Some(Load::Ready(loaded)) => loaded,
            Some(Load::Failed(why)) => {
                return column.child(notice(
                    "[ NO DATA ]",
                    "THE CHEAT LIST IS NOT DOWNLOADED",
                    &format!("{why}. Impulse tries again when you open the game."),
                ));
            }
            // The disk answers within a frame or two: a card would only flash.
            None if self.loading.contains(&game.id) => return column,
            _ => {
                return column.child(notice("[ LOADING ]", "LOADING THE CHEAT LIST", KEPT));
            }
        };
        column
            .when(game.support == Support::UpdatePending, |column| column.child(update_callout()))
            .when(!running, |column| column.child(self.run_note(&game.id)))
            .child(self.cheat_list(loaded, running, cx))
    }

    fn cheat_list(&self, loaded: &Loaded, running: bool, cx: &mut Context<Self>) -> impl IntoElement {
        let query = self.mod_filter.read(cx).value().to_lowercase();
        // Pinned first, then categories in the trainer's own order.
        let mut sections: Vec<(SharedString, Vec<usize>)> = vec![(PINNED.into(), Vec::new())];
        for (index, row) in loaded.rows.iter().enumerate() {
            let at = match loaded.pinned.contains(&index) {
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
                .filter(|(_, index)| loaded.rows[*index].search.contains(&query))
                .collect();
            if cheats.is_empty() {
                continue;
            }
            shown += cheats.len();
            let folded = loaded.folded.contains(&title);
            let divided = !rows.is_empty();
            rows.push(self.section(at, title, cheats.len(), folded, divided, cx).into_any_element());
            if !folded {
                rows.extend(
                    cheats.into_iter().map(|(number, index)| self.cheat_row(number, index, loaded, running, cx).into_any_element()),
                );
            }
        }
        v_flex()
            .border_1()
            .border_color(line(0.09))
            .bg(alpha(BG, 0.78))
            .child(self.collections_bar(loaded, running, cx))
            .child(
                filter(&self.mod_filter, self.accent)
                    .h(px(38.))
                    .pl(px(14.))
                    .pr(px(8.))
                    .gap(px(10.))
                    .border_b_1()
                    .border_color(line(0.09))
                    .child(mono(format!("{shown:02} MODS"), 10.5).flex_none().text_color(rgb(GHOST)))
                    .child(self.saved_bar(loaded, running, cx)),
            )
            .children(rows)
            .when(shown == 0, |list| list.child(mono("NO MODS MATCH THE FILTER", 11.).px(px(14.)).py(px(16.)).text_color(rgb(FAINT))))
    }

    fn toggle_fold(&mut self, title: SharedString, cx: &mut Context<Self>) {
        let Some(game) = self.selected.clone() else { return };
        let Some(loaded) = self.loaded_mut() else { return };
        if !loaded.folded.remove(&title) {
            loaded.folded.insert(title);
        }
        let folded = loaded.folded.iter().map(SharedString::to_string).collect();
        self.profiles.edit(&game, |profile| profile.folded = folded);
        cx.notify();
    }

    fn toggle_pin(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(game) = self.selected.clone() else { return };
        let Some(loaded) = self.loaded_mut() else { return };
        if !loaded.pinned.remove(&index) {
            loaded.pinned.insert(index);
        }
        let pinned = loaded.pinned.iter().map(|&index| loaded.cheats[index].id.clone()).collect();
        self.profiles.edit(&game, |profile| profile.pinned = Some(pinned));
        cx.notify();
    }

    fn section(
        &self,
        at: usize,
        title: SharedString,
        count: usize,
        folded: bool,
        divided: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let chevron = if folded { Icon::ChevronRight } else { Icon::ChevronDown };
        h_flex()
            .h(px(36.))
            .flex_none()
            .px(px(14.))
            .gap(px(10.))
            .border_b_1()
            .when(divided, |header| header.border_t_1())
            .border_color(line(0.08))
            .when(at == 0, |header| header.child(icon(Icon::Pin, 13., rgb(self.accent))))
            .child(tracked(&title, 13., 0.16).font_weight(FontWeight::BOLD))
            .child(mono(format!("{count:02}"), 11.).text_color(rgb(FAINT)))
            .child(div().flex_1())
            .child(
                icon_button(("fold", at), chevron, 26., 13.)
                    .on_click(cx.listener(move |page, _, _, cx| page.toggle_fold(title.clone(), cx))),
            )
    }

    fn cheat_row(&self, number: usize, index: usize, loaded: &Loaded, running: bool, cx: &mut Context<Self>) -> impl IntoElement {
        let accent = rgb(self.accent);
        let row = &loaded.rows[index];
        let live = running && loaded.is_live(index);
        let pinned = loaded.pinned.contains(&index);
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
            .child(self.caps(index, loaded, cx))
            .child(
                h_flex()
                    .w(px(loaded.control_width))
                    .flex_none()
                    .gap(px(8.))
                    .opacity(if running { 1. } else { 0.4 })
                    .child(self.control(index, loaded, running, live, cx)),
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
