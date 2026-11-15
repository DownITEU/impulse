//! What the UI, a run and the sources share, in no source's terms.

mod chord;
mod failure;
mod game;
mod ids;
mod map;
mod process;
mod run;
mod source;
mod store;
mod text;
mod trainer;

pub use chord::{Chord, Held, mouse};
pub use failure::{Failure, FailureKind};
pub use game::{Game, Installation, Locator, Offer, Standalone};
pub use ids::{Ref, SourceId};
pub use map::{Category, Frame, Glyph, Group, Label, Level, Map, MapOffer, PIN_LINK, Picture, Pin, Point, Region, Tiles};
pub use process::{Process, Rule};
pub use run::{Attach, Event, Live, Runner, Session};
pub use source::{Account, Accounts, Caches, Clear, Files, Folder, Folders, Games, Maps, SignIn, Snapshot, Source, Store, Trainers};
pub use store::{Origin, StoreId};
pub use text::{Arg, Text};
pub use trainer::{Args, Cheat, Kind, Plan, Trainer};
