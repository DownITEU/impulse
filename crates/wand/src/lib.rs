//! Everything WeMod/Wand: its API through the browser bridge, the session handed off from Wand,
//! the catalog, trainer metadata and DLLs, Wand's install, and the Tophat host. Read `CLAUDE.md`.

mod attach;
mod binary;
mod catalog;
mod folder;
mod inject;
mod install;
mod map;
mod provider;
mod run;
mod session;
mod store;
mod tophat;
mod trainer;
mod transport;

pub use provider::Wand;

fn source() -> model::SourceId {
    model::SourceId::new(Wand::SOURCE).expect("a valid source id")
}
