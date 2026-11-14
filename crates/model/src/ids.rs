//! Ids a source mints: its own name, and a thing's id within it.

use std::{fmt, sync::Arc};

/// A source's name: `[a-z0-9-]{1,32}`.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct SourceId(Arc<str>);

impl SourceId {
    /// The app's own ids (`app:pinned`, its media stores): never a source's.
    pub fn app() -> Self {
        Self("app".into())
    }

    pub fn new(name: &str) -> Option<Self> {
        let plain = |b: u8| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-';
        ((1..=32).contains(&name.len()) && name.bytes().all(plain)).then(|| Self(name.into()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Something a source minted, printed `source:id`; the id is the source's own and opaque.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct Ref {
    source: SourceId,
    id: Arc<str>,
}

impl Ref {
    pub fn new(source: &SourceId, id: &str) -> Self {
        Self { source: source.clone(), id: id.into() }
    }

    /// Split at the first `:`; `None` for a bad source or an empty id.
    pub fn parse(text: &str) -> Option<Self> {
        let (source, id) = text.split_once(':')?;
        (!id.is_empty()).then(|| Some(Self::new(&SourceId::new(source)?, id))).flatten()
    }

    pub fn source(&self) -> &SourceId {
        &self.source
    }

    pub fn id(&self) -> &str {
        &self.id
    }
}

impl fmt::Display for Ref {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.source.as_str(), self.id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_ref_round_trips_and_its_id_may_hold_colons() {
        let parsed = Ref::parse("wand:a:B c").unwrap();
        assert_eq!((parsed.source().as_str(), parsed.id()), ("wand", "a:B c"));
        assert_eq!(parsed.to_string(), "wand:a:B c");
    }

    #[test]
    fn odd_source_names_and_empty_ids_are_refused() {
        for name in ["", "Wand", "a:b", "..", "a/b", "a b", &"a".repeat(33)] {
            assert_eq!(SourceId::new(name), None, "{name}");
        }
        for text in ["wand", "wand:", ":x", "a/b:x"] {
            assert_eq!(Ref::parse(text), None, "{text}");
        }
    }
}
