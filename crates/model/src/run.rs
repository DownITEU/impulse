//! A trainer running in a game process, whoever provides it.

use futures::{future::BoxFuture, stream::BoxStream};

use crate::{Failure, Process};

type Result<T> = std::result::Result<T, Failure>;

/// What one cheat list takes to run.
pub trait Runner: Send + Sync {
    /// Before the game is looked for, e.g. fetching the trainer.
    fn prepare(&self) -> BoxFuture<'_, Result<Box<dyn Attach>>>;
}

pub trait Attach: Send {
    /// `process` as found: an attach refuses a process that is no longer this one.
    fn attach(self: Box<Self>, process: Process) -> BoxFuture<'static, Result<Live>>;
}

/// Dropping it ends the run; `stop` is the clean way.
pub trait Session: Send + Sync {
    /// Sets the variable a cheat's `key` names; `Err` when the trainer refuses it.
    fn set<'a>(&'a self, key: &'a str, value: f64) -> BoxFuture<'a, Result<()>>;
    fn stop(&self) -> BoxFuture<'_, Result<()>>;
    /// Where the player is, in the game's own coordinates.
    fn position(&self) -> BoxFuture<'_, Result<[f64; 3]>>;
    fn teleport(&self, to: [f64; 3]) -> BoxFuture<'_, Result<()>>;
}

pub struct Live {
    pub session: Box<dyn Session>,
    /// Ends with the session: the game closed or the trainer left.
    pub events: BoxStream<'static, Event>,
}

pub enum Event {
    Log(String),
    Status(String),
}
