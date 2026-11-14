//! The app's texts in the user's language: the `Texts` global over the catalog, set by `Settings`.

use std::{cell::RefCell, env};

use gpui_kit::{App, Global, SharedString};
use i18n::{Catalog, Files, Key, Language};
use rustc_hash::FxHashMap;

/// The app's own domain.
const APP: &str = "app";
const FILES: Files = &[("en", include_str!("../../locales/en.ftl"))];

pub struct Texts {
    catalog: Catalog,
    // Messages without arguments, each made once per language.
    made: RefCell<FxHashMap<&'static str, SharedString>>,
}

impl Global for Texts {}

impl Texts {
    pub fn new(language: Language) -> Self {
        // `IMPULSE_LOCALE=pseudo` in a debug build shows cuts and English left in the code.
        let pseudo = cfg!(debug_assertions) && env::var_os("IMPULSE_LOCALE").is_some_and(|value| value == "pseudo");
        Self { catalog: Catalog::new(language, pseudo, [(APP, FILES)]), made: RefCell::default() }
    }

    pub fn language(&self) -> Language {
        self.catalog.language()
    }
}

/// A message without arguments.
pub fn t(cx: &App, key: Key) -> SharedString {
    let texts = cx.global::<Texts>();
    texts.made.borrow_mut().entry(key.0).or_insert_with(|| texts.catalog.text(APP, key.0, &[]).into()).clone()
}
