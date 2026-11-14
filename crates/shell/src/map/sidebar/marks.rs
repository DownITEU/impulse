//! The sidebar's list of the user's own pins, with a switch for all of them.

use client::map::state::{Mark, PALETTE};
use gpui_kit::{AnyElement, Context, IntoElement, component::h_flex, div, prelude::*, px, rgb};

use super::{Sidebar, check, section};
use crate::{map::board::Board, ui::theme::*};

impl Sidebar {
    pub(super) fn marks_section(&self, board: &Board, cx: &Context<Self>) -> Vec<AnyElement> {
        let (marks, shown) = (&board.saved.marks, board.saved.marks_shown);
        let toggle = div().id("marks-check").child(check(PALETTE[0], shown)).on_click(cx.listener(|sidebar, _, _, cx| {
            cx.stop_propagation();
            sidebar.board.update(cx, |board, cx| board.filter(|saved| saved.marks_shown ^= true, cx));
        }));
        let header = section("map-marks", self.marks_open, "MY PINS", marks.len().to_string(), Some(toggle.into_any_element()), cx)
            .on_click(cx.listener(|sidebar, _, _, cx| {
                sidebar.marks_open ^= true;
                cx.notify();
            }));
        let mut rows = vec![header.into_any_element()];
        if !self.marks_open {
            return rows;
        }
        if marks.is_empty() {
            let hint = div().px(px(32.)).pb(px(8.)).text_size(px(12.)).text_color(rgb(FAINT)).child("Right click the map to add one.");
            rows.push(hint.into_any_element());
        }
        for (index, mark) in marks.iter().enumerate() {
            let row = h_flex()
                .id(("map-mark", index))
                .h(px(28.))
                .pl(px(32.))
                .pr(px(14.))
                .gap(px(10.))
                .cursor_pointer()
                .hover(|style| style.bg(rgb(ROW_HOVER)))
                .when(board.mark == Some(index), |row| row.bg(rgb(ROW_SELECTED)))
                .child(div().size(px(10.)).flex_none().bg(rgb(mark.color)))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .text_size(px(13.))
                        .text_color(rgb(if shown { TEXT_2 } else { FAINT }))
                        .child(title(index, mark)),
                )
                .on_click(cx.listener(move |sidebar, _, _, cx| sidebar.board.update(cx, |board, cx| board.select_mark(index, true, cx))));
            rows.push(row.into_any_element());
        }
        rows
    }
}

/// An unnamed mark goes by its place in the list.
fn title(index: usize, mark: &Mark) -> String {
    if mark.name.trim().is_empty() { format!("My pin {}", index + 1) } else { mark.name.clone() }
}
