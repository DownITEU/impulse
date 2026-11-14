//! The Settings `OVERLAY` tab and the overlay's and minimap's hotkeys' rebinding.

use gpui_kit::{App, Context, Entity, IntoElement, Render, SharedString, Window, component::v_flex, prelude::*, px, rgb};
use hotkeys::{Chord, Outcome};
use overlay::Key;

use super::Layer;
use crate::{
    bindings::{Hotkeys, PRESS_KEYS, Target},
    settings::row,
    trainer::Trainers,
    ui::{
        theme::*,
        widgets::{choice, keycap, segmented},
    },
};

const OVERLAY: Target = Target::Key { name: "overlay", required: true };
const MINIMAP: Target = Target::Key { name: "minimap", required: false };

pub struct OverlaySettings {
    layer: Entity<Layer>,
    hotkeys: Entity<Hotkeys>,
    trainers: Entity<Trainers>,
}

impl OverlaySettings {
    pub fn new(layer: Entity<Layer>, hotkeys: Entity<Hotkeys>, trainers: Entity<Trainers>, cx: &mut Context<Self>) -> Self {
        cx.observe(&layer, |_, _, cx| cx.notify()).detach();
        cx.observe(&hotkeys, |_, _, cx| cx.notify()).detach();
        Self { layer, hotkeys, trainers }
    }

    fn hotkey_cap(&self, id: &'static str, target: Target, key: Option<Key>, cx: &mut Context<Self>) -> impl IntoElement {
        let held = self.hotkeys.read(cx).held(&target);
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
                cap.border_color(accent)
                    .text_color(accent)
                    .on_mouse_down_out(cx.listener(|view, _, _, cx| view.hotkeys.update(cx, |hotkeys, cx| hotkeys.cancel(cx))))
            })
            .on_click(cx.listener(move |view, _, window, cx| view.bind(target.clone(), window, cx)))
    }

    /// Binds the overlay's or the minimap's hotkey by what the user presses.
    fn bind(&mut self, target: Target, window: &mut Window, cx: &mut Context<Self>) {
        let overlay = target == OVERLAY;
        let Some(outcome) = self.hotkeys.update(cx, |hotkeys, cx| hotkeys.capture(target, window, cx)) else { return };
        cx.spawn(async move |view, cx| {
            let Ok(outcome) = outcome.await else { return };
            view.update(cx, |view, cx| match overlay {
                true => view.rebind_overlay(outcome, cx),
                false => view.rebind_minimap(outcome, cx),
            })
            .ok();
        })
        .detach();
    }

    /// A refused chord keeps the old hotkey; the tab's hint says what is allowed.
    fn rebind_overlay(&mut self, outcome: Outcome, cx: &mut Context<Self>) {
        let key = match outcome {
            Outcome::Bound(chord) => Key::from_vks(chord.vks()),
            Outcome::Reset => Some(Key::DEFAULT),
            _ => None,
        };
        let minimap = self.layer.read(cx).minimap_hotkey();
        if let Some(key) = key.filter(|&key| Some(key) != minimap && !self.cheats_hold(key, cx))
            && let Err(e) = self.layer.update(cx, |layer, cx| layer.set_hotkey(key, cx))
        {
            log::warn!("overlay hotkey not changed: {e}");
        }
    }

    fn rebind_minimap(&mut self, outcome: Outcome, cx: &mut Context<Self>) {
        let key = match outcome {
            Outcome::Bound(chord) => match Key::from_vks(chord.vks()) {
                Some(key) => Some(key),
                None => return,
            },
            Outcome::Reset => Some(Key::MINIMAP),
            Outcome::Unbound => None,
            Outcome::Cancelled => return,
        };
        if !key.is_some_and(|key| key == self.layer.read(cx).hotkey() || self.cheats_hold(key, cx)) {
            self.layer.update(cx, |layer, cx| layer.set_minimap_hotkey(key, cx));
        }
    }

    fn cheats_hold(&self, key: Key, cx: &App) -> bool {
        self.trainers.read(cx).sheets().any(|sheet| sheet.hotkeys.iter().flatten().flatten().any(|bound| key.fired_by(bound.vks())))
    }
}

impl Render for OverlaySettings {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
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
                OVERLAY,
                overlay,
                cx,
            )))
            .child(
                row("MINIMAP HOTKEY", "shows or hides the minimap; held only while the game is in front", 64.)
                    .border_b_0()
                    .child(self.hotkey_cap("minimap-hotkey", MINIMAP, minimap, cx)),
            )
    }
}

/// The overlay's hotkey as the hero shows it.
pub fn overlay_hotkey(layer: &Layer) -> SharedString {
    key_text(Some(layer.hotkey()))
}

fn key_text(key: Option<Key>) -> SharedString {
    let vks: Vec<u16> = key.map(|key| key.vks().map(u16::from).collect()).unwrap_or_default();
    Chord::from_vks(&vks).map_or_else(|| "-".into(), |chord| chord.to_string().into())
}
