//! The app's texts in the user's language: the `Texts` global over the catalogs, set by `Settings`.

use std::{cell::RefCell, env, sync::Arc};

use gpui_kit::{App, Global, SharedString};
use i18n::{Arg, Catalog, Key, Language};
use model::{Failure, FailureKind, Files, SourceId, Text};
use rustc_hash::FxHashMap;

use super::key;

const APP: &str = SourceId::APP;
const FILES: Files = &[("en", include_str!("../../locales/en.ftl"))];

pub struct Texts {
    catalog: Catalog,
    // Messages without arguments by domain, each made once per language.
    made: RefCell<FxHashMap<String, FxHashMap<Arc<str>, SharedString>>>,
}

impl Global for Texts {}

impl Texts {
    /// The app's catalog, then each source's under its id.
    pub fn new(language: Language, sources: &[(SourceId, Files)]) -> Self {
        // `IMPULSE_LOCALE=pseudo` in a debug build shows cuts and English left in the code.
        let pseudo = cfg!(debug_assertions) && env::var_os("IMPULSE_LOCALE").is_some_and(|value| value == "pseudo");
        let domains = [(APP, FILES)].into_iter().chain(sources.iter().map(|(id, files)| (id.as_str(), *files)));
        Self { catalog: Catalog::new(language, pseudo, domains), made: RefCell::default() }
    }

    pub fn language(&self) -> Language {
        self.catalog.language()
    }

    fn made(&self, domain: &str, key: &str) -> SharedString {
        let mut made = self.made.borrow_mut();
        if let Some(text) = made.get(domain).and_then(|keys| keys.get(key)) {
            return text.clone();
        }
        let text: SharedString = self.catalog.text(domain, key, &[]).into();
        made.entry(domain.to_owned()).or_default().insert(key.into(), text.clone());
        text
    }
}

/// A message without arguments.
pub fn t(cx: &App, key: Key) -> SharedString {
    cx.global::<Texts>().made(APP, key.0)
}

/// A message with arguments: made when its data changes, or while drawing a rare state.
pub fn tf(cx: &App, key: Key, args: &[(&str, Arg)]) -> SharedString {
    cx.global::<Texts>().catalog.text(APP, key.0, args).into()
}

/// The app's own message as a `Text`, for what a source would say.
pub fn app_text(key: Key) -> Text {
    Text::key(&SourceId::app(), key.0)
}

pub fn text(cx: &App, text: &Text) -> SharedString {
    match text {
        Text::Plain(plain) => plain.clone().into(),
        Text::Key { domain, key, args } if args.is_empty() => cx.global::<Texts>().made(domain.as_str(), key),
        Text::Key { domain, key, args } => {
            let args: Vec<(&str, Arg)> = args
                .iter()
                .map(|(name, arg)| match arg {
                    model::Arg::Str(value) => (&**name, Arg::Str(value)),
                    model::Arg::Num(value) => (&**name, Arg::Num(*value)),
                })
                .collect();
            cx.global::<Texts>().catalog.text(domain.as_str(), key, &args).into()
        }
    }
}

/// A message whose `why` is the failure, worded.
pub fn failed(cx: &App, key: Key, failure: &Failure) -> SharedString {
    tf(cx, key, &[("why", Arg::Str(&why(cx, failure)))])
}

/// What went wrong: the source's words, else its kind's.
pub fn said(cx: &App, failure: &Failure) -> SharedString {
    failure.message.as_ref().map_or_else(|| t(cx, kind(failure.kind)), |message| text(cx, message))
}

/// What went wrong and, for a missing setting, where to fix it.
pub fn why(cx: &App, failure: &Failure) -> SharedString {
    match (failure.kind, &failure.message) {
        (FailureKind::SetupNeeded, Some(_)) => tf(cx, key::FAILURE_PLACE, &[("why", Arg::Str(&said(cx, failure)))]),
        _ => said(cx, failure),
    }
}

fn kind(kind: FailureKind) -> Key {
    match kind {
        FailureKind::Offline => key::FAILURE_OFFLINE,
        FailureKind::SignedOut => key::FAILURE_SIGNED_OUT,
        FailureKind::NotFound => key::FAILURE_NOT_FOUND,
        FailureKind::Unsupported => key::FAILURE_UNSUPPORTED,
        FailureKind::SetupNeeded => key::FAILURE_SETUP_NEEDED,
        FailureKind::GameNotFound => key::FAILURE_GAME_NOT_FOUND,
        FailureKind::Refused => key::FAILURE_REFUSED,
        FailureKind::Internal => key::FAILURE_INTERNAL,
    }
}
