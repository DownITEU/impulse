//! Everything WeMod/Wand: its API through the browser bridge, the session handed off from Wand,
//! the catalog, trainer metadata and DLLs, Wand's install, and the Tophat host. Read `CLAUDE.md`.

mod attach;
mod binary;
mod catalog;
mod folder;
mod inject;
mod install;
mod locale;
mod map;
mod overlay;
mod provider;
mod proxy;
mod run;
mod session;
mod store;
mod texts;
mod tophat;
mod trainer;
mod trainer_host;
mod transport;

pub use provider::Wand;

const SOURCE: &str = "wand";

fn source() -> model::SourceId {
    model::SourceId::new(SOURCE).expect("a valid source id")
}
