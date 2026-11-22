//! The app's messages, one const per message of `locales/en.ftl`, a file per UI area and its `##` sections; `checks` holds the two
//! together.

mod about;
mod cheats;
mod log_view;
mod map;
mod overlay;
mod page;
mod remote;
mod run;
mod settings;
mod update;

use i18n::Key;
pub use {about::*, cheats::*, log_view::*, map::*, overlay::*, page::*, remote::*, run::*, settings::*, update::*};

pub const OFF: Key = Key("off");
pub const ON: Key = Key("on");
pub const NUMBER_GROUP: Key = Key("number-group");
pub const NUMBER_DECIMAL: Key = Key("number-decimal");
pub const TRAY_OPEN: Key = Key("tray-open");
pub const TRAY_STOP: Key = Key("tray-stop");
pub const TRAY_OVERLAY: Key = Key("tray-overlay");
pub const TRAY_QUIT: Key = Key("tray-quit");
pub const FAILURE_OFFLINE: Key = Key("failure-offline");
pub const FAILURE_SIGNED_OUT: Key = Key("failure-signed-out");
pub const FAILURE_NOT_FOUND: Key = Key("failure-not-found");
pub const FAILURE_UNSUPPORTED: Key = Key("failure-unsupported");
pub const FAILURE_SETUP_NEEDED: Key = Key("failure-setup-needed");
pub const FAILURE_GAME_NOT_FOUND: Key = Key("failure-game-not-found");
pub const FAILURE_REFUSED: Key = Key("failure-refused");
pub const FAILURE_ELEVATION: Key = Key("failure-elevation");
pub const FAILURE_PROTECTED: Key = Key("failure-protected");
pub const FAILURE_ARCHITECTURE: Key = Key("failure-architecture");
pub const FAILURE_RESTART: Key = Key("failure-restart");
pub const FAILURE_INTERNAL: Key = Key("failure-internal");
pub const FAILURE_PLACE: Key = Key("failure-place");
