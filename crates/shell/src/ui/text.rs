//! The app's texts in the user's language: the `Texts` global over the catalogs, set by `Settings`.

use std::{
    cell::RefCell,
    env,
    hash::{Hash, Hasher},
    sync::Arc,
};

use gpui_kit::{App, Global, SharedString};
use i18n::{Arg, Catalog, Key, Language};
use model::{Failure, FailureKind, Files, Locale, SourceId, Text};
use rustc_hash::{FxHashMap, FxHasher};

use super::key;

const APP: &str = SourceId::APP;
const FILES: Files = &[("en", include_str!("../../locales/en.ftl"))];

pub struct Texts {
    catalog: Catalog,
    // What sources are asked in: the language's tag.
    locale: Locale,
    // Messages without arguments by domain, each made once per language.
    made: RefCell<FxHashMap<String, FxHashMap<Arc<str>, SharedString>>>,
    // Messages with arguments by a hash of key and arguments, checked against the copy kept, so a redraw formats nothing.
    formatted: RefCell<FxHashMap<u64, (Made, SharedString)>>,
}

/// A message's key and arguments, owned.
#[derive(PartialEq)]
struct Made {
    key: &'static str,
    args: Vec<(String, Owned)>,
}

#[derive(PartialEq)]
enum Owned {
    Str(String),
    Num(u64),
}

/// Bounds the formatted messages kept; past it they are made again.
const FORMATTED: usize = 1024;

impl Global for Texts {}

impl Texts {
    /// The app's catalog, then each source's under its id.
    pub fn new(language: Language, sources: &[(SourceId, Files)]) -> Self {
        // `IMPULSE_LOCALE=pseudo` in a debug build shows cuts and English left in the code.
        let pseudo = cfg!(debug_assertions) && env::var_os("IMPULSE_LOCALE").is_some_and(|value| value == "pseudo");
        let domains = [(APP, FILES)].into_iter().chain(sources.iter().map(|(id, files)| (id.as_str(), *files)));
        let (locale, made, formatted) = (Locale::new(language.tag), RefCell::default(), RefCell::default());
        Self { catalog: Catalog::new(language, pseudo, domains), locale, made, formatted }
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

    fn formatted(&self, key: Key, args: &[(&str, Arg)]) -> SharedString {
        let mut hasher = FxHasher::default();
        key.0.hash(&mut hasher);
        for (name, arg) in args {
            name.hash(&mut hasher);
            match arg {
                Arg::Str(text) => text.hash(&mut hasher),
                Arg::Num(number) => number.to_bits().hash(&mut hasher),
            }
        }
        let hash = hasher.finish();
        let same = |made: &Made| {
            made.key == key.0
                && made.args.len() == args.len()
                && made.args.iter().zip(args).all(|((name, kept), (given, arg))| {
                    name == given
                        && match (kept, arg) {
                            (Owned::Str(kept), Arg::Str(text)) => kept == text,
                            (Owned::Num(kept), Arg::Num(number)) => *kept == number.to_bits(),
                            _ => false,
                        }
                })
        };
        let mut formatted = self.formatted.borrow_mut();
        if let Some((_, text)) = formatted.get(&hash).filter(|(made, _)| same(made)) {
            return text.clone();
        }
        if formatted.len() >= FORMATTED {
            formatted.clear();
        }
        let text: SharedString = self.catalog.text(APP, key.0, args).into();
        let owned = |arg: &Arg| match arg {
            Arg::Str(text) => Owned::Str((*text).to_owned()),
            Arg::Num(number) => Owned::Num(number.to_bits()),
        };
        let made = Made { key: key.0, args: args.iter().map(|(name, arg)| ((*name).to_owned(), owned(arg))).collect() };
        formatted.insert(hash, (made, text.clone()));
        text
    }
}

/// The language content is asked in.
pub fn content_locale(cx: &App) -> Locale {
    cx.global::<Texts>().locale.clone()
}

/// A message without arguments.
pub fn t(cx: &App, key: Key) -> SharedString {
    cx.global::<Texts>().made(APP, key.0)
}

/// A message with arguments, formatted once per distinct arguments.
pub fn tf(cx: &App, key: Key, args: &[(&str, Arg)]) -> SharedString {
    cx.global::<Texts>().formatted(key, args)
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

#[cfg(test)]
mod tests {
    use i18n::{Arg, ENGLISH};

    use super::{Texts, key};

    #[test]
    fn a_message_is_made_again_only_for_other_arguments() {
        let texts = Texts::new(ENGLISH, &[]);
        let made = |count: f64| texts.formatted(key::TRAINER_MODS, &[("count", Arg::Num(count))]);
        assert_eq!(made(28.).as_ref(), "28 MODS");
        assert_eq!(made(28.).as_ref(), "28 MODS");
        assert_eq!(made(3.).as_ref(), "3 MODS");
        assert_eq!(texts.formatted.borrow().len(), 2);
    }
}
