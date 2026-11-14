//! Where an install comes from: a launcher's id for the game, or the executable the user picked.

use std::{fmt, path::PathBuf, sync::Arc};

/// `<store>:<id>`, such as `steam:1203620`, `epic:<namespace>:<item>:<app>`, `uwp:<family>`; the id is opaque.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct StoreId(Arc<str>);

impl StoreId {
    /// `None` without a store before the first `:`.
    pub fn parse(text: &str) -> Option<Self> {
        text.split_once(':').filter(|(store, _)| !store.is_empty()).map(|_| Self(text.into()))
    }

    pub fn store(&self) -> &str {
        self.0.split_once(':').map_or("", |(store, _)| store)
    }

    pub fn id(&self) -> &str {
        self.0.split_once(':').map_or("", |(_, id)| id)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn steam_app(&self) -> Option<u32> {
        (self.store() == "steam").then(|| self.id().parse().ok()).flatten()
    }
}

impl fmt::Display for StoreId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Clone, PartialEq, Debug)]
pub enum Origin {
    Launcher(StoreId),
    /// The executable the user picked.
    Picked(PathBuf),
}

impl Origin {
    pub fn store_id(&self) -> Option<&StoreId> {
        match self {
            Origin::Launcher(id) => Some(id),
            Origin::Picked(_) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_store_is_before_the_first_colon_and_the_rest_is_the_id() {
        let id = StoreId::parse("epic:ns:item:app").unwrap();
        assert_eq!((id.store(), id.id()), ("epic", "ns:item:app"));
        assert_eq!(StoreId::parse("steam:1203620").unwrap().steam_app(), Some(1203620));
        for text in ["", "steam", ":1"] {
            assert_eq!(StoreId::parse(text), None, "{text}");
        }
        for text in ["steam:", "steam:1 2", "gog:1"] {
            assert_eq!(StoreId::parse(text).unwrap().steam_app(), None, "{text}");
        }
    }
}
