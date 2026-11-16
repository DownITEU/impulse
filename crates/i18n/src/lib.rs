//! The languages the app ships and its catalogs: Fluent messages by domain, over English.

mod pseudo;

use fluent_bundle::{FluentArgs, FluentBundle, FluentResource, FluentValue};
use unic_langid::LanguageIdentifier;

/// A language we ship.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Language {
    /// A BCP 47 tag, also the name of a domain's file.
    pub tag: &'static str,
    /// In itself, as a menu shows it.
    pub name: &'static str,
}

pub const ENGLISH: Language = Language { tag: "en", name: "ENGLISH" };
pub const FRENCH: Language = Language { tag: "fr", name: "FRANÇAIS" };

/// What a language menu lists.
pub const LANGUAGES: &[Language] = &[ENGLISH, FRENCH];

impl Language {
    pub fn find(tag: &str) -> Option<Self> {
        LANGUAGES.iter().copied().find(|language| language.tag == tag)
    }

    /// The first of the system's preferences we ship, matched by language alone; else English.
    pub fn system() -> Self {
        sys_locale::get_locales().find_map(|tag| closest(&tag)).unwrap_or(ENGLISH)
    }
}

fn closest(tag: &str) -> Option<Language> {
    let language = |tag: &str| tag.split(['-', '_']).next().unwrap_or_default().to_ascii_lowercase();
    LANGUAGES.iter().copied().find(|ours| language(ours.tag) == language(tag))
}

/// A message of a domain's catalog.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Key(pub &'static str);

pub enum Arg<'a> {
    Str(&'a str),
    Num(f64),
}

impl<'a> From<&'a str> for Arg<'a> {
    fn from(text: &'a str) -> Self {
        Self::Str(text)
    }
}

macro_rules! numbers {
    ($($number:ty),*) => {$(
        impl From<$number> for Arg<'_> {
            fn from(number: $number) -> Self {
                Self::Num(number as f64)
            }
        }
    )*};
}

numbers!(f64, u32, u64, usize, i64);

/// A domain's files: a language tag and its Fluent source.
pub type Files = &'static [(&'static str, &'static str)];

pub struct Catalog {
    language: Language,
    /// Each domain's bundles, the language's first, then English.
    domains: Vec<(String, Vec<FluentBundle<FluentResource>>)>,
}

impl Catalog {
    /// `pseudo` accents and stretches every message's own text, to find cuts and literals left.
    pub fn new<'a>(language: Language, pseudo: bool, domains: impl IntoIterator<Item = (&'a str, Files)>) -> Self {
        let tags = if language == ENGLISH { vec![ENGLISH.tag] } else { vec![language.tag, ENGLISH.tag] };
        let domains = domains
            .into_iter()
            .map(|(name, files)| {
                let source = |tag: &&str| files.iter().find(|(file, _)| file == tag).map(|(_, source)| bundle(tag, source, pseudo));
                (name.to_owned(), tags.iter().filter_map(source).collect())
            })
            .collect();
        Self { language, domains }
    }

    pub fn language(&self) -> Language {
        self.language
    }

    /// A domain's message: in English when the language lacks it, else its key.
    pub fn text(&self, domain: &str, key: &str, args: &[(&str, Arg)]) -> String {
        let args: Option<FluentArgs> = (!args.is_empty()).then(|| args.iter().map(|(name, arg)| (*name, value(arg))).collect());
        let bundles = self.domains.iter().find(|(name, _)| name == domain).map_or(&[][..], |(_, bundles)| bundles);
        for bundle in bundles {
            let Some(pattern) = bundle.get_message(key).and_then(|message| message.value()) else { continue };
            let mut errors = Vec::new();
            let text = bundle.format_pattern(pattern, args.as_ref(), &mut errors);
            debug_assert!(errors.is_empty(), "{domain} {key}: {errors:?}");
            return text.into_owned();
        }
        key.to_owned()
    }
}

fn value<'a>(arg: &Arg<'a>) -> FluentValue<'a> {
    match *arg {
        Arg::Str(text) => text.into(),
        Arg::Num(number) => number.into(),
    }
}

fn bundle(tag: &str, source: &str, pseudo: bool) -> FluentBundle<FluentResource> {
    let id: LanguageIdentifier = tag.parse().unwrap_or_default();
    let mut bundle = FluentBundle::new(vec![id]);
    // GPUI draws the isolation marks as boxes; no language we ship runs right to left.
    bundle.set_use_isolating(false);
    if pseudo {
        bundle.set_transform(Some(pseudo::stretch));
    }
    // A file that does not parse keeps the messages that do; `checks` parses every shipped file.
    let resource = FluentResource::try_new(source.to_owned()).unwrap_or_else(|(resource, _)| resource);
    bundle.add_resource(resource).ok();
    bundle
}

#[cfg(test)]
mod tests;
