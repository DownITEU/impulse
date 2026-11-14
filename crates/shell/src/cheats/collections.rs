//! Collections: named sets of mods with their values, a chip each over the cheat list. A click sets the
//! running trainer to one, or turns its mods off when all are on; a right click edits or deletes it.

use client::{
    library::GameId,
    profile::{Collection, Values},
    trainer::Sheet,
};
use gpui_kit::{
    Context, Div, Entity, EventEmitter, FocusHandle, IntoElement, MouseButton, Render, Stateful, Subscription, Window,
    component::{
        h_flex,
        input::{Input, InputEvent, InputState},
        scroll::ScrollableElement,
        v_flex,
    },
    deferred, div,
    prelude::*,
    px, rgb,
};

use super::{CheatList, EditorClosed, Menu, open_menu};
use crate::{
    trainer::Trainers,
    ui::{
        assets::Icon,
        dialog::dialog,
        dropdown::{close, picks},
        theme::*,
        widgets::{button, icon, icon_button, mono, tip},
    },
};

/// By the key `games.json` keeps; an unknown key shows the first.
const ICONS: [(&str, Icon); 12] = [
    ("swords", Icon::Swords),
    ("shield", Icon::Shield),
    ("heart", Icon::Heart),
    ("zap", Icon::Zap),
    ("skull", Icon::Skull),
    ("crosshair", Icon::Crosshair),
    ("flame", Icon::Flame),
    ("gem", Icon::Gem),
    ("coins", Icon::Coins),
    ("rocket", Icon::Rocket),
    ("ghost", Icon::Ghost),
    ("crown", Icon::Crown),
];
const NAME_LENGTH: usize = 24;
const TILE: f32 = 44.;
const TILE_GAP: f32 = 8.;

fn listed(key: &str) -> (&'static str, Icon) {
    *ICONS.iter().find(|(known, _)| *known == key).unwrap_or(&ICONS[0])
}

impl CheatList {
    pub(super) fn collections_bar(&self, game: &GameId, sheet: &Sheet, running: bool, cx: &Context<Self>) -> impl IntoElement {
        let collections = &self.trainers.read(cx).trainer_profile(game).collections;
        // A new one is made of the mods that are on.
        let addable = running && sheet.active() > 0;
        let chips = collections
            .iter()
            .enumerate()
            .map(|(at, collection)| self.chip(at, collection, running, running && sheet.holds(&collection.mods), cx));
        h_flex()
            .h(px(40.))
            .flex_none()
            .pl(px(14.))
            .pr(px(6.))
            .gap(px(8.))
            .border_b_1()
            .border_color(line(0.09))
            .child(
                h_flex()
                    .id("collections")
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .gap(px(6.))
                    .when(collections.is_empty(), |bar| bar.child(mono("NO COLLECTIONS", 10.5).text_color(rgb(GHOST))))
                    .children(chips)
                    // The kit's scroller: inside the pane's, a plain one never gets the wheel.
                    .overflow_x_scrollbar(),
            )
            .child(
                icon_button("collection-add", Icon::Plus, 28., 13.)
                    .tooltip(|window, cx| tip("Keeps the mods that are on now as a collection.".into(), window, cx))
                    .when(!addable, |add| add.opacity(0.4).cursor_default())
                    .when(addable, |add| add.on_click(cx.listener(|list, _, window, cx| list.edit_collection(None, window, cx)))),
            )
    }

    fn chip(&self, at: usize, collection: &Collection, running: bool, on: bool, cx: &Context<Self>) -> Stateful<Div> {
        let color = rgb(match (on, running) {
            (true, _) => accent(cx),
            (false, true) => MUTED,
            (false, false) => FAINT,
        });
        let menu = Menu::Collection(at);
        h_flex()
            .id(("collection", at))
            .relative()
            .flex_none()
            .h(px(26.))
            .px(px(9.))
            .gap(px(7.))
            .border_1()
            .border_color(if on { color } else { line(0.16) })
            .bg(rgb(RAISED))
            .text_color(color)
            .child(icon(listed(&collection.icon).1, 13., color))
            .child(mono(collection.name.to_uppercase(), 10.5))
            .when(running, |chip| {
                chip.cursor_pointer()
                    .hover(|style| style.bg(rgb(RAISED_HOVER)))
                    .on_click(cx.listener(move |list, _, _, cx| list.toggle_collection(at, cx)))
            })
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(move |list, _, _, cx| {
                    list.menu = Some(menu);
                    cx.notify();
                }),
            )
            .on_mouse_down_out(cx.listener(move |list, _, _, cx| close(list, open_menu, menu, cx)))
            .when(self.menu == Some(menu), |chip| {
                let actions =
                    picks(vec!["EDIT".into(), "DELETE".into()], usize::MAX, open_menu, cx, move |list, pick, window, cx| match pick {
                        0 => list.edit_collection(Some(at), window, cx),
                        _ => list.delete_collection(at, cx),
                    });
                chip.child(deferred(actions.left(px(-1.)).min_w(px(110.))))
            })
    }

    fn toggle_collection(&mut self, at: usize, cx: &mut Context<Self>) {
        let Some(game) = self.selected(cx) else { return };
        let trainers = self.trainers.read(cx);
        let Some(mods) = trainers.trainer_profile(&game).collections.get(at).map(|collection| collection.mods.clone()) else { return };
        let Some(sheet) = trainers.sheet(&game) else { return };
        let values = if sheet.holds(&mods) { sheet.rest(&mods) } else { mods };
        self.runs.update(cx, |runs, cx| runs.apply(&values, cx));
    }

    fn delete_collection(&mut self, at: usize, cx: &mut Context<Self>) {
        let Some(game) = self.selected(cx) else { return };
        self.trainers.update(cx, |trainers, cx| trainers.delete_collection(&game, at, cx));
    }

    /// Opens the editor on the collection at `at`, or on a new one of the mods that are on.
    fn edit_collection(&mut self, at: Option<usize>, window: &mut Window, cx: &mut Context<Self>) {
        let Some(game) = self.selected(cx) else { return };
        let trainers = self.trainers.read(cx);
        let changed = at.and_then(|at| trainers.trainer_profile(&game).collections.get(at));
        let (title, key, mods) = match (changed, at) {
            (Some(collection), _) => (collection.name.clone(), listed(&collection.icon).0, collection.mods.clone()),
            (None, None) => (String::new(), ICONS[0].0, trainers.sheet(&game).map(Sheet::live_values).unwrap_or_default()),
            (None, Some(_)) => return,
        };
        if mods.is_empty() {
            return;
        }
        let trainers = self.trainers.clone();
        let editor = cx.new(|cx| CollectionEditor::new(trainers, game, at, mods, key, title, window, cx));
        cx.subscribe(&editor, |list, _, _: &EditorClosed, cx| {
            list.editor = None;
            cx.emit(EditorClosed);
            cx.notify();
        })
        .detach();
        self.editor = Some(editor);
        cx.notify();
    }
}

/// The dialog that names a collection and picks its icon.
pub struct CollectionEditor {
    trainers: Entity<Trainers>,
    game: GameId,
    /// The collection being changed; `None` makes one of `mods`.
    at: Option<usize>,
    mods: Values,
    icon: &'static str,
    name: Entity<InputState>,
    focus: FocusHandle,
    _typed: Subscription,
}

impl EventEmitter<EditorClosed> for CollectionEditor {}

impl CollectionEditor {
    #[expect(clippy::too_many_arguments, reason = "the collection's game, place, mods, icon and name")]
    fn new(
        trainers: Entity<Trainers>,
        game: GameId,
        at: Option<usize>,
        mods: Values,
        icon: &'static str,
        title: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let name = cx.new(|cx| {
            InputState::new(window, cx).placeholder("name").validate(|text, _| text.chars().count() <= NAME_LENGTH).default_value(title)
        });
        let typed = cx.subscribe_in(&name, window, |editor, _, event: &InputEvent, window, cx| match event {
            InputEvent::PressEnter { .. } => editor.keep(window, cx),
            _ => cx.notify(),
        });
        // Typing replaces the old name.
        name.update(cx, |name, cx| {
            name.focus(window, cx);
            name.select_all(window, cx);
        });
        Self { trainers, game, at, mods, icon, name, focus: cx.focus_handle(), _typed: typed }
    }

    fn keep(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let name = self.name.read(cx).value().trim().to_owned();
        if name.is_empty() {
            return;
        }
        let collection = Collection { name, icon: self.icon.to_owned(), mods: self.mods.clone() };
        let (game, at) = (self.game.clone(), self.at);
        self.trainers.update(cx, |trainers, cx| trainers.keep_collection(&game, at, collection, cx));
        self.close(window, cx);
    }

    pub(super) fn close(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        cx.emit(EditorClosed);
    }
}

impl Render for CollectionEditor {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let accent = rgb(accent(cx));
        let named = !self.name.read(cx).value().trim().is_empty();
        let tiles = ICONS.map(|(key, glyph)| {
            let current = key == self.icon;
            div()
                .id(key)
                .size(px(TILE))
                .flex()
                .items_center()
                .justify_center()
                .border_1()
                .border_color(if current { accent } else { line(0.12) })
                .bg(rgb(RAISED))
                .cursor_pointer()
                .hover(|style| style.bg(rgb(RAISED_HOVER)))
                .on_click(cx.listener(move |editor, _, window, cx| {
                    editor.icon = key;
                    // Enter keeps saving from the field.
                    editor.name.update(cx, |name, cx| name.focus(window, cx));
                    cx.notify();
                }))
                .child(icon(glyph, 18., if current { accent } else { rgb(MUTED) }))
        });
        let field = h_flex()
            .h(px(36.))
            .px(px(10.))
            .border_1()
            .border_color(line(0.14))
            .bg(rgb(SUNKEN))
            .child(Input::new(&self.name).appearance(false).px_0().font_family(MONO).text_size(px(12.5)));
        let buttons = h_flex()
            .gap(px(8.))
            .child(button("CANCEL").id("collection-cancel").on_click(cx.listener(|editor, _, window, cx| editor.close(window, cx))))
            .child(
                button("SAVE")
                    .id("collection-save")
                    .border_color(accent)
                    .text_color(accent)
                    .when(!named, |save| save.opacity(0.4).cursor_default())
                    .on_click(cx.listener(|editor, _, window, cx| editor.keep(window, cx))),
            );
        let body = v_flex()
            .p(px(16.))
            .gap(px(16.))
            .child(part("NAME").child(field))
            .child(part("ICON").child(h_flex().flex_wrap().gap(px(TILE_GAP)).children(tiles)))
            .child(
                h_flex().justify_between().child(mono(format!("{:02} MODS", self.mods.len()), 10.5).text_color(rgb(FAINT))).child(buttons),
            );
        let title = if self.at.is_some() { "EDIT COLLECTION" } else { "NEW COLLECTION" };
        // Six tiles across, inside the padding and the borders.
        let width = 6. * TILE + 5. * TILE_GAP + 34.;
        dialog("collection", title, width, &self.focus, Self::close, body, cx)
    }
}

/// A captioned part of the editor.
fn part(caption: &'static str) -> Div {
    v_flex().gap(px(8.)).child(mono(caption, 10.5).text_color(rgb(DIM)))
}
