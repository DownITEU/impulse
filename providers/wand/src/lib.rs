//! Everything WeMod/Wand: its API through the browser bridge, the session handed off from Wand,
//! the catalog, trainer metadata and DLLs, Wand's install, and the Tophat host. Read `CLAUDE.md`.

mod attach;
pub mod binary;
pub mod catalog;
mod inject;
pub mod install;
pub mod run;
pub mod session;
mod store;
mod tophat;
pub mod trainer;
pub mod transport;
