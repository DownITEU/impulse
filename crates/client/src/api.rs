//! The remote panel's protocol, as JSON: what the app shows a device (`View`) and what a device asks (`Command`).
//! The panel's TypeScript is generated from these types (`web/panel/src/api.ts`).

use model::{Cheat, Kind};
use serde::{Deserialize, Serialize};

use crate::trainer::Span;

mod link;

pub use link::{Ask, Feed, Seen, ServerSide, Views, coalesce, link};

/// Bumped with any change of a view's or a command's shape.
pub const VERSION: u32 = 1;

/// What the app shows; each replaces the last of its type.
#[derive(Serialize, Clone, PartialEq, Debug)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum View {
    /// First on each connection. `accent`: `#rrggbb`.
    Hello {
        version: u32,
        language: String,
        accent: String,
    },
    Library {
        games: Vec<Game>,
    },
    /// The open game's trainer. `sheet` changes with every other list; cheat commands name it.
    Trainer {
        sheet: u32,
        game: Option<u32>,
        trainer: Sheet,
    },
    /// One per cheat of `sheet`: 0 or 1 for a toggle, the option's index for a selection, else the number.
    Values {
        sheet: u32,
        values: Vec<f64>,
    },
    /// `since`: when the run went live, in Unix seconds; `why`: a failed start's reason, worded.
    Run {
        state: RunState,
        game: Option<u32>,
        since: Option<u64>,
        why: Option<String>,
    },
    /// A failure or a refusal, worded; to the device that asked.
    Note {
        text: String,
    },
}

/// A game as the library lists it; `game` is a handle that stays while the app runs.
#[derive(Serialize, Clone, PartialEq, Debug)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct Game {
    pub game: u32,
    pub name: String,
    pub favorite: bool,
    pub open: bool,
    pub live: bool,
    /// A run can start: installed, with a trainer, and no other run.
    pub startable: bool,
    /// The game has a cover: `/art/<game>` serves it once the window has shown it, else answers 404.
    pub art: bool,
}

#[derive(Serialize, Clone, PartialEq, Debug)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum Sheet {
    /// No game open, or one without a trainer.
    None,
    Pending,
    Failed {
        why: String,
    },
    Ready {
        sections: Vec<Section>,
        cheats: Vec<CheatRow>,
        collections: Vec<Collection>,
    },
}

/// A section as the window prints it, pinned first; `cheats` are indexes.
#[derive(Serialize, Clone, PartialEq, Debug)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct Section {
    pub name: String,
    pub pinned: bool,
    pub cheats: Vec<u32>,
}

#[derive(Serialize, Clone, PartialEq, Debug)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct CheatRow {
    pub name: String,
    pub description: Option<String>,
    pub instructions: Option<String>,
    pub pinned: bool,
    pub control: Control,
}

#[derive(Serialize, Clone, PartialEq, Debug)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Control {
    Toggle,
    Button,
    /// `apply`: typed, then sent with a button.
    Number {
        min: Option<f64>,
        max: Option<f64>,
        step: f64,
        apply: bool,
    },
    Slider {
        min: f64,
        max: f64,
        step: f64,
    },
    Selection {
        options: Vec<String>,
    },
    None,
}

impl Control {
    pub fn of(cheat: &Cheat) -> Self {
        let args = &cheat.args;
        match cheat.kind {
            Kind::Toggle => Self::Toggle,
            Kind::Button => Self::Button,
            Kind::Number => Self::Number { min: args.min, max: args.max, step: args.stride(), apply: args.button },
            Kind::Slider => {
                let Span { min, max, step } = Span::of(args);
                Self::Slider { min, max, step }
            }
            Kind::Selection => Self::Selection { options: args.options.clone() },
            Kind::Unknown => Self::None,
        }
    }
}

#[derive(Serialize, Clone, PartialEq, Debug)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct Collection {
    pub name: String,
    pub icon: String,
}

#[derive(Serialize, Clone, Copy, PartialEq, Debug)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum RunState {
    Idle,
    Starting,
    Live,
    Stopping,
    /// The last start failed; `Run::why` says why, until the next run.
    Failed,
}

/// What a device asks. Cheat commands name the `sheet` they were made on; another sheet refuses them.
#[derive(Deserialize, Clone, PartialEq, Debug)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Command {
    Open {
        game: u32,
    },
    Favorite {
        game: u32,
    },
    Start {
        game: u32,
    },
    /// Stops the run of `game` only.
    Stop {
        game: u32,
    },
    Set {
        sheet: u32,
        cheat: u32,
        value: f64,
    },
    Press {
        sheet: u32,
        cheat: u32,
    },
    Pin {
        sheet: u32,
        cheat: u32,
    },
    /// Every mod back at rest, as the window's RESET.
    Reset {
        sheet: u32,
    },
    /// Keeps the live values as a collection.
    Collect {
        sheet: u32,
        name: String,
    },
    Apply {
        sheet: u32,
        collection: u32,
    },
    Forget {
        sheet: u32,
        collection: u32,
    },
}

#[cfg(test)]
mod tests;
