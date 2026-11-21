//! The game's process: find it, check what may enter it, load a DLL into it and start an export.
//! No handle crosses this boundary, so it can later run as a helper inside a Proton prefix.

mod elevated;
mod handle;
mod image;
mod inject;
mod process;
mod trust;

pub use elevated::serve_elevated;
pub use image::Image;
pub use inject::load;
pub use process::{find, of, wait};
