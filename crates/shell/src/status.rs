//! The status bar: the live run, the log's problems and the hotkeys' state.

use gpui_kit::{Context, IntoElement, component::h_flex, div, prelude::*, px, rgb};

use super::{
    Page,
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
        let running = self.library.read(cx).games().iter().find(|game| Some(&game.id) == self.play.live());
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
                    let (active, total) = self.sheet(&game.id, cx).map_or((0, 0), |sheet| (sheet.active(), sheet.cheats.len()));
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
}
