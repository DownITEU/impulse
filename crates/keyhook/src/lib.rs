//! Hotkeys, ported from Easymic's `Core/Hotkeys` without its input router: matching, capture and the
//! keyboard and mouse hook, for every hotkey of the app. A trigger is a `model::Chord` or a `Key`.

mod capture;
mod hook;
mod key;
mod matcher;
mod mouse;

pub use capture::{Capture, Outcome, Step};
pub use hook::{Hook, Stroke, caps_lock, foreground_pid};
pub use key::{Key, Modifiers};
pub use matcher::{Binding, Matcher, Trigger};
