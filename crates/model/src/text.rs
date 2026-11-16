//! Text a source gives the UI: a message of its own catalog, or content already in the asked language.

use std::sync::Arc;

use crate::SourceId;

#[derive(Clone, Debug, PartialEq)]
pub enum Text {
    /// Content as given: a name, a version.
    Plain(String),
    /// A message of `domain`'s catalog, the domain being its source's id.
    Key { domain: SourceId, key: Arc<str>, args: Vec<(Arc<str>, Arg)> },
}

#[derive(Clone, Debug, PartialEq)]
pub enum Arg {
    Str(String),
    Num(f64),
}

/// The language content is asked in: a BCP 47 tag (`fr`, `zh-CN`). A source answers in the closest it has.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Locale(Arc<str>);

impl Locale {
    pub fn new(tag: &str) -> Self {
        Self(tag.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Text {
    pub fn key(domain: &SourceId, key: &str) -> Self {
        Self::Key { domain: domain.clone(), key: key.into(), args: Vec::new() }
    }

    pub fn arg(mut self, name: &str, value: impl Into<Arg>) -> Self {
        if let Self::Key { args, .. } = &mut self {
            args.push((name.into(), value.into()));
        }
        self
    }
}

impl From<String> for Arg {
    fn from(text: String) -> Self {
        Self::Str(text)
    }
}

impl From<&str> for Arg {
    fn from(text: &str) -> Self {
        Self::Str(text.to_owned())
    }
}

impl From<f64> for Arg {
    fn from(number: f64) -> Self {
        Self::Num(number)
    }
}

impl From<u64> for Arg {
    fn from(number: u64) -> Self {
        Self::Num(number as f64)
    }
}
