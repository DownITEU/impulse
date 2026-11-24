//! The game's process: find it, check what may enter it, load a DLL into it and start an export.
//! No handle crosses this boundary, so it can later run as a helper inside a Proton prefix.

mod elevated;
mod event;
mod exit;
mod handle;
mod image;
mod inject;
mod pause;
mod process;
mod trust;

pub use elevated::{elevated, serve_elevated};
pub use event::signaled;
pub use exit::{Exit, exit};
pub use image::{Image, Program};
pub use inject::{load, map};
pub use pause::{idle, resume, suspend};
pub use process::{of, settled, wait, x64};
pub use trust::check as signed;
