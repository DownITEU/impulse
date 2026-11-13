//! A run's bookkeeping: its count and time, the mods it leaves on, kept for the next run when the game
//! says so, and their reset.

use std::mem;

use gpui_kit::{Context, IntoElement, Window, component::h_flex, div, prelude::*, px, rgb};

use super::{
    Load, Page,
    loaded::Loaded,
    profile::Values,
    run::Play,
    theme::*,
    widgets::{button, mono, tip, toggle},
};

impl Page {
    /// Sets the live trainer's cheats to `values`, as moving each control would.
    pub(super) fn apply(&mut self, values: &Values, window: &mut Window, cx: &mut Context<Self>) {
        let Some(game) = self.play.live().cloned() else { return };
        let Some(Load::Ready(loaded)) = self.trainers.get_mut(&game) else { return };
        let changes = loaded.changes(values);
        for &(index, value) in &changes {
            loaded.put(index, value, window, cx);
        }
        for (index, value) in changes {
            self.push_value(index, value, cx);
        }
        cx.notify();
    }

    /// Counts the run that went live and sets the mods kept for it.
    pub(super) fn open_session(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(game) = self.play.live().cloned() else { return };
        let kept = self.profiles.edit(&game, |profile| {
            profile.sessions += 1;
            profile.save_mods.then(|| profile.values.clone())
        });
        if let Some(values) = kept {
            self.apply(&values, window, cx);
        }
    }

    /// Keeps the live run's time and what it leaves on, once: a run ends in several ways, and the
    /// window closing or the app quitting may come after any of them.
    pub(super) fn close_session(&mut self) {
        let Play::Live { game, since, closed, .. } = &mut self.play else { return };
        if mem::replace(closed, true) {
            return;
        }
        let (game, secs) = (game.clone(), since.elapsed().as_secs());
        let left = self.trainer(Some(&game)).map(Loaded::live_values);
        self.profiles.edit(&game, |profile| {
            profile.played += secs;
            profile.last = secs;
            if let Some(left) = left.filter(|_| profile.save_mods) {
                profile.values = left;
            }
        });
    }

    /// Turns every live mod off; with no run, forgets the kept ones.
    fn reset_mods(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(game) = self.selected.clone() else { return };
        if !self.is_running() {
            self.profiles.edit(&game, |profile| profile.values.clear());
            return cx.notify();
        }
        if let Some(rest) = self.trainer(Some(&game)).map(|loaded| loaded.rest(&loaded.live_values())) {
            self.apply(&rest, window, cx);
        }
    }

    /// The right of the filter row: the game's save switch and the reset.
    pub(super) fn saved_bar(&self, loaded: &Loaded, running: bool, cx: &mut Context<Self>) -> impl IntoElement {
        let profile = self.profiles.of(self.selected.as_deref().unwrap_or_default());
        let (save, kept) = (profile.save_mods, profile.values.len());
        let resettable = if running { loaded.active() > 0 } else { kept > 0 };
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
                        let Some(game) = page.selected.clone() else { return };
                        page.profiles.edit(&game, |profile| {
                            profile.save_mods = !save;
                            profile.values.clear();
                        });
                        cx.notify();
                    }))
                    .child(mono(label, 10.5).text_color(rgb(if save { MUTED } else { GHOST })))
                    .child(toggle(save, cx)),
            )
            .child(
                button("RESET")
                    .id("reset-mods")
                    .h(px(24.))
                    .when(!resettable, |button| button.opacity(0.4).cursor_default())
                    .when(resettable, |button| button.on_click(cx.listener(|page, _, window, cx| page.reset_mods(window, cx)))),
            )
    }
}
