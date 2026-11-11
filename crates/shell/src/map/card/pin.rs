//! The selected pin's card: its picture or clip, what it is, where, and the pins near it.

use gpui_kit::{
    AnyElement, ClipboardItem, Context, FontWeight, Window,
    component::{h_flex, text::TextView, v_flex},
    div, img,
    prelude::*,
    px, rgb,
};
use model::{Map, PIN_LINK};

use super::{Card, PICTURE};
use crate::{
    Load,
    assets::Icon,
    map::{
        CARD_WIDTH,
        board::Board,
        group, kind,
        markers::{ink, legible},
        picture::{fill, picture},
    },
    theme::*,
    widgets::{button, flash, icon, icon_button, mono},
};

const BADGE: f32 = 40.;
const NEARBY: usize = 4;

/// The pins closest to the selected one, closest first: one pass, nothing the size of the map kept.
pub(super) fn nearest(board: &Board) -> Vec<usize> {
    let (Load::Ready(map), Some(index)) = (&board.map, board.selected) else { return Vec::new() };
    let at = map.pins[index].at;
    let mut near: Vec<(f64, usize)> = Vec::with_capacity(NEARBY + 1);
    for (other, pin) in map.pins.iter().enumerate().filter(|&(other, _)| other != index) {
        let distance = (pin.at.x - at.x).powi(2) + (pin.at.y - at.y).powi(2);
        if near.len() < NEARBY || distance < near[NEARBY - 1].0 {
            near.insert(near.partition_point(|&(closer, _)| closer <= distance), (distance, other));
            near.truncate(NEARBY);
        }
    }
    near.into_iter().map(|(_, other)| other).collect()
}

impl Card {
    pub(super) fn pin_card(&mut self, map: &Map, board: &Board, window: &Window, cx: &Context<Self>) -> Option<AnyElement> {
        let index = board.selected?;
        let pin = &map.pins[index];
        let category = &map.categories[pin.group.0];
        let found = board.saved.found.contains(&pin.id);
        let teleport = pin.position.filter(|_| board.can_teleport(map));
        let accent = rgb(board.accent);
        let badge = h_flex()
            .size(px(BADGE))
            .flex_none()
            .justify_center()
            .bg(rgb(category.color))
            .border_2()
            .border_color(rgb(PANEL))
            .child(self.images.glyph(map, pin.group, ink(category.color), 20., window, cx));
        // Under the lightbox the card's clip does not play.
        let clip = self.clip.as_ref().filter(|_| !self.lightbox).map(|clip| clip.view.clone().into_any_element());
        let preview = pin.image.as_ref().map(|image| fill(img(image.preview.clone()), ("pin-media", index)));
        let shown = (pin.image.is_some() || pin.video.is_some()).then(|| {
            div()
                .id("pin-picture")
                .relative()
                .h(px(PICTURE))
                .flex_none()
                .cursor_pointer()
                .bg(rgb(SUNKEN))
                .child(picture(index, preview.into_iter().chain(clip), (CARD_WIDTH - 2., PICTURE)))
                .on_click(cx.listener(|card, _, _, cx| {
                    (card.lightbox, card.photo) = (true, false);
                    cx.notify();
                }))
        });
        let close = icon_button("pin-close", Icon::Close, 28., 11.).bg(alpha(PANEL, 0.85)).on_click(cx.listener(|card, _, _, cx| {
            card.board.update(cx, |board, cx| board.deselect(cx));
        }));
        let title = v_flex()
            .flex_1()
            .min_w_0()
            .gap(px(3.))
            .child(div().font_weight(FontWeight::BOLD).text_size(px(16.)).text_color(rgb(TEXT_BRIGHT)).child(pin.name.clone()))
            .child(mono(kind(map, pin), 10.).text_color(rgb(DIM)));
        let head = h_flex().items_end().gap(px(12.)).px(px(14.)).child(badge).child(title);
        let head = match &shown {
            Some(_) => head.mt(px(-BADGE / 2.)),
            None => head.pt(px(14.)).pr(px(36.)),
        };
        let coordinates = pin.position.map(|[x, y, z]| {
            let text = format!("X {x:.0}  Y {y:.0}  Z {z:.0}");
            let copy = text.clone();
            h_flex()
                .id("pin-coordinates")
                .mx(px(14.))
                .px(px(8.))
                .py(px(4.))
                .gap(px(8.))
                .cursor_pointer()
                .border_1()
                .border_color(line(0.14))
                .bg(rgb(SUNKEN))
                .hover(|style| style.border_color(line(0.3)))
                .child(mono(text, 10.5).flex_1().text_color(rgb(MUTED)))
                .child(mono(if self.copied { "COPIED" } else { "COPY" }, 9.5).text_color(if self.copied { accent } else { rgb(DIM) }))
                .on_click(cx.listener(move |card, _, _, cx| {
                    cx.write_to_clipboard(ClipboardItem::new_string(copy.clone()));
                    card.copied = true;
                    card._flash = flash(cx, |card| card.copied = false);
                    cx.notify();
                }))
        });
        let mark = button(if found { "FOUND" } else { "MARK FOUND" })
            .id("pin-found")
            .flex_1()
            .when(found, |button| button.border_color(accent).text_color(accent).child(icon(Icon::Check, 11., accent)))
            .on_click(cx.listener(move |card, _, _, cx| card.board.update(cx, |board, cx| board.toggle_found(index, cx))));
        let go = teleport.map(|to| {
            button(if self.confirm { "CONFIRM" } else { "TELEPORT" })
                .id("pin-teleport")
                .flex_1()
                .when(self.confirm, |button| button.border_color(accent).text_color(accent))
                .on_click(cx.listener(move |card, _, _, cx| {
                    if card.confirm {
                        card.confirm = false;
                        card.board.update(cx, |board, cx| board.teleport(to, cx));
                    } else {
                        card.confirm = true;
                        cx.notify();
                    }
                }))
        });
        let hide_id = group(map, pin).id.clone();
        let hide = h_flex()
            .id("pin-hide-group")
            .px(px(14.))
            .pb(px(12.))
            .gap(px(6.))
            .cursor_pointer()
            .text_color(rgb(FAINT))
            .hover(|style| style.text_color(rgb(MUTED)))
            .child(mono(format!("HIDE {} ON THE MAP", group(map, pin).name.to_uppercase()), 9.5))
            .on_click(cx.listener(move |card, _, _, cx| {
                card.board.update(cx, |board, cx| {
                    board.filter(
                        |saved| {
                            saved.hidden.insert(hide_id.clone());
                        },
                        cx,
                    )
                })
            }));
        let card = self
            .panel("pin-card")
            .children(shown)
            .child(head)
            .children(coordinates)
            .when(!pin.description.is_empty(), |card| {
                let board = self.board.downgrade();
                let text = TextView::markdown(("pin-text", index), pin.description.clone()).on_link_click(move |url, _, _, cx| {
                    match url.strip_prefix(PIN_LINK) {
                        Some(id) => {
                            let id = id.to_owned();
                            board.update(cx, |board, cx| board.select_id(&id, cx)).ok();
                        }
                        // Links into other apps are not followed from here.
                        None if url.starts_with("https://") || url.starts_with("http://") => cx.open_url(url),
                        None => {}
                    }
                });
                card.child(div().px(px(14.)).text_size(px(13.)).text_color(rgb(TEXT_2)).child(text))
            })
            .child(h_flex().gap(px(8.)).px(px(14.)).child(mark).children(go))
            .children(self.nearby(map, window, cx))
            .child(hide)
            .child(div().absolute().top(px(6.)).right(px(6.)).child(close));
        Some(card.into_any_element())
    }

    /// The pins closest to the selected one, a click away.
    fn nearby(&mut self, map: &Map, window: &Window, cx: &Context<Self>) -> Option<AnyElement> {
        if self.near.is_empty() {
            return None;
        }
        let rows: Vec<_> = self
            .near
            .iter()
            .map(|&other| {
                let pin = &map.pins[other];
                let color = legible(map.categories[pin.group.0].color);
                h_flex()
                    .id(("pin-nearby", other))
                    .px(px(14.))
                    .py(px(4.))
                    .gap(px(10.))
                    .cursor_pointer()
                    .hover(|style| style.bg(rgb(ROW_HOVER)))
                    .child(self.images.glyph(map, pin.group, color, 14., window, cx))
                    .child(div().flex_1().min_w_0().truncate().text_size(px(12.)).text_color(rgb(TEXT_2)).child(pin.name.clone()))
                    .child(mono(group(map, pin).name.to_uppercase(), 9.).text_color(rgb(FAINT)))
                    .on_click(cx.listener(move |card, _, _, cx| card.board.update(cx, |board, cx| board.select(other, true, cx))))
            })
            .collect();
        Some(v_flex().child(mono("NEARBY", 10.).text_color(rgb(DIM)).px(px(14.)).pb(px(4.))).children(rows).into_any_element())
    }
}

#[cfg(test)]
mod tests {
    use std::{path::PathBuf, rc::Rc};

    use model::{Map, Pin, Point, Tiles};

    use super::*;

    #[test]
    fn the_nearest_pins_come_closest_first() {
        let pin = |x: f64| Pin {
            id: String::new(),
            at: Point { x, y: 0.5 },
            name: String::new(),
            description: String::new(),
            group: (0, 0),
            image: None,
            video: None,
            position: None,
            level: None,
        };
        let tiles = Tiles { template: String::new(), flipped: false, size: 256, min_zoom: 0, max_zoom: 0 };
        let pins = [0.5, 0.9, 0.52, 0.3, 0.45, 0.6, 0.51].map(pin).into();
        let map = Map { tiles, background: 0, categories: Vec::new(), pins, regions: Vec::new(), labels: Vec::new(), frame: None, teleport: false, unfinished: false };
        let mut board = Board::new(PathBuf::new(), 0);
        board.map = Load::Ready(Rc::new(map));
        assert!(nearest(&board).is_empty());
        board.selected = Some(0);
        assert_eq!(nearest(&board), [6, 2, 4, 5]);
    }
}
