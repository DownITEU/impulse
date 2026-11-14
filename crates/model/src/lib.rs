//! What the UI, a run and the providers share, in no provider's terms.

mod chord;
mod game;
mod ids;
mod map;
mod process;
mod provider;
mod run;
mod store;
mod trainer;

pub use chord::{Chord, Held, mouse};
pub use game::{Game, Installation, Locator, Offer, Standalone};
pub use ids::{Ref, SourceId};
pub use map::{Category, Frame, Glyph, Group, Label, Level, Map, MapOffer, PIN_LINK, Picture, Pin, Point, Region, Tiles};
pub use process::{Process, Rule};
pub use provider::{Account, Clear, Folder, Provider, SignIn, Snapshot, Store};
pub use run::{Attach, Event, Live, Runner, Session};
pub use store::{Origin, StoreId};
pub use trainer::{Args, Cheat, Kind, Plan, Trainer};
