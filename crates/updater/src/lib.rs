//! Updating Impulse: what a release feed offers, the download checked against its digest, the swap of the
//! running executable, and the sections of `CHANGELOG.md`. No GPUI: blocking calls, run off the UI thread.

mod changelog;
mod feed;
mod install;

pub use changelog::{Section, base, sections};
pub use feed::{Channel, Check, Fetched, Release, check, newer, parse};
pub use install::{cleanup, download, restart, restarted, swap};
pub use semver::Version;

/// The running build's version.
pub fn current() -> Version {
    Version::parse(env!("CARGO_PKG_VERSION")).expect("the workspace version is semver")
}
