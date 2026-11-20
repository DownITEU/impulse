//! The open game's trainer as the cheat list prints it: its rows, its number fields and the control column's width.

use std::collections::HashMap;

use client::{library::GameId, trainer::Sheet};
use gpui_kit::{App, AppContext as _, Entity, SharedString, Window, component::input::InputState};
use model::{Cheat, Chord, Kind};

use crate::{bindings::cap, ui::text::text};

const INPUT_DIGITS: usize = 6;

/// What a cheat row prints, made once so a frame allocates nothing for it.
pub struct Row {
    pub name: SharedString,
    pub search: String,
    pub category: SharedString,
    /// One cap per hotkey slot.
    pub keys: Vec<SharedString>,
    pub tip: Option<SharedString>,
    pub options: Vec<SharedString>,
}

pub struct Printed {
    pub game: GameId,
    pub rows: Vec<Row>,
    /// Text fields of the cheats applied with a button, by cheat index.
    pub inputs: HashMap<usize, Entity<InputState>>,
    /// The control column's least width: the widest control here, so the caps sit next to it. A longer button text widens
    /// its own row instead of running over the caps.
    pub control_width: f32,
}

impl Printed {
    pub fn new(game: GameId, sheet: &Sheet, window: &mut Window, cx: &mut App) -> Self {
        let inputs = sheet
            .cheats
            .iter()
            .enumerate()
            .filter(|(_, cheat)| cheat.kind == Kind::Number && cheat.args.button)
            .map(|(index, _)| {
                let input = cx.new(|cx| {
                    InputState::new(window, cx)
                        .validate(|text, _| text.len() <= INPUT_DIGITS && text.bytes().all(|b| b.is_ascii_digit()))
                        .default_value(number(sheet.values[index]))
                });
                (index, input)
            })
            .collect();
        Self {
            game,
            rows: sheet.cheats.iter().zip(&sheet.hotkeys).map(|(cheat, slots)| row(cheat, slots, cx)).collect(),
            control_width: sheet.cheats.iter().map(control_width).fold(0., f32::max),
            inputs,
        }
    }

    pub fn recap(&mut self, sheet: &Sheet) {
        for ((row, cheat), slots) in self.rows.iter_mut().zip(&sheet.cheats).zip(&sheet.hotkeys) {
            row.keys = caps(cheat.kind, slots);
        }
    }

    /// A cheat's text field follows its value.
    pub fn put(&self, index: usize, value: f64, window: &mut Window, cx: &mut App) {
        if let Some(input) = self.inputs.get(&index) {
            input.update(cx, |input, cx| input.set_value(number(value), window, cx));
        }
    }
}

/// What `CheatList::control` draws for the cheat, across.
fn control_width(cheat: &Cheat) -> f32 {
    match cheat.kind {
        Kind::Toggle => 70.,
        Kind::Slider => 134.,
        Kind::Number if cheat.args.button => 140.,
        Kind::Number => 120.,
        Kind::Button => 100.,
        Kind::Selection => 112.,
        Kind::Unknown => 0.,
    }
}

const UNBOUND: &str = "-";

fn caps(kind: Kind, slots: &[Option<Chord>]) -> Vec<SharedString> {
    let body = |chord: &Option<Chord>| chord.map_or_else(|| UNBOUND.to_owned(), |chord| chord.to_string());
    slots.iter().enumerate().map(|(slot, chord)| cap(kind, slot, &body(chord))).collect()
}

/// A cheat's section as printed: the source's worded name, else its id.
pub fn category(cheat: &Cheat, cx: &App) -> SharedString {
    cheat.category_name.as_ref().map_or_else(|| cheat.category.to_uppercase().into(), |name| text(cx, name))
}

fn row(cheat: &Cheat, slots: &[Option<Chord>], cx: &App) -> Row {
    Row {
        name: cheat.name.to_uppercase().into(),
        search: cheat.name.to_lowercase(),
        category: category(cheat, cx),
        keys: caps(cheat.kind, slots),
        tip: cheat.instructions.clone().or_else(|| cheat.description.clone()).filter(|text| !text.is_empty()).map(SharedString::from),
        options: cheat.args.options.iter().map(|option| option.to_uppercase().into()).collect(),
    }
}

/// `3`, `0.5`: one decimal only when there is a fraction.
pub fn number(value: f64) -> String {
    if value.fract() == 0. { format!("{value:.0}") } else { format!("{value:.1}") }
}

#[cfg(test)]
mod tests {
    use model::{Chord, Kind};

    use super::caps;

    #[test]
    fn every_slot_gets_a_cap_with_its_pair_arrow() {
        let chords = |all: &[&[u16]]| all.iter().map(|vks| Chord::from_vks(vks)).collect::<Vec<_>>();
        assert_eq!(caps(Kind::Number, &chords(&[&[18, 117], &[]])), ["↓ ALT+F6", "↑ -"]);
        assert_eq!(caps(Kind::Selection, &chords(&[&[112], &[113]])), ["← F1", "→ F2"]);
        assert_eq!(caps(Kind::Toggle, &chords(&[&[112]])), ["F1"]);
    }
}
