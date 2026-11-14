//! Collections: named sets of mods with their values, a chip each over the cheat list. A click sets the
//! running trainer to one, or turns its mods off when all are on; a right click edits or deletes it.

use std::sync::Arc;

use client::profile::{Collection, Values};
use gpui_kit::{
    Context, Div, Entity, IntoElement, MouseButton, Stateful, Subscription, Window,
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

use super::{
    Menu, Page,
    assets::Icon,
    loaded::Loaded,
    theme::*,
    widgets::{button, icon, icon_button, mono, tip},
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

/// The dialog that names a collection and picks its icon.
pub(super) struct Editor {
    game: Arc<str>,
    /// The collection being changed; `None` makes one of `mods`.
    at: Option<usize>,
    mods: Values,
    icon: &'static str,
    name: Entity<InputState>,
    _typed: Subscription,
}

impl Page {
    pub(super) fn collections_bar(&self, loaded: &Loaded, running: bool, cx: &mut Context<Self>) -> impl IntoElement {
        let collections = &self.profiles.of(self.selected(cx).as_deref().unwrap_or_default()).collections;
        // A new one is made of the mods that are on.
        let addable = running && loaded.sheet.active() > 0;
        let chips = collections
            .iter()
            .enumerate()
            .map(|(at, collection)| self.chip(at, collection, running, running && loaded.sheet.holds(&collection.mods), cx));
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
                    .when(addable, |add| add.on_click(cx.listener(|page, _, window, cx| page.edit_collection(None, window, cx)))),
            )
    }

    fn chip(&self, at: usize, collection: &Collection, running: bool, on: bool, cx: &mut Context<Self>) -> Stateful<Div> {
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
                    .on_click(cx.listener(move |page, _, window, cx| page.toggle_collection(at, window, cx)))
            })
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(move |page, _, _, cx| {
                    page.menu = Some(menu);
                    cx.notify();
                }),
            )
            .on_mouse_down_out(cx.listener(move |page, _, _, cx| {
                if page.menu == Some(menu) {
                    page.menu = None;
                    cx.notify();
                }
            }))
            .when(self.menu == Some(menu), |chip| {
                let actions = self.picks(vec!["EDIT".into(), "DELETE".into()], usize::MAX, cx, move |page, pick, window, cx| match pick {
                    0 => page.edit_collection(Some(at), window, cx),
                    _ => page.delete_collection(at, cx),
                });
                chip.child(deferred(actions.left(px(-1.)).min_w(px(110.))))
            })
    }

    fn toggle_collection(&mut self, at: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(game) = self.selected(cx).clone() else { return };
        let Some(mods) = self.profiles.of(&game).collections.get(at).map(|collection| collection.mods.clone()) else { return };
        let Some(loaded) = self.trainer(Some(&game)) else { return };
        let values = if loaded.sheet.holds(&mods) { loaded.sheet.rest(&mods) } else { mods };
        self.apply(&values, window, cx);
    }

    fn delete_collection(&mut self, at: usize, cx: &mut Context<Self>) {
        let Some(game) = self.selected(cx).clone() else { return };
        self.profiles.edit(&game, |profile| {
            if at < profile.collections.len() {
                profile.collections.remove(at);
            }
        });
        cx.notify();
    }

    /// Opens the editor on the collection at `at`, or on a new one of the mods that are on.
    fn edit_collection(&mut self, at: Option<usize>, window: &mut Window, cx: &mut Context<Self>) {
        let Some(game) = self.selected(cx).clone() else { return };
        let changed = at.and_then(|at| self.profiles.of(&game).collections.get(at));
        let (title, key, mods) = match (changed, at) {
            (Some(collection), _) => (collection.name.clone(), listed(&collection.icon).0, collection.mods.clone()),
            (None, None) => {
                (String::new(), ICONS[0].0, self.trainer(Some(&game)).map(|loaded| loaded.sheet.live_values()).unwrap_or_default())
            }
            (None, Some(_)) => return,
        };
        if mods.is_empty() {
            return;
        }
        let name = cx.new(|cx| {
            InputState::new(window, cx).placeholder("name").validate(|text, _| text.chars().count() <= NAME_LENGTH).default_value(title)
        });
        let typed = cx.subscribe_in(&name, window, |page, _, event: &InputEvent, window, cx| match event {
            InputEvent::PressEnter { .. } => page.keep_collection(window, cx),
            _ => cx.notify(),
        });
        // Typing replaces the old name.
        name.update(cx, |name, cx| {
            name.focus(window, cx);
            name.select_all(window, cx);
        });
        self.editor = Some(Editor { game, at, mods, icon: key, name, _typed: typed });
        cx.notify();
    }

    fn keep_collection(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(editor) = &self.editor else { return };
        let name = editor.name.read(cx).value().trim().to_owned();
        if name.is_empty() {
            return;
        }
        let (game, at, icon, mods) = (editor.game.clone(), editor.at, editor.icon.to_owned(), editor.mods.clone());
        self.profiles.edit(&game, |profile| match at.and_then(|at| profile.collections.get_mut(at)) {
            Some(collection) => (collection.name, collection.icon) = (name, icon),
            None => profile.collections.push(Collection { name, icon, mods }),
        });
        self.close_editor(window, cx);
    }

    pub(super) fn close_editor(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.editor.take().is_some() {
            // The focus must not stay in the gone field, or Escape would reach nothing.
            if self.over {
                self.hud_focus.focus(window, cx);
            }
            cx.notify();
        }
    }

    pub(super) fn collection_editor(&self, cx: &mut Context<Self>) -> Option<impl IntoElement> {
        let editor = self.editor.as_ref()?;
        let accent = rgb(accent(cx));
        let named = !editor.name.read(cx).value().trim().is_empty();
        let tiles = ICONS.map(|(key, glyph)| {
            let current = key == editor.icon;
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
                .on_click(cx.listener(move |page, _, window, cx| {
                    if let Some(editor) = &mut page.editor {
                        editor.icon = key;
                        // Enter keeps saving from the field.
                        editor.name.update(cx, |name, cx| name.focus(window, cx));
                    }
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
            .child(Input::new(&editor.name).appearance(false).px_0().font_family(MONO).text_size(px(12.5)));
        let buttons = h_flex()
            .gap(px(8.))
            .child(button("CANCEL").id("collection-cancel").on_click(cx.listener(|page, _, window, cx| page.close_editor(window, cx))))
            .child(
                button("SAVE")
                    .id("collection-save")
                    .border_color(accent)
                    .text_color(accent)
                    .when(!named, |save| save.opacity(0.4).cursor_default())
                    .on_click(cx.listener(|page, _, window, cx| page.keep_collection(window, cx))),
            );
        let body = v_flex()
            .p(px(16.))
            .gap(px(16.))
            .child(part("NAME").child(field))
            .child(part("ICON").child(h_flex().flex_wrap().gap(px(TILE_GAP)).children(tiles)))
            .child(
                h_flex()
                    .justify_between()
                    .child(mono(format!("{:02} MODS", editor.mods.len()), 10.5).text_color(rgb(FAINT)))
                    .child(buttons),
            );
        let title = if editor.at.is_some() { "EDIT COLLECTION" } else { "NEW COLLECTION" };
        // Six tiles across, inside the padding and the borders.
        let width = 6. * TILE + 5. * TILE_GAP + 34.;
        Some(self.dialog("collection", title, width, Self::close_editor, body, cx))
    }
}

/// A captioned part of the editor.
fn part(caption: &'static str) -> Div {
    v_flex().gap(px(8.)).child(mono(caption, 10.5).text_color(rgb(DIM)))
}
