//! The status bar: the live run, the log's problems and the hotkeys' state.

use gpui_kit::{App, Context, Div, IntoElement, component::h_flex, div, prelude::*, px, rgb};

use super::{
    Page,
    bindings::Target,
    theme::*,
    widgets::{mono, square},
};

impl Page {
    pub(super) fn status_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let (errors, warnings) = self.log.read(cx).problems();
        let problem = |count: usize, color: u32| {
            h_flex().gap(px(6.)).child(square(7., rgb(color))).child(mono(count.to_string(), 11.).text_color(rgb(color)))
        };
        let accent = rgb(accent(cx));
        let running = self.runs.read(cx).live().and_then(|live| self.library.read(cx).game(live));
        h_flex()
            .h(px(30.))
            .flex_none()
            .px(px(20.))
            .gap(px(22.))
            .border_t_1()
            .border_color(line(0.09))
            .bg(rgb(STATUSBAR))
            .text_color(rgb(DIM))
            .map(|bar| match running {
                Some(game) => {
                    let (active, total) =
                        self.trainers.read(cx).sheet(&game.id).map_or((0, 0), |sheet| (sheet.active(), sheet.cheats.len()));
                    bar.child(h_flex().gap(px(8.)).child(square(7., accent)).child(mono("LIVE", 11.).text_color(accent)))
                        .child(mono(game.name.clone(), 11.))
                        .child(mono(format!("{active}/{total} ACTIVE"), 11.))
                }
                None => bar.child(mono("IDLE", 11.)),
            })
            .child(div().flex_1())
            .child(
                h_flex()
                    .id("log")
                    .h_full()
                    .gap(px(10.))
                    .cursor_pointer()
                    .hover(|style| style.text_color(rgb(TEXT)))
                    .when(self.log_open, |item| item.text_color(rgb(TEXT)))
                    .on_click(cx.listener(|page, _, window, cx| {
                        page.log_open = !page.log_open;
                        if page.log_open {
                            page.log.update(cx, |log, cx| log.focus(window, cx));
                        }
                        cx.notify();
                    }))
                    .child(mono("LOG", 11.))
                    .when(errors > 0, |item| item.child(problem(errors, DANGER)))
                    .when(warnings > 0, |item| item.child(problem(warnings, WARN))),
            )
            .child(self.hotkeys_status(cx))
            .child(mono("IMPULSE 0.1", 11.).text_color(rgb(GHOST)))
    }

    /// The hotkeys' state; while binding, the keys that end it.
    fn hotkeys_status(&self, cx: &App) -> Div {
        let hotkeys = self.hotkeys.read(cx);
        if let Some(target) = hotkeys.capturing() {
            let keys = match target {
                Target::Key { required: true, .. } => "PRESS KEYS · ESC CANCEL · BKSP DEFAULT",
                _ => "PRESS KEYS · ESC CANCEL · BKSP DEFAULT · DEL UNBIND",
            };
            return mono(keys, 11.).text_color(rgb(accent(cx)));
        }
        match hotkeys.is_armed() {
            true => mono("HOTKEYS ON", 11.),
            false => mono("HOTKEYS OFF", 11.).text_color(rgb(GHOST)),
        }
    }
}
