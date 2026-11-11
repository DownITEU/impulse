//! What the UI, a run and the providers share, in no provider's terms.

mod game;
mod map;
mod process;
mod provider;
mod run;
mod trainer;

pub use game::{Game, Installation, Locator, Offer, Standalone};
pub use map::{Category, Frame, Glyph, Group, Label, Level, Map, MapOffer, PIN_LINK, Picture, Pin, Point, Region, Tiles};
pub use process::{Process, Rule};
pub use provider::{Account, Clear, Folder, Provider, SignIn, Snapshot, Store};
pub use run::{Attach, Event, Live, Runner, Session};
pub use trainer::{Args, Cheat, Kind, Plan, Trainer};
