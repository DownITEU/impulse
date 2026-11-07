//! The log panel: every record of this run, filterable, selectable and copyable.

use std::{
    collections::{BTreeSet, VecDeque},
    sync::Arc,
};

use futures::StreamExt;
use gpui_kit::{
    App, ClipboardItem, Context, Entity, EventEmitter, FocusHandle, FontWeight, IntoElement,
    KeyBinding, Modifiers, Render, Task, UniformListScrollHandle, Window, actions,
    base::SelectableText,
    component::{
        h_flex,
        input::{InputEvent, InputState},
        scroll::ScrollableElement,
        v_flex,
    },
    div,
    prelude::*,
    px, rgb, uniform_list,
};

use super::{
    assets::Icon,
    theme::*,
    widgets::{button, filter, flash, flash_button, icon_button, mono, square, tracked},
};
use crate::journal::{self, Level, Record};

actions!(log_view, [CopyRows, SelectAllRows, CloseLog]);

const CONTEXT: &str = "LogView";
const LEVELS: [Level; 4] = [Level::Error, Level::Warn, Level::Info, Level::Debug];

pub fn bind_keys(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("ctrl-c", CopyRows, Some(CONTEXT)),
        KeyBinding::new("ctrl-a", SelectAllRows, Some(CONTEXT)),
        KeyBinding::new("escape", CloseLog, Some(CONTEXT)),
    ]);
}

pub enum LogEvent {
    Close,
    /// The error or warning count changed.
    Problems,
}

pub struct LogView {
    pub(super) accent: u32,
    // Mirrors the journal's buffer, so filtering never holds its lock.
    all: VecDeque<Arc<Record>>,
    next_seq: u64,
    counts: [usize; LEVELS.len()],
    shown: [bool; LEVELS.len()],
    query: String,
    rows: Vec<Arc<Record>>,
    selected: BTreeSet<u64>,
    // The last clicked record: the shift-click origin and the detail strip's content.
    anchor: Option<u64>,
    follow: bool,
    // Runs while the copy button shows its confirmation.
    copied: Option<Task<()>>,
    filter: Entity<InputState>,
    scroll: UniformListScrollHandle,
    focus: FocusHandle,
    _pump: Task<()>,
}

impl EventEmitter<LogEvent> for LogView {}

fn slot(level: Level) -> usize {
    LEVELS
        .iter()
        .position(|l| *l == level)
        .unwrap_or(LEVELS.len() - 1)
}

fn look(level: Level) -> (&'static str, u32) {
    match level {
        Level::Error => ("ERROR", DANGER),
        Level::Warn => ("WARN", WARN),
        Level::Info => ("INFO", MUTED),
        _ => ("DEBUG", GHOST),
    }
}

impl LogView {
    pub fn new(accent: u32, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let filter = cx.new(|cx| InputState::new(window, cx).placeholder("filter log"));
        cx.subscribe(&filter, |view, filter, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Change) {
                view.query = filter.read(cx).value().to_lowercase();
                view.refilter(cx);
            }
        })
        .detach();
        let mut pings = journal::subscribe();
        let pump = cx.spawn(async move |view, cx| {
            loop {
                if view.update(cx, |view, cx| view.pull(cx)).is_err()
                    || pings.next().await.is_none()
                {
                    break;
                }
            }
        });
        Self {
            accent,
            all: VecDeque::new(),
            next_seq: 0,
            counts: [0; LEVELS.len()],
            shown: [true; LEVELS.len()],
            query: String::new(),
            rows: Vec::new(),
            selected: BTreeSet::new(),
            anchor: None,
            follow: true,
            copied: None,
            filter,
            scroll: UniformListScrollHandle::new(),
            focus: cx.focus_handle(),
            _pump: pump,
        }
    }

    pub fn focus(&self, window: &mut Window, cx: &mut App) {
        self.focus.focus(window, cx);
    }

    /// Buffered errors and warnings, for the status bar.
    pub fn problems(&self) -> (usize, usize) {
        (
            self.counts[slot(Level::Error)],
            self.counts[slot(Level::Warn)],
        )
    }

    fn pull(&mut self, cx: &mut Context<Self>) {
        let problems = self.problems();
        for record in journal::since(self.next_seq) {
            self.next_seq = record.seq + 1;
            self.counts[slot(record.level)] += 1;
            if self.passes(&record) {
                self.rows.push(record.clone());
            }
            self.all.push_back(record);
            if self.all.len() > journal::CAPACITY
                && let Some(old) = self.all.pop_front()
            {
                self.counts[slot(old.level)] -= 1;
                self.selected.remove(&old.seq);
                self.anchor = self.anchor.filter(|&anchor| anchor != old.seq);
                if self.rows.first().is_some_and(|row| row.seq == old.seq) {
                    self.rows.remove(0);
                }
            }
        }
        if problems != self.problems() {
            cx.emit(LogEvent::Problems);
        }
        if self.follow {
            self.scroll.scroll_to_bottom();
        }
        cx.notify();
    }

    fn passes(&self, record: &Record) -> bool {
        self.shown[slot(record.level)]
            && (self.query.is_empty()
                || record.message.to_lowercase().contains(&self.query)
                || record.source.to_lowercase().contains(&self.query))
    }

    fn refilter(&mut self, cx: &mut Context<Self>) {
        self.rows = self
            .all
            .iter()
            .filter(|record| self.passes(record))
            .cloned()
            .collect();
        // A hidden row must not stay selected: copy would promise more than it copies.
        let rows = &self.rows;
        let visible = |seq: &u64| rows.binary_search_by_key(seq, |row| row.seq).is_ok();
        self.selected.retain(visible);
        self.anchor = self.anchor.filter(visible);
        cx.notify();
    }

    fn click(
        &mut self,
        seq: u64,
        modifiers: Modifiers,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match self.anchor {
            Some(anchor) if modifiers.shift => {
                let span = anchor.min(seq)..=anchor.max(seq);
                self.selected = self
                    .rows
                    .iter()
                    .map(|row| row.seq)
                    .filter(|seq| span.contains(seq))
                    .collect();
            }
            _ if modifiers.control => {
                if !self.selected.remove(&seq) {
                    self.selected.insert(seq);
                }
                self.anchor = Some(seq);
            }
            _ => {
                self.selected = BTreeSet::from([seq]);
                self.anchor = Some(seq);
            }
        }
        self.focus(window, cx);
        cx.notify();
    }

    // The selection, or every visible row when nothing is selected.
    fn copy(&mut self, cx: &mut Context<Self>) {
        let lines: Vec<String> = self
            .rows
            .iter()
            .filter(|row| self.selected.is_empty() || self.selected.contains(&row.seq))
            .map(|row| row.line())
            .collect();
        // An empty copy would only wipe the clipboard.
        if lines.is_empty() {
            return;
        }
        cx.write_to_clipboard(ClipboardItem::new_string(lines.join("\n")));
        self.copied = Some(flash(cx, |view: &mut Self| view.copied = None));
        cx.notify();
    }

    // Empties the view only: the file keeps every record.
    fn clear(&mut self, cx: &mut Context<Self>) {
        self.all.clear();
        self.rows.clear();
        self.counts = [0; LEVELS.len()];
        self.selected.clear();
        self.anchor = None;
        cx.emit(LogEvent::Problems);
        cx.notify();
    }

    fn toolbar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let chips = LEVELS.into_iter().enumerate().map(|(slot, level)| {
            let (label, color) = look(level);
            h_flex()
                .id(label)
                .h(px(26.))
                .px(px(8.))
                .gap(px(6.))
                .border_1()
                .border_color(line(0.16))
                .cursor_pointer()
                .when(!self.shown[slot], |chip| chip.opacity(0.4))
                .on_click(cx.listener(move |view, _, _, cx| {
                    view.shown[slot] = !view.shown[slot];
                    view.refilter(cx);
                }))
                .child(square(7., rgb(color)))
                .child(mono(label, 10.5).font_weight(FontWeight::BOLD))
                .child(mono(self.counts[slot].to_string(), 10.5).text_color(rgb(FAINT)))
        });
        let copy = match self.selected.len() {
            0 => "COPY ALL".to_owned(),
            n => format!("COPY {n}"),
        };
        h_flex()
            .h(px(48.))
            .flex_none()
            .pl(px(16.))
            .pr(px(8.))
            .gap(px(10.))
            .border_b_1()
            .border_color(line(0.09))
            .child(tracked("LOG", 13., 0.16).font_weight(FontWeight::BOLD))
            .children(chips)
            .child(
                filter(&self.filter, self.accent)
                    .flex_1()
                    .min_w(px(120.))
                    .h(px(28.))
                    .px(px(10.))
                    .border_1()
                    .border_color(line(0.1)),
            )
            .child(
                button("FOLLOW")
                    .id("follow")
                    .when(self.follow, |button| {
                        button.child(square(7., rgb(self.accent)))
                    })
                    .on_click(cx.listener(|view, _, _, cx| {
                        view.follow = !view.follow;
                        if view.follow {
                            view.scroll.scroll_to_bottom();
                        }
                        cx.notify();
                    })),
            )
            .child(
                flash_button(&copy, "COPIED", self.copied.is_some(), self.accent)
                    .id("copy")
                    .on_click(cx.listener(|view, _, _, cx| view.copy(cx))),
            )
            .child(
                button("CLEAR")
                    .id("clear")
                    .on_click(cx.listener(|view, _, _, cx| view.clear(cx))),
            )
            .child(button("FOLDER").id("folder").on_click(|_, _, cx| {
                if let Some(dir) = journal::dir() {
                    cx.open_with_system(dir);
                }
            }))
            .child(
                icon_button("log-close", Icon::Close, 32., 12.)
                    .on_click(cx.listener(|_, _, _, cx| cx.emit(LogEvent::Close))),
            )
    }

    fn row(&self, record: &Record, cx: &mut Context<Self>) -> impl IntoElement {
        let seq = record.seq;
        let (label, color) = look(record.level);
        let (text, tint) = match record.level {
            Level::Error => (TEXT_BRIGHT, Some(alpha(DANGER, 0.07))),
            Level::Warn => (TEXT, Some(alpha(WARN, 0.05))),
            Level::Info => (TEXT_2, None),
            _ => (DIM, None),
        };
        h_flex()
            .id(("log-row", seq as usize))
            .h(px(ROW_HEIGHT))
            .w_full()
            .px(px(16.))
            .gap(px(12.))
            .cursor_pointer()
            .map(|row| match (self.selected.contains(&seq), tint) {
                (true, _) => row.bg(rgb(ROW_SELECTED)),
                (false, Some(tint)) => row.bg(tint),
                (false, None) => row.hover(|style| style.bg(rgb(ROW_HOVER))),
            })
            .on_click(
                cx.listener(move |view, event: &gpui_kit::ClickEvent, window, cx| {
                    view.click(seq, event.modifiers(), window, cx);
                }),
            )
            .child(
                mono(record.time.format("%H:%M:%S%.3f").to_string(), 12.)
                    .w(px(92.))
                    .flex_none()
                    .text_color(rgb(FAINT)),
            )
            .child(
                h_flex()
                    .w(px(58.))
                    .flex_none()
                    .gap(px(6.))
                    .child(square(7., rgb(color)))
                    .child(
                        mono(label, 10.5)
                            .font_weight(FontWeight::BOLD)
                            .text_color(rgb(color)),
                    ),
            )
            .child(
                mono(record.source.clone(), 12.)
                    .w(px(150.))
                    .flex_none()
                    .truncate()
                    .text_color(rgb(DIM)),
            )
            .child(
                mono(record.message.clone(), 12.)
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .text_color(rgb(text)),
            )
    }

    fn detail(&self) -> impl IntoElement {
        let record = self
            .anchor
            .and_then(|seq| self.all.iter().find(|record| record.seq == seq));
        div()
            .id("log-detail")
            .h(px(92.))
            .flex_none()
            .px(px(16.))
            .py(px(10.))
            .border_t_1()
            .border_color(line(0.09))
            .bg(rgb(PANEL))
            .font_family(MONO)
            .text_size(px(12.))
            .overflow_y_scroll()
            .map(|strip| match record {
                // Keyed by record, so a selection never carries over to another one.
                Some(record) => strip.text_color(rgb(TEXT)).child(SelectableText::new(
                    ("log-detail-text", record.seq as usize),
                    record.line(),
                )),
                None => strip
                    .text_color(rgb(FAINT))
                    .child("CLICK A ROW TO READ AND SELECT THE FULL RECORD"),
            })
    }
}

const ROW_HEIGHT: f32 = 26.;

impl Render for LogView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .size_full()
            .bg(rgb(BG))
            .track_focus(&self.focus)
            .key_context(CONTEXT)
            .on_action(cx.listener(|view, _: &CopyRows, _, cx| view.copy(cx)))
            .on_action(cx.listener(|view, _: &SelectAllRows, _, cx| {
                view.selected = view.rows.iter().map(|row| row.seq).collect();
                cx.notify();
            }))
            .on_action(cx.listener(|_, _: &CloseLog, _, cx| cx.emit(LogEvent::Close)))
            .child(self.toolbar(cx))
            .child(
                div()
                    .relative()
                    .flex_1()
                    .min_h_0()
                    .child(
                        uniform_list(
                            "log-rows",
                            self.rows.len(),
                            cx.processor(|view, range: std::ops::Range<usize>, _, cx| {
                                let rows = view.rows[range].to_vec();
                                rows.iter()
                                    .map(|record| view.row(record, cx).into_any_element())
                                    .collect::<Vec<_>>()
                            }),
                        )
                        .track_scroll(&self.scroll)
                        .size_full(),
                    )
                    .vertical_scrollbar(&self.scroll),
            )
            .child(self.detail())
    }
}
