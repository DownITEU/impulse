//! What the UI, a run and the providers share, in no provider's terms.

mod process;
mod run;
mod trainer;

pub use process::{Process, Rule};
pub use run::{Attach, Event, Live, Runner, Session};
pub use trainer::{Args, Cheat, Kind, Trainer};
