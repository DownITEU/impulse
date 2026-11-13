//! The Settings `OVERLAY` tab and the overlay's and minimap's hotkeys' rebinding.

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
        let layer = self.layer.read(cx);
        let (overlay, minimap) = (Some(layer.hotkey()), layer.minimap_hotkey());
        v_flex()
            .flex_1()
            .px(px(18.))
            .py(px(6.))
            .child(
                row("MODE", "in-game, drawn inside the game: later", 64.).child(
                    segmented()
                        .h(px(26.))
                        .child(choice("overlay-over", "OVER THE GAME", true, cx))
                        .child(choice("overlay-in-game", "IN-GAME", false, cx).cursor_default().opacity(0.4)),
                ),
            )
            .child(row("HOTKEY", "modifiers and one key, free in other apps and mods", 64.).child(self.hotkey_cap(
                "overlay-hotkey",
                Target::Overlay,
                overlay,
                cx,
            )))
            .child(
                row("MINIMAP HOTKEY", "shows or hides the minimap; held only while the game is in front", 64.)
                    .border_b_0()
                    .child(self.hotkey_cap("minimap-hotkey", Target::Minimap, minimap, cx)),
            )
    }

    fn hotkey_cap(&self, id: &'static str, target: Target, key: Option<Key>, cx: &mut Context<Self>) -> impl IntoElement {
        let held = self.bindings.layer_held(&target);
        let text: SharedString = match held {
            Some(held) => held.map_or_else(|| PRESS_KEYS.into(), |chord| chord.to_string().into()),
            None => key_text(key),
        };
        let accent = rgb(accent(cx));
        keycap(id, text)
            // A long preview while binding truncates instead of covering the label.
            .max_w(px(220.))
            .text_size(px(12.))
            .px(px(10.))
            .py(px(5.))
            .cursor_pointer()
            .hover(|style| style.border_color(line(0.3)))
            .when(held.is_some(), |cap| {
                cap.border_color(accent).text_color(accent).on_mouse_down_out(cx.listener(|page, _, _, cx| page.cancel_capture(cx)))
            })
            .on_click(cx.listener(move |page, _, window, cx| page.start_capture(target.clone(), window, cx)))
    }

    pub(in super::super) fn overlay_hotkey(&self, cx: &App) -> SharedString {
        key_text(Some(self.layer.read(cx).hotkey()))
    }

    /// A refused chord keeps the old hotkey; the tab's hint says what is allowed.
    pub(in super::super) fn rebind_overlay(&mut self, outcome: Outcome, cx: &mut Context<Self>) {
        let key = match outcome {
            Outcome::Bound(chord) => Key::from_vks(chord.vks()),
            Outcome::Reset => Some(Key::DEFAULT),
            _ => None,
        };
        let minimap = self.layer.read(cx).minimap_hotkey();
        if let Some(key) = key.filter(|&key| Some(key) != minimap && !self.cheats_hold(key))
            && let Err(e) = self.layer.update(cx, |layer, cx| layer.set_hotkey(key, cx))
        {
            log::warn!("overlay hotkey not changed: {e}");
        }
    }

    pub(in super::super) fn rebind_minimap(&mut self, outcome: Outcome, cx: &mut Context<Self>) {
        let key = match outcome {
            Outcome::Bound(chord) => match Key::from_vks(chord.vks()) {
                Some(key) => Some(key),
                None => return,
            },
            Outcome::Reset => Some(Key::MINIMAP),
            Outcome::Unbound => None,
            Outcome::Cancelled => return,
        };
        if !key.is_some_and(|key| key == self.layer.read(cx).hotkey() || self.cheats_hold(key)) {
            self.layer.update(cx, |layer, cx| layer.set_minimap_hotkey(key, cx));
        }
    }

    fn cheats_hold(&self, key: Key) -> bool {
        self.trainers.values().any(|load| match load {
            Load::Ready(loaded) => loaded.hotkeys.iter().flatten().flatten().any(|bound| key.fired_by(bound.vks())),
            _ => false,
        })
    }
}

fn key_text(key: Option<Key>) -> SharedString {
    let vks: Vec<u16> = key.map(|key| key.vks().map(u16::from).collect()).unwrap_or_default();
    Chord::from_vks(&vks).map_or_else(|| "-".into(), |chord| chord.to_string().into())
}
