//! Collections: named sets of mods with their values, a chip each over the cheat list. A click sets the
//! running trainer to one, or turns its mods off when all are on; a right click edits or deletes it.

use client::{
    library::GameId,
    profile::{Collection, NAME_LENGTH, Values},
    trainer::Sheet,
};
use gpui_kit::{
    Context, Div, Entity, EventEmitter, FocusHandle, IntoElement, MouseButton, Render, SharedString, Stateful, Subscription, Window,
    component::{
        h_flex,
        input::{Input, InputEvent, InputState},
        scroll::ScrollableElement,
        v_flex,
    },
    deferred, div,
    prelude::*,
    px, relative, rgb,
};

use i18n::Arg;

use super::{CheatList, EditorClosed, Menu, open_menu};
use crate::{
    trainer::Trainers,
    ui::{
        assets::Icon,
        dialog::dialog,
        dropdown::{close, picks},
        key,
        text::{t, tf},
        theme::*,
        widgets::{button, icon, icon_button, input, mono, tip},
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
/// What a new collection shows until the user picks.
pub const DEFAULT_ICON: &str = ICONS[0].0;
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
        let add_tip = t(cx, key::CHEATS_COLLECT_TIP);
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
                    .when(collections.is_empty(), |bar| {
                        bar.child(mono(t(cx, key::CHEATS_NO_COLLECTIONS), TYPE_CAPTION).text_color(rgb(GHOST)))
                    })
                    .children(chips)
                    // The kit's scroller: inside the pane's, a plain one never gets the wheel.
                    .overflow_x_scrollbar(),
            )
            .child(
                icon_button("collection-add", Icon::Plus, 28., 13.)
                    .tooltip(move |window, cx| tip(add_tip.clone(), window, cx))
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
            .child(mono(collection.name.to_uppercase(), TYPE_CAPTION))
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
                let options = vec![t(cx, key::CHEATS_EDIT), t(cx, key::CHEATS_DELETE)];
                let actions = picks(options, usize::MAX, open_menu, cx, move |list, pick, window, cx| match pick {
                    0 => list.edit_collection(Some(at), window, cx),
                    _ => list.delete_collection(at, cx),
                });
                chip.child(deferred(actions.absolute().top(relative(1.)).mt(px(3.)).left(px(-1.)).min_w(px(110.))))
            })
    }

    fn toggle_collection(&mut self, at: usize, cx: &mut Context<Self>) {
        let Some(game) = self.selected(cx) else { return };
        self.runs.update(cx, |runs, cx| runs.toggle_collection(&game, at, cx));
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
        let was = changed.cloned();
        let (title, key, mods) = match (changed, at) {
            (Some(collection), _) => (collection.name.clone(), listed(&collection.icon).0, collection.mods.clone()),
            (None, None) => (String::new(), DEFAULT_ICON, trainers.sheet(&game).map(Sheet::live_values).unwrap_or_default()),
            (None, Some(_)) => return,
        };
        if mods.is_empty() {
            return;
        }
        let trainers = self.trainers.clone();
        let editor = cx.new(|cx| CollectionEditor::new(trainers, game, was, mods, key, title, window, cx));
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
    was: Option<Collection>,
    mods: Values,
    icon: &'static str,
    name: Entity<InputState>,
    focus: FocusHandle,
    // Typing, and the placeholder's language.
    _typed: Subscription,
}

impl EventEmitter<EditorClosed> for CollectionEditor {}

impl CollectionEditor {
    #[expect(clippy::too_many_arguments, reason = "the collection's game, old self, mods, icon and name")]
    fn new(
        trainers: Entity<Trainers>,
        game: GameId,
        was: Option<Collection>,
        mods: Values,
        icon: &'static str,
        title: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let (name, relabel) = input(key::CHEATS_NAME_FIELD, window, cx);
        name.update(cx, |name, cx| {
            name.set_value(title, window, cx);
            name.set_validator(|text, _| text.chars().count() <= NAME_LENGTH, cx);
        });
        let typed = cx.subscribe_in(&name, window, |editor, _, event: &InputEvent, window, cx| match event {
            InputEvent::PressEnter { .. } => editor.keep(window, cx),
            _ => cx.notify(),
        });
        let typed = Subscription::join(typed, relabel);
        name.update(cx, |name, cx| {
            name.focus(window, cx);
            name.select_all(window, cx);
        });
        Self { trainers, game, was, mods, icon, name, focus: cx.focus_handle(), _typed: typed }
    }

    fn keep(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(collection) = Collection::new(&self.name.read(cx).value(), self.icon, self.mods.clone()) else { return };
        let (game, was) = (self.game.clone(), self.was.clone());
        self.trainers.update(cx, |trainers, cx| trainers.keep_collection(&game, was.as_ref(), collection, cx));
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
            .child(Input::new(&self.name).appearance(false).px_0().font_family(MONO).text_size(px(TYPE_FIELD)));
        let buttons = h_flex()
            .gap(px(8.))
            .child(
                button(&t(cx, key::CHEATS_CANCEL))
                    .id("collection-cancel")
                    .on_click(cx.listener(|editor, _, window, cx| editor.close(window, cx))),
            )
            .child(
                button(&t(cx, key::CHEATS_SAVE))
                    .id("collection-save")
                    .border_color(accent)
                    .text_color(accent)
                    .when(!named, |save| save.opacity(0.4).cursor_default())
                    .on_click(cx.listener(|editor, _, window, cx| editor.keep(window, cx))),
            );
        let count = self.mods.len();
        let padded = format!("{count:02}");
        let mods = tf(cx, key::CHEATS_COLLECTION_MODS, &[("mods", Arg::Str(&padded)), ("count", Arg::Num(count as f64))]);
        let body = v_flex()
            .p(px(16.))
            .gap(px(16.))
            .child(part(t(cx, key::CHEATS_NAME)).child(field))
            .child(part(t(cx, key::CHEATS_ICON)).child(h_flex().flex_wrap().gap(px(TILE_GAP)).children(tiles)))
            .child(h_flex().justify_between().child(mono(mods, TYPE_CAPTION).text_color(rgb(FAINT))).child(buttons));
        let title = t(cx, if self.was.is_some() { key::CHEATS_EDIT_COLLECTION } else { key::CHEATS_NEW_COLLECTION });
        // Six tiles across, inside the padding and the borders.
        let width = 6. * TILE + 5. * TILE_GAP + 34.;
        dialog("collection", title, width, &self.focus, Self::close, body, cx)
    }
}

/// A captioned part of the editor.
fn part(caption: SharedString) -> Div {
    v_flex().gap(px(8.)).child(mono(caption, TYPE_CAPTION).text_color(rgb(DIM)))
}
