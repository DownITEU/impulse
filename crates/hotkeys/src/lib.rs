//! Hotkeys, ported from Easymic's `Core/Hotkeys` without its input router: the chord and its names,
//! matching, capture and the keyboard and mouse hook.

mod capture;
mod chord;
mod hook;
mod matcher;
mod mouse;
mod names;

pub use capture::{Capture, Outcome, Step};
pub use chord::Chord;
pub use hook::{Hook, foreground_pid};
pub use matcher::{Binding, Fired, Matcher};
