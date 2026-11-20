//! The SOUNDS tab: a row per cue with LISTEN, the file to play instead, and its switch.

use gpui_kit::{
    App, Context, Entity, IntoElement, Render, Window,
    component::{h_flex, v_flex},
    prelude::*,
    px,
};
use i18n::Key;
use sound::Cue;

use super::Cues;
use crate::{
    settings::{Settings, row},
    ui::{
        assets::Icon,
        key,
        text::t,
        widgets::{icon_button, switch, tip},
    },
};

/// Each cue's id, title and hint.
const ROWS: [(Cue, &str, Key, Key); 6] = [
    (Cue::ToggleOn, "toggle-on", key::SOUNDS_TOGGLE_ON, key::SOUNDS_TOGGLE_ON_HINT),
    (Cue::ToggleOff, "toggle-off", key::SOUNDS_TOGGLE_OFF, key::SOUNDS_TOGGLE_OFF_HINT),
    (Cue::Slider, "slider", key::SOUNDS_SLIDER, key::SOUNDS_SLIDER_HINT),
    (Cue::Number, "number", key::SOUNDS_NUMBER, key::SOUNDS_NUMBER_HINT),
    (Cue::Button, "button", key::SOUNDS_BUTTON, key::SOUNDS_BUTTON_HINT),
    (Cue::Selection, "selection", key::SOUNDS_SELECTION, key::SOUNDS_SELECTION_HINT),
];

pub struct SoundsView {
    settings: Entity<Settings>,
    cues: Entity<Cues>,
}

impl SoundsView {
    pub fn new(settings: Entity<Settings>, cues: Entity<Cues>, cx: &mut Context<Self>) -> Self {
        cx.observe(&settings, |_, _, cx| cx.notify()).detach();
        cx.observe(&cues, |_, _, cx| cx.notify()).detach();
        Self { settings, cues }
    }
}

impl Render for SoundsView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let refused = self.cues.read(cx).refused();
        let rows = ROWS.into_iter().enumerate().map(|(at, (cue, id, title, hint))| {
            let sounds = &self.settings.read(cx).prefs().cues;
            let on = sounds.plays(cue);
            let file = sounds.file(cue).and_then(|file| file.file_name()).map(|name| name.to_string_lossy().into_owned());
            let hint = match (&file, refused == Some(cue)) {
                (_, true) => t(cx, key::SOUNDS_REFUSED),
                (Some(name), _) => name.clone().into(),
                _ => t(cx, hint),
            };
            let tooltip = |label: Key| {
                let label = t(cx, label);
                move |window: &mut Window, cx: &mut App| tip(label.clone(), window, cx)
            };
            row(t(cx, title), hint, 64.).when(at + 1 == ROWS.len(), |row| row.border_b_0()).child(
                h_flex()
                    .gap(px(4.))
                    .child(
                        icon_button(("listen", at), Icon::Play, 28., 12.)
                            .tooltip(tooltip(key::SOUNDS_LISTEN))
                            .on_click(cx.listener(move |view, _, _, cx| view.cues.read(cx).play(cue, cx))),
                    )
                    .child(
                        icon_button(("choose", at), Icon::Folder, 28., 12.)
                            .tooltip(tooltip(key::SOUNDS_CHOOSE))
                            .on_click(cx.listener(move |view, _, _, cx| view.cues.update(cx, |cues, cx| cues.choose(cue, cx)))),
                    )
                    .when(file.is_some(), |buttons| {
                        buttons.child(
                            icon_button(("reset", at), Icon::Close, 28., 12.)
                                .tooltip(tooltip(key::SOUNDS_RESET))
                                .on_click(cx.listener(move |view, _, _, cx| view.cues.update(cx, |cues, cx| cues.clear(cue, cx)))),
                        )
                    })
                    .child(switch(id, on, cx).ml(px(6.)).on_click(cx.listener(move |view, _, _, cx| {
                        view.settings.update(cx, |settings, cx| settings.edit(|prefs| prefs.cues.set(cue, !on), cx));
                    }))),
            )
        });
        v_flex().flex_1().px(px(18.)).py(px(6.)).children(rows)
    }
}
