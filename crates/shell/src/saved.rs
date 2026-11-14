//! A run's bookkeeping: its count and time, the mods it leaves on, kept for the next run when the game
//! says so, and their reset.

use std::mem;

use client::{profile::Values, trainer::Sheet};
use gpui_kit::{App, Context, IntoElement, component::h_flex, div, prelude::*, px, rgb};

use super::{
    Page,
    run::Play,
    theme::*,
    widgets::{button, mono, tip, toggle},
};

impl Page {
    /// Sets the live trainer's cheats to `values`, as moving each control would.
    pub(super) fn apply(&mut self, values: &Values, cx: &mut Context<Self>) {
        let Some(game) = self.play.live().cloned() else { return };
        let Some(changes) = self.sheet(&game, cx).map(|sheet| sheet.changes(values)) else { return };
        for (index, value) in changes {
            self.trainers.update(cx, |trainers, cx| trainers.set(&game, index, value, cx));
            self.push_value(index, value, cx);
        }
    }

    /// Counts the run that went live and sets the mods kept for it.
    pub(super) fn open_session(&mut self, cx: &mut Context<Self>) {
        let Some(game) = self.play.live().cloned() else { return };
        let kept = self.trainers.update(cx, |trainers, cx| trainers.run_opened(&game, cx));
        if let Some(values) = kept {
            self.apply(&values, cx);
        }
    }

    /// Keeps the live run's time and what it leaves on, once: a run ends in several ways, and the
    /// window closing or the app quitting may come after any of them.
    pub(super) fn close_session(&mut self, cx: &mut App) {
        let Play::Live { game, since, closed, .. } = &mut self.play else { return };
        if mem::replace(closed, true) {
            return;
        }
        let (game, secs) = (game.clone(), since.elapsed().as_secs());
        self.trainers.update(cx, |trainers, cx| trainers.run_closed(&game, secs, cx));
    }

    /// Turns every live mod off; with no run, forgets the kept ones.
    fn reset_mods(&mut self, cx: &mut Context<Self>) {
        let Some(game) = self.selected(cx) else { return };
        if !self.is_running(cx) {
            return self.trainers.update(cx, |trainers, cx| trainers.forget_kept(&game, cx));
        }
        if let Some(rest) = self.sheet(&game, cx).map(|sheet| sheet.rest(&sheet.live_values())) {
            self.apply(&rest, cx);
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
