//! The saved bar: whether the mods a run leaves on come back at the next one, and their reset.

use client::trainer::Sheet;
use gpui_kit::{Context, IntoElement, component::h_flex, div, prelude::*, px, rgb};

use super::{
    Page,
    theme::*,
    widgets::{button, mono, tip, toggle},
};

impl Page {
    /// Turns every live mod off; with no run, forgets the kept ones.
    fn reset_mods(&mut self, cx: &mut Context<Self>) {
        let Some(game) = self.selected(cx) else { return };
        if !self.is_running(cx) {
            return self.trainers.update(cx, |trainers, cx| trainers.forget_kept(&game, cx));
        }
        if let Some(rest) = self.sheet(&game, cx).map(|sheet| sheet.rest(&sheet.live_values())) {
            self.runs.update(cx, |runs, cx| runs.apply(&rest, cx));
        }
    }

    /// The right of the filter row: the game's save switch and the reset.
    pub(super) fn saved_bar(&self, sheet: &Sheet, running: bool, cx: &Context<Self>) -> impl IntoElement {
        let profile = self.trainers.read(cx).profile(self.selected(cx).as_deref().unwrap_or_default());
        let (save, kept) = (profile.save_mods, profile.values.len());
        let resettable = if running { sheet.active() > 0 } else { kept > 0 };
        let label = match kept {
            0 => "SAVE".to_owned(),
            _ if running => "SAVE".to_owned(),
            kept => format!("SAVED {kept:02}"),
        };
        h_flex()
            .flex_none()
            .gap(px(10.))
            .child(div().w(px(1.)).h(px(16.)).bg(line(0.12)))
            .child(
                h_flex()
                    .id("save-mods")
                    .gap(px(8.))
                    .cursor_pointer()
                    .tooltip(|window, cx| tip("The mods a run leaves on come back on at the next one.".into(), window, cx))
                    .on_click(cx.listener(move |page, _, _, cx| {
                        let Some(game) = page.selected(cx) else { return };
                        page.trainers.update(cx, |trainers, cx| trainers.set_save_mods(&game, !save, cx));
                    }))
                    .child(mono(label, 10.5).text_color(rgb(if save { MUTED } else { GHOST })))
                    .child(toggle(save, cx)),
            )
            .child(
                button("RESET")
                    .id("reset-mods")
                    .h(px(24.))
                    .when(!resettable, |button| button.opacity(0.4).cursor_default())
                    .when(resettable, |button| button.on_click(cx.listener(|page, _, _, cx| page.reset_mods(cx)))),
            )
    }
}
