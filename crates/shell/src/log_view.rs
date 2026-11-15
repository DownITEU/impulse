//! The log panel: every record of this run, filterable, selectable and copyable.

use std::{
    collections::{BTreeSet, VecDeque},
    sync::Arc,
};

use futures::StreamExt;
use gpui_kit::{
    App, ClipboardItem, Context, Entity, EventEmitter, FocusHandle, FontWeight, IntoElement, KeyBinding, Modifiers, Render, SharedString,
    Subscription, Task, UniformListScrollHandle, Window, actions,
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

use super::ui::{
    assets::Icon,
    key,
    text::{Texts, t, tf},
    theme::*,
    widgets::{button, filter, flash, flash_button, icon_button, input, mono, square, tracked},
};
use i18n::{Arg, Key, Language};
use journal::{Level, Record};

actions!(log_view, [CopyRows, SelectAllRows, CloseLog]);

const CONTEXT: &str = "log_view";
const LEVELS: [Level; 4] = [Level::Error, Level::Warn, Level::Info, Level::Debug];

pub fn bind_keys(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("ctrl-c", CopyRows, Some(CONTEXT)),
        KeyBinding::new("ctrl-a", SelectAllRows, Some(CONTEXT)),
        KeyBinding::new("escape", CloseLog, Some(CONTEXT)),
    ]);
}

pub enum LogEvent {
    /// Opened or closed.
    Toggled,
    /// The error or warning count changed.
    Problems,
}

pub struct LogView {
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
    // The COPY n caption of the last count and language.
    copy_label: Option<(usize, Language, SharedString)>,
    filter: Entity<InputState>,
    scroll: UniformListScrollHandle,
    focus: FocusHandle,
    open: bool,
    _relabel: Subscription,
    _pump: Task<()>,
}

impl EventEmitter<LogEvent> for LogView {}

fn slot(level: Level) -> usize {
    LEVELS.iter().position(|l| *l == level).unwrap_or(LEVELS.len() - 1)
}

/// A level's caption and color.
fn look(level: Level) -> (Key, u32) {
    match level {
        Level::Error => (key::LOG_ERROR, DANGER),
        Level::Warn => (key::LOG_WARN, WARN),
        Level::Info => (key::LOG_INFO, MUTED),
        _ => (key::LOG_DEBUG, GHOST),
    }
}

impl LogView {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let (filter, relabel) = input(key::LOG_FILTER, window, cx);
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
                if view.update(cx, |view, cx| view.pull(cx)).is_err() || pings.next().await.is_none() {
                    break;
                }
            }
        });
        Self {
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
            copy_label: None,
            filter,
            scroll: UniformListScrollHandle::new(),
            focus: cx.focus_handle(),
            open: false,
            _relabel: relabel,
            _pump: pump,
        }
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    pub fn toggle(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.open = !self.open;
        if self.open {
            self.focus.focus(window, cx);
        }
        cx.emit(LogEvent::Toggled);
    }

    pub fn close(&mut self, cx: &mut Context<Self>) {
        if self.open {
            self.open = false;
            cx.emit(LogEvent::Toggled);
        }
    }

    /// Buffered errors and warnings, for the status bar.
    pub fn problems(&self) -> (usize, usize) {
        (self.counts[slot(Level::Error)], self.counts[slot(Level::Warn)])
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
        self.rows = self.all.iter().filter(|record| self.passes(record)).cloned().collect();
        // A hidden row must not stay selected: copy would promise more than it copies.
        let rows = &self.rows;
        let visible = |seq: &u64| rows.binary_search_by_key(seq, |row| row.seq).is_ok();
        self.selected.retain(visible);
        self.anchor = self.anchor.filter(visible);
        cx.notify();
    }

    fn click(&mut self, seq: u64, modifiers: Modifiers, window: &mut Window, cx: &mut Context<Self>) {
        match self.anchor {
            Some(anchor) if modifiers.shift => {
                let span = anchor.min(seq)..=anchor.max(seq);
                self.selected = self.rows.iter().map(|row| row.seq).filter(|seq| span.contains(seq)).collect();
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
        self.focus.focus(window, cx);
        cx.notify();
    }

    // The selection, or every visible row when nothing is selected.
    fn copy(&mut self, cx: &mut Context<Self>) {
        let lines: Vec<String> =
            self.rows.iter().filter(|row| self.selected.is_empty() || self.selected.contains(&row.seq)).map(|row| row.line()).collect();
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

    // Made when the count or the language changes, not per repaint.
    fn copy_text(&mut self, cx: &App) -> SharedString {
        let count = self.selected.len();
        if count == 0 {
            return t(cx, key::LOG_COPY_ALL);
        }
        let language = cx.global::<Texts>().language();
        match &self.copy_label {
            Some((n, lang, text)) if *n == count && *lang == language => text.clone(),
            _ => {
                let text = tf(cx, key::LOG_COPY, &[("count", Arg::Num(count as f64))]);
                self.copy_label = Some((count, language, text.clone()));
                text
            }
        }
    }

    fn toolbar(&self, copy: &str, cx: &mut Context<Self>) -> impl IntoElement {
        let chips = LEVELS.into_iter().enumerate().map(|(slot, level)| {
            let (label, color) = look(level);
            let label = t(cx, label);
            h_flex()
                .id(("log-level", slot))
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
        h_flex()
            .h(px(48.))
            .flex_none()
            .pl(px(16.))
            .pr(px(8.))
            .gap(px(10.))
            .border_b_1()
            .border_color(line(0.09))
            .child(tracked(&t(cx, key::LOG_TITLE), 13., 0.16).font_weight(FontWeight::BOLD))
            .children(chips)
            .child(filter(&self.filter, cx).flex_1().min_w(px(120.)).h(px(28.)).px(px(10.)).border_1().border_color(line(0.1)))
            .child(
                button(&t(cx, key::LOG_FOLLOW))
                    .id("follow")
                    .when(self.follow, |button| button.child(square(7., rgb(accent(cx)))))
                    .on_click(cx.listener(|view, _, _, cx| {
                        view.follow = !view.follow;
                        if view.follow {
                            view.scroll.scroll_to_bottom();
                        }
                        cx.notify();
                    })),
            )
            .child(
                flash_button(copy, &t(cx, key::LOG_COPIED), self.copied.is_some(), cx)
                    .id("copy")
                    .on_click(cx.listener(|view, _, _, cx| view.copy(cx))),
            )
            .child(button(&t(cx, key::LOG_CLEAR)).id("clear").on_click(cx.listener(|view, _, _, cx| view.clear(cx))))
            .child(button(&t(cx, key::LOG_FOLDER)).id("folder").on_click(|_, _, cx| {
                if let Some(dir) = journal::dir() {
                    cx.open_with_system(dir);
                }
            }))
            .child(icon_button("log-close", Icon::Close, 32., 12.).on_click(cx.listener(|view, _, _, cx| view.close(cx))))
    }

    fn row(&self, record: &Record, cx: &mut Context<Self>) -> impl IntoElement {
        let seq = record.seq;
        let (label, color) = look(record.level);
        let label = t(cx, label);
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
            .on_click(cx.listener(move |view, event: &gpui_kit::ClickEvent, window, cx| {
                view.click(seq, event.modifiers(), window, cx);
            }))
            .child(mono(record.time.format("%H:%M:%S%.3f").to_string(), 12.).w(px(92.)).flex_none().text_color(rgb(FAINT)))
            .child(
                h_flex()
                    .w(px(58.))
                    .flex_none()
                    .gap(px(6.))
                    .child(square(7., rgb(color)))
                    .child(mono(label, 10.5).font_weight(FontWeight::BOLD).text_color(rgb(color))),
            )
            .child(mono(record.source.clone(), 12.).w(px(150.)).flex_none().truncate().text_color(rgb(DIM)))
            .child(mono(record.message.clone(), 12.).flex_1().min_w_0().truncate().text_color(rgb(text)))
    }

    fn detail(&self, cx: &App) -> impl IntoElement {
        let record = self.anchor.and_then(|seq| self.all.iter().find(|record| record.seq == seq));
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
                Some(record) => {
                    strip.text_color(rgb(TEXT)).child(SelectableText::new(("log-detail-text", record.seq as usize), record.line()))
                }
                None => strip.text_color(rgb(FAINT)).child(t(cx, key::LOG_DETAIL_EMPTY)),
            })
    }
}

const ROW_HEIGHT: f32 = 26.;

impl Render for LogView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let copy = self.copy_text(cx);
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
            .on_action(cx.listener(|view, _: &CloseLog, _, cx| view.close(cx)))
            .child(self.toolbar(&copy, cx))
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
                                rows.iter().map(|record| view.row(record, cx).into_any_element()).collect::<Vec<_>>()
                            }),
                        )
                        .track_scroll(&self.scroll)
                        .size_full(),
                    )
                    .vertical_scrollbar(&self.scroll),
            )
            .child(self.detail(cx))
    }
}
