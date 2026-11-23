//! What the UI, a run and the sources share, in no source's terms.

mod chord;
mod failure;
mod game;
mod ids;
mod map;
mod overlay;
mod plugin;
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
pub use map::{Category, Frame, Glyph, Group, Icon, Label, Level, Map, MapOffer, PIN_LINK, Picture, Pin, Point, Region, TileSet, Tiles};
pub use overlay::{Canvas, CanvasEvent, Cursor, Layers, Overlays, Rect, Texture};
pub use plugin::{Approval, Installed, Setting, SettingKind, Value};
pub use process::{Process, Rule};
pub use run::{Attach, Event, Live, Runner, Session};
pub use source::{
    Account, Accounts, Caches, Clear, Downloads, Files, Folder, Folders, Games, Maps, SignIn, Snapshot, Source, Store, Title, Titles,
    Trainers,
};
pub use store::{Origin, StoreId};
pub use text::{Arg, Locale, Text};
pub use trainer::{Args, Cheat, Kind, Launch, Plan, Trainer};
