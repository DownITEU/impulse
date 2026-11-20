//! The app's logic without GPUI: the sources' registry, the library, a trainer's sheet, runs, hotkey choices, map
//! state, the remote panel's protocol and devices, and the files that keep them. `shell`'s model entities hold these
//! and run their async work.

pub mod api;
pub mod bindings;
pub mod library;
pub mod map;
pub mod media;
pub mod plugins;
pub mod profile;
mod registry;
pub mod remote;
pub mod run;
pub mod storage;
pub mod trainer;

pub use registry::{Capabilities, Change, Registry};

pub enum Load<T> {
    Pending,
    Ready(T),
    Failed(model::Failure),
}
