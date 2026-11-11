//! The Settings `OVERLAY` tab and the overlay hotkey's rebinding.

use gpui_kit::{App, Context, IntoElement, SharedString, component::v_flex, prelude::*, px, rgb};
use hotkeys::{Chord, Outcome};
use overlay::Key;

use super::super::{
    Load, Page,
    bindings::{PRESS_KEYS, Target},
    settings::row,
    theme::*,
    widgets::{choice, keycap, segmented},
};

impl Page {
    pub(in super::super) fn overlay_settings(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let held = self.bindings.overlay_held();
        let text: SharedString = match held {
            Some(held) => held.map_or_else(|| PRESS_KEYS.into(), |chord| chord.to_string().into()),
            None => self.overlay_hotkey(cx),
        };
        let accent = rgb(self.accent);
        v_flex()
            .flex_1()
            .px(px(18.))
            .py(px(6.))
            .child(
                row("MODE", "in-game, drawn inside the game: later", 64.).child(
                    segmented()
                        .h(px(26.))
                        .child(choice("overlay-over", "OVER THE GAME", true, self.accent))
                        .child(choice("overlay-in-game", "IN-GAME", false, self.accent).cursor_default().opacity(0.4)),
                ),
            )
            .child(
                row("HOTKEY", "modifiers and one key, free in other apps and mods", 64.).border_b_0().child(
                    keycap("overlay-hotkey", text)
                        // A long preview while binding truncates instead of covering the label.
                        .max_w(px(220.))
                        .text_size(px(12.))
                        .px(px(10.))
                        .py(px(5.))
                        .cursor_pointer()
                        .hover(|style| style.border_color(line(0.3)))
                        .when(held.is_some(), |cap| {
                            cap.border_color(accent)
                                .text_color(accent)
                                .on_mouse_down_out(cx.listener(|page, _, _, cx| page.cancel_capture(cx)))
                        })
                        .on_click(cx.listener(|page, _, window, cx| page.start_capture(Target::Overlay, window, cx))),
                ),
            )
    }

    pub(in super::super) fn overlay_hotkey(&self, cx: &App) -> SharedString {
        let vks: Vec<u16> = self.layer.read(cx).hotkey().vks().map(u16::from).collect();
        Chord::from_vks(&vks).map_or_else(|| "-".into(), |chord| chord.to_string().into())
    }

    /// A refused chord keeps the old hotkey; the tab's hint says what is allowed.
    pub(in super::super) fn rebind_overlay(&mut self, outcome: Outcome, cx: &mut Context<Self>) {
        let key = match outcome {
            Outcome::Bound(chord) => Key::from_vks(chord.vks()),
            Outcome::Reset => Some(Key::DEFAULT),
            _ => None,
        };
        if let Some(key) = key.filter(|&key| !self.cheats_hold(key))
            && let Err(e) = self.layer.update(cx, |layer, cx| layer.set_hotkey(key, cx))
        {
            log::warn!("overlay hotkey not changed: {e}");
        }
    }

    fn cheats_hold(&self, key: Key) -> bool {
        self.trainers.values().any(|load| match load {
            Load::Ready(loaded) => loaded.hotkeys.iter().flatten().flatten().any(|bound| key.fired_by(bound.vks())),
            _ => false,
        })
    }
}
