//! Wand's messages, one const per message of `locales/en.ftl`; `checks` holds the two together.

use model::Text;

pub const FILES: model::Files = &[("en", include_str!("../locales/en.ftl"))];

pub struct Key(&'static str);

pub const STORE_MAPS: Key = Key("store-maps");
pub const STORE_MAPS_HINT: Key = Key("store-maps-hint");
pub const STORE_CATALOG: Key = Key("store-catalog");
pub const STORE_CATALOG_HINT: Key = Key("store-catalog-hint");
pub const STORE_TRAINERS: Key = Key("store-trainers");
pub const STORE_TRAINERS_HINT: Key = Key("store-trainers-hint");
pub const STORE_TRAINER_FILES: Key = Key("store-trainer-files");
pub const STORE_TRAINER_FILES_HINT: Key = Key("store-trainer-files-hint");
pub const STORE_BROWSER: Key = Key("store-browser");
pub const STORE_BROWSER_HINT: Key = Key("store-browser-hint");
pub const FOLDER: Key = Key("folder");
pub const FOLDER_HINT: Key = Key("folder-hint");
pub const FOLDER_EMPTY: Key = Key("folder-empty");
pub const FOLDER_UNSET: Key = Key("folder-unset");
pub const SIGNED_OUT: Key = Key("signed-out");

pub fn text(key: Key) -> Text {
    Text::key(&crate::source(), key.0)
}
