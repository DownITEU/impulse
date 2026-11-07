//! A trainer as the page holds it: the cheats, their printed form and the state of their controls.

use std::collections::{HashMap, HashSet};

use gpui_kit::{App, AppContext as _, Entity, SharedString, Window, component::input::InputState};

use super::keys;
use crate::trainer::{Cheat, Kind, Stamp, Trainer};

const INPUT_DIGITS: usize = 6;

/// What a cheat row prints, made once so a frame allocates nothing for it.
pub struct Row {
    pub name: SharedString,
    pub search: String,
    pub category: SharedString,
    pub keys: Vec<SharedString>,
    pub tip: Option<SharedString>,
    pub options: Vec<SharedString>,
}

pub struct Loaded {
    pub cheats: Vec<Cheat>,
    pub rows: Vec<Row>,
    /// What the disk copy was fetched for; another stamp on the game means it is stale.
    pub stamp: Stamp,
    /// One per cheat: 0 or 1 for a toggle, the option index for a selection, else the number.
    pub values: Vec<f64>,
    /// Text fields of the cheats applied with a button, by cheat index.
    pub inputs: HashMap<usize, Entity<InputState>>,
    pub pinned: HashSet<usize>,
    pub folded: HashSet<SharedString>,
}

impl Loaded {
    pub fn new(trainer: Trainer, stamp: Stamp, window: &mut Window, cx: &mut App) -> Self {
        let cheats = trainer.blueprint.cheats;
        let values: Vec<f64> = cheats.iter().map(initial).collect();
        let inputs = cheats
            .iter()
            .enumerate()
            .filter(|(_, cheat)| cheat.kind == Kind::Number && cheat.args.has_button())
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
            rows: cheats.iter().map(row).collect(),
            pinned: (0..cheats.len())
                .filter(|&index| trainer.pinned_mods.contains(&cheats[index].uuid))
                .collect(),
            cheats,
            stamp,
            values,
            inputs,
            folded: HashSet::new(),
        }
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

fn row(cheat: &Cheat) -> Row {
    let chord = |keys: &Vec<u16>| keys::chord(keys);
    let keys = match cheat.hotkeys.as_slice() {
        // A pair is "decrease, increase".
        [down, up] => vec![format!("↑ {}", chord(up)), format!("↓ {}", chord(down))],
        chords => chords.iter().map(chord).collect(),
    };
    Row {
        name: cheat.name.to_uppercase().into(),
        search: cheat.name.to_lowercase(),
        category: cheat.category.to_uppercase().into(),
        keys: keys.into_iter().map(SharedString::from).collect(),
        tip: cheat
            .instructions
            .clone()
            .or_else(|| cheat.description.clone())
            .filter(|text| !text.is_empty())
            .map(SharedString::from),
        options: cheat
            .args
            .options()
            .into_iter()
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
