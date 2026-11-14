use std::{fmt, sync::Arc};

use model::Ref;
use serde::{Deserialize, Serialize};

/// A library game: its first source's game `Ref`, as text.
#[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Debug, Serialize, Deserialize)]
#[serde(transparent)]
pub struct GameId(Arc<str>);

impl GameId {
    pub fn of(game: &Ref) -> Self {
        Self(game.to_string().into())
    }

    /// The source's game it names.
    pub fn game(&self) -> Option<Ref> {
        Ref::parse(&self.0)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for GameId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
