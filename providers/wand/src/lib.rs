//! Everything WeMod/Wand: its API through the browser bridge, the session handed off from Wand,
//! the catalog, trainer metadata and DLLs, Wand's install, and the Tophat host. Read `CLAUDE.md`.

pub mod attach;
pub mod binary;
pub mod catalog;
pub mod inject;
pub mod install;
pub mod session;
pub mod store;
pub mod tophat;
pub mod trainer;
pub mod transport;
