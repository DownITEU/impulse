//! Hotkeys, ported from Easymic's `Core/Hotkeys` without its input router: matching, capture and the
//! keyboard and mouse hook. The chord is `model::Chord`.

mod capture;
mod hook;
mod matcher;
mod mouse;

pub use capture::{Capture, Outcome, Step};
pub use hook::{Hook, foreground_pid};
pub use matcher::{Binding, Fired, Matcher};
