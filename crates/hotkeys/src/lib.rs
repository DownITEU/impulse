//! Hotkeys, ported from Easymic's `Core/Hotkeys` without its input router: the chord and its names,
//! matching, capture and the keyboard hook.

mod capture;
mod chord;
mod hook;
mod matcher;
mod names;

pub use capture::{Capture, Step};
pub use chord::Chord;
pub use hook::Hook;
pub use matcher::{Binding, Fired, Matcher};
