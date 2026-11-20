//! The ABOUT tab and the links the app offers: the repository, bug reports, Discord, support and the author's address.
//! `AboutView` reaches `settings` as one of its `Tabs`, so `settings` never names this slice; the update rows reach
//! it the same way.

mod links;
mod view;

pub use links::{FEED, Link};
pub use view::{AboutView, stacked_row, tab_row};
