//! What the UI, a run and the providers share, in no provider's terms.

mod game;
mod process;
mod provider;
mod run;
mod trainer;

pub use game::{Game, Installation, Locator, Offer, Standalone};
pub use process::{Process, Rule};
pub use provider::{Account, Folder, Provider, SignIn, Snapshot};
pub use run::{Attach, Event, Live, Runner, Session};
pub use trainer::{Args, Cheat, Kind, Plan, Trainer};
