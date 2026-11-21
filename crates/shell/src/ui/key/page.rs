//! The page: header, hero, library and status bar.

use i18n::Key;

pub const CATALOG_FAILED_TAG: Key = Key("catalog-failed-tag");
pub const CATALOG_FAILED: Key = Key("catalog-failed");
pub const CATALOG_FAILED_BODY: Key = Key("catalog-failed-body");
pub const LIBRARY_FILTER: Key = Key("library-filter");
pub const PICKER_FILTER: Key = Key("picker-filter");

pub const DISCLAIMER_TITLE: Key = Key("disclaimer-title");
pub const DISCLAIMER_BODY: Key = Key("disclaimer-body");
pub const DISCLAIMER_ACCEPT: Key = Key("disclaimer-accept");
pub const DISCLAIMER_EXIT: Key = Key("disclaimer-exit");

pub const SIGNED_OUT_TITLE: Key = Key("signed-out-title");
pub const SIGNED_OUT_BODY: Key = Key("signed-out-body");
pub const SIGNED_OUT_RETRY: Key = Key("signed-out-retry");
pub const SIGNED_OUT_CLOSE: Key = Key("signed-out-close");

#[expect(dead_code, reason = "the header's search is hidden until it works")]
pub const PAGE_SEARCH: Key = Key("page-search");
#[expect(dead_code, reason = "the header's search is hidden until it works")]
pub const PAGE_SEARCH_NONE: Key = Key("page-search-none");
#[expect(dead_code, reason = "the header's search is hidden until it works")]
pub const PAGE_SEARCH_KEY: Key = Key("page-search-key");
pub const PAGE_SIGNED_OUT: Key = Key("page-signed-out");
pub const PAGE_ELEVATED: Key = Key("page-elevated");
pub const PAGE_OVERLAY: Key = Key("page-overlay");
pub const PAGE_STAR: Key = Key("page-star");
pub const PAGE_FOLDER: Key = Key("page-folder");
pub const PAGE_LAUNCHER: Key = Key("page-launcher");
pub const PAGE_SOURCE: Key = Key("page-source");
pub const PAGE_CUSTOM: Key = Key("page-custom");
pub const PAGE_LIVE: Key = Key("page-live");
pub const PAGE_ACTIVE: Key = Key("page-active");
pub const PAGE_IDLE: Key = Key("page-idle");
pub const PAGE_LOG: Key = Key("page-log");
pub const PAGE_BINDING: Key = Key("page-binding");
pub const PAGE_BINDING_UNBIND: Key = Key("page-binding-unbind");
pub const PAGE_HOTKEYS_ON: Key = Key("page-hotkeys-on");
pub const PAGE_HOTKEYS_OFF: Key = Key("page-hotkeys-off");

pub const LIBRARY_FAVORITES: Key = Key("library-favorites");
pub const LIBRARY_GAMES: Key = Key("library-games");
pub const LIBRARY_NO_MATCH: Key = Key("library-no-match");
pub const LIBRARY_ADD: Key = Key("library-add");
pub const LIBRARY_LIVE: Key = Key("library-live");
pub const LIBRARY_SCANNING_TAG: Key = Key("library-scanning-tag");
pub const LIBRARY_SCANNING: Key = Key("library-scanning");
pub const LIBRARY_SCANNING_BODY: Key = Key("library-scanning-body");
pub const LIBRARY_EMPTY_TAG: Key = Key("library-empty-tag");
pub const LIBRARY_EMPTY: Key = Key("library-empty");
pub const LIBRARY_EMPTY_BODY: Key = Key("library-empty-body");
pub const LIBRARY_SOURCE_EXE: Key = Key("library-source-exe");
pub const LIBRARY_SOURCE_ADD: Key = Key("library-source-add");
pub const LIBRARY_SOURCE_CHANGE: Key = Key("library-source-change");
pub const LIBRARY_SOURCE_REMOVE: Key = Key("library-source-remove");

pub const PICKER_TITLE: Key = Key("picker-title");
pub const PICKER_OWNED: Key = Key("picker-owned");
pub const PICKER_UNOWNED: Key = Key("picker-unowned");
pub const PICKER_IN_LIBRARY: Key = Key("picker-in-library");
pub const PICKER_PROMPT: Key = Key("picker-prompt");
