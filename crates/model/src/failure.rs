//! Why a source or a run failed, for the UI to word: the kind picks the text, the source may add its own.

use std::fmt;

use crate::Text;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FailureKind {
    Offline,
    SignedOut,
    NotFound,
    Unsupported,
    /// Something the user sets up is missing, such as a folder.
    SetupNeeded,
    GameNotFound,
    LaunchFailed,
    /// Started by the run, the game did not show in time.
    NotStarted,
    /// The run waited for the user to start the game, and it did not show in time.
    NotRunning,
    GameExited,
    Refused,
    /// The game runs as administrator and the user did not let the app be one too.
    Elevation,
    /// The game's process cannot be opened, even as administrator: its protection keeps trainers out.
    Protected,
    /// The trainer is built for the game's other architecture, such as another of its executables: 64-bit when `needs_x64`.
    Architecture {
        needs_x64: bool,
    },
    LoadFailed,
    TrainerFailed,
    /// The game runs, but only one the run starts itself may be used: close it and start again.
    Restart,
    Internal,
}

/// Made at a crate's boundary from an `anyhow` error, whose origin marks it with `.context(Failure::new(kind))`;
/// an unmarked one is `Internal`.
#[derive(Clone, Debug)]
pub struct Failure {
    pub kind: FailureKind,
    /// The source's own words, when the kind's text says too little.
    pub message: Option<Text>,
    /// English, for the log only.
    pub detail: String,
}

impl Failure {
    pub fn new(kind: FailureKind) -> Self {
        Self { kind, message: None, detail: String::new() }
    }

    pub fn saying(mut self, message: Text) -> Self {
        self.message = Some(message);
        self
    }
}

impl fmt::Display for Failure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.detail.is_empty() { write!(f, "{:?}", self.kind) } else { f.write_str(&self.detail) }
    }
}

impl std::error::Error for Failure {}

impl From<anyhow::Error> for Failure {
    fn from(error: anyhow::Error) -> Self {
        let mark = error.downcast_ref::<Failure>().cloned().unwrap_or(Failure::new(FailureKind::Internal));
        Self { detail: format!("{error:#}"), ..mark }
    }
}

#[cfg(test)]
mod tests {
    use anyhow::{Context, anyhow};

    use super::*;

    #[test]
    fn the_mark_is_found_under_later_context_and_the_detail_keeps_the_chain() {
        let marked = Err::<(), _>(anyhow!("HTTP 401")).context(Failure::new(FailureKind::SignedOut)).context("catalog");
        let failure = Failure::from(marked.unwrap_err());
        assert_eq!((failure.kind, failure.detail.as_str()), (FailureKind::SignedOut, "catalog: SignedOut: HTTP 401"));
        assert_eq!(Failure::from(anyhow!("bad")).kind, FailureKind::Internal);
        let again: anyhow::Error = Failure::from(anyhow!("x").context(Failure::new(FailureKind::Refused))).into();
        assert_eq!(Failure::from(again.context("run")).kind, FailureKind::Refused);
    }
}
