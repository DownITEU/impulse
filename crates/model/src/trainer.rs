use std::{sync::Arc, time::Duration};

use crate::{Chord, Runner, Text};

/// A trainer's cheat list, as the user sees it, and what running exactly this list takes.
pub struct Trainer {
    pub cheats: Vec<Cheat>,
    /// Ids of the cheats the user pinned.
    pub pinned: Vec<String>,
    /// Whether a run of it tells where the player is, and can move the player.
    pub location: bool,
    pub teleport: bool,
    pub plan: Plan,
}

#[derive(Clone)]
pub struct Plan {
    /// Path suffix of the game's executable, `*` allowed; without it the install folder decides.
    pub image: Option<String>,
    /// A younger process is still starting.
    pub minimum_age: Duration,
    /// Whether to start the game; `false` waits for the user to start it.
    pub launch: bool,
    pub runner: Arc<dyn Runner>,
}

pub struct Cheat {
    /// Unique within the trainer; pins refer to it.
    pub id: String,
    /// The variable the trainer sets; cheats may share one, and `None` sets nothing.
    pub key: Option<String>,
    pub name: String,
    /// Its section's id.
    pub category: String,
    /// The category in words; without them the category itself shows.
    pub category_name: Option<Text>,
    pub kind: Kind,
    pub args: Args,
    /// By slot (see `Kind::slots`), so a pair stays "decrease, increase" when one side is unset or unusable (`None`).
    pub hotkeys: Vec<Option<Chord>>,
    pub instructions: Option<String>,
    pub description: Option<String>,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Kind {
    Toggle,
    Number,
    Slider,
    Button,
    Selection,
    Unknown,
}

impl Kind {
    /// Hotkey slots: a toggle flips, a button applies, a pair steps down and up (a selection wraps).
    pub fn slots(self) -> usize {
        match self {
            Self::Toggle | Self::Button => 1,
            Self::Number | Self::Slider | Self::Selection => 2,
            Self::Unknown => 0,
        }
    }
}

#[derive(Default, Debug, PartialEq)]
pub struct Args {
    pub min: Option<f64>,
    pub max: Option<f64>,
    pub step: Option<f64>,
    /// A selection's labels; the value sent is the index.
    pub options: Vec<String>,
    /// A number typed, then applied with a button.
    pub button: bool,
    /// What a button cheat sends when pressed.
    pub pressed: f64,
}

impl Args {
    /// Never panics: trainers may send `min > max`.
    pub fn clamp(&self, value: f64) -> f64 {
        value.max(self.min.unwrap_or(f64::MIN)).min(self.max.unwrap_or(f64::MAX))
    }
}
