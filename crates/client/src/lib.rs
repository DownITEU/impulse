//! The app's logic without GPUI: the sources' registry, the library, a trainer's sheet, runs, hotkey choices, map
//! state and the files that keep them. `shell`'s model entities hold these and run their async work.

pub mod bindings;
pub mod library;
pub mod map;
pub mod media;
pub mod profile;
mod registry;
pub mod run;
pub mod storage;
pub mod trainer;

pub use registry::Registry;

/// Something that arrives after the window opens.
pub enum Load<T> {
    Pending,
    Ready(T),
    Failed(String),
}
