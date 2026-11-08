use hotkeys::Chord;

/// A trainer's cheat list, as the user sees it.
pub struct Trainer {
    pub cheats: Vec<Cheat>,
    /// Ids of the cheats the user pinned.
    pub pinned: Vec<String>,
}

pub struct Cheat {
    /// Unique within the trainer; pins refer to it.
    pub id: String,
    /// The variable the trainer sets; cheats may share one, and `None` sets nothing.
    pub key: Option<String>,
    pub name: String,
    pub category: String,
    pub kind: Kind,
    pub args: Args,
    /// By slot, so a pair stays "decrease, increase" when one side is unset or unusable (`None`).
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
