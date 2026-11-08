//! A trainer as the page holds it: the cheats, their printed form and the state of their controls.

use std::collections::{HashMap, HashSet};

use gpui_kit::{App, AppContext as _, Entity, SharedString, Window, component::input::InputState};
use hotkeys::Chord;
use model::{Cheat, Kind, Plan, Trainer};

use super::library::Stamp;

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

pub struct Loaded {
    pub cheats: Vec<Cheat>,
    pub rows: Vec<Row>,
    /// By cheat, then slot: the provider's bindings with the user's choices laid over them.
    pub hotkeys: Vec<Vec<Option<Chord>>>,
    /// What it was asked for; another stamp on the game means it is stale.
    pub stamp: Stamp,
    /// The disk copy was stale and its refresh failed: the next open asks again.
    pub stale: bool,
    /// One per cheat: 0 or 1 for a toggle, the option index for a selection, else the number.
    pub values: Vec<f64>,
    /// Text fields of the cheats applied with a button, by cheat index.
    pub inputs: HashMap<usize, Entity<InputState>>,
    pub pinned: HashSet<usize>,
    pub folded: HashSet<SharedString>,
    /// What running this trainer takes.
    pub plan: Plan,
}

impl Loaded {
    pub fn new(trainer: Trainer, hotkeys: Vec<Vec<Option<Chord>>>, stamp: Stamp, window: &mut Window, cx: &mut App) -> Self {
        let Trainer { cheats, pinned, plan } = trainer;
        let values: Vec<f64> = cheats.iter().map(initial).collect();
        let inputs = cheats
            .iter()
            .enumerate()
            .filter(|(_, cheat)| cheat.kind == Kind::Number && cheat.args.button)
            .map(|(index, cheat)| {
                let input = cx.new(|cx| {
                    InputState::new(window, cx)
                        .validate(|text, _| {
                            text.len() <= INPUT_DIGITS && text.bytes().all(|b| b.is_ascii_digit())
                        })
                        .default_value(number(initial(cheat)))
                });
                (index, input)
            })
            .collect();
        Self {
            rows: cheats.iter().zip(&hotkeys).map(|(cheat, slots)| row(cheat, slots)).collect(),
            hotkeys,
            pinned: (0..cheats.len())
                .filter(|&index| pinned.contains(&cheats[index].id))
                .collect(),
            cheats,
            stamp,
            stale: false,
            values,
            inputs,
            folded: HashSet::new(),
            plan,
        }
    }

    pub fn rebind(&mut self, index: usize, slots: Vec<Option<Chord>>) {
        self.rows[index].keys = caps(self.cheats[index].kind, &slots);
        self.hotkeys[index] = slots;
    }

    /// A trainer starts with every cheat at rest.
    pub fn reset(&mut self) {
        self.values = self.cheats.iter().map(initial).collect();
    }

    /// Whether the cheat was moved away from its resting value.
    pub fn is_live(&self, index: usize) -> bool {
        self.values[index] != initial(&self.cheats[index])
    }

    pub fn active(&self) -> usize {
        (0..self.cheats.len())
            .filter(|&index| self.is_live(index))
            .count()
    }
}

fn initial(cheat: &Cheat) -> f64 {
    cheat.args.min.unwrap_or(0.)
}

const UNBOUND: &str = "-";

/// A slot's cap: a pair's arrow (`Kind::slots` order), then `body`.
pub fn cap(kind: Kind, slot: usize, body: &str) -> SharedString {
    let arrow = match (kind, slot) {
        (Kind::Selection, 0) => "← ",
        (Kind::Selection, _) => "→ ",
        (Kind::Number | Kind::Slider, 0) => "↓ ",
        (Kind::Number | Kind::Slider, _) => "↑ ",
        _ => "",
    };
    format!("{arrow}{body}").into()
}

fn caps(kind: Kind, slots: &[Option<Chord>]) -> Vec<SharedString> {
    let body = |chord: &Option<Chord>| chord.map_or_else(|| UNBOUND.to_owned(), |chord| chord.to_string());
    slots.iter().enumerate().map(|(slot, chord)| cap(kind, slot, &body(chord))).collect()
}

fn row(cheat: &Cheat, slots: &[Option<Chord>]) -> Row {
    Row {
        name: cheat.name.to_uppercase().into(),
        search: cheat.name.to_lowercase(),
        category: cheat.category.to_uppercase().into(),
        keys: caps(cheat.kind, slots),
        tip: cheat
            .instructions
            .clone()
            .or_else(|| cheat.description.clone())
            .filter(|text| !text.is_empty())
            .map(SharedString::from),
        options: cheat
            .args
            .options
            .iter()
            .map(|option| option.to_uppercase().into())
            .collect(),
    }
}

/// `3`, `0.5`: one decimal only when there is a fraction.
pub fn number(value: f64) -> String {
    if value.fract() == 0. {
        format!("{value:.0}")
    } else {
        format!("{value:.1}")
    }
}

#[cfg(test)]
mod tests {
    use hotkeys::Chord;
    use model::Kind;

    use super::caps;

    #[test]
    fn every_slot_gets_a_cap_with_its_pair_arrow() {
        let chords = |all: &[&[u16]]| all.iter().map(|vks| Chord::from_vks(vks)).collect::<Vec<_>>();
        assert_eq!(caps(Kind::Number, &chords(&[&[18, 117], &[]])), ["↓ ALT+F6", "↑ -"]);
        assert_eq!(caps(Kind::Selection, &chords(&[&[112], &[113]])), ["← F1", "→ F2"]);
        assert_eq!(caps(Kind::Toggle, &chords(&[&[112]])), ["F1"]);
    }
}
