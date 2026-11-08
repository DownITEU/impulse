//! Running a trainer in a game process.

mod launch;
mod run;

pub use run::{Request, Stage, run};
