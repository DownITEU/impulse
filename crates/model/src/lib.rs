//! What the UI, a run and the providers share, in no provider's terms.

mod process;
mod trainer;

pub use process::{Process, Rule};
pub use trainer::{Args, Cheat, Kind, Trainer};
