//! A whole run: the trainer's DLL, the game's process, the trainer inside it.

use std::{path::PathBuf, time::Duration};

use anyhow::Result;
use blocking::unblock;
use futures::channel::mpsc::Receiver;
use model::{Process, Rule};

use super::{
    attach::{Game, attach},
    binary, launch,
    tophat::{Event, Server, Session},
};

// Wand's wait after it starts a game itself; when the user has to start it there is more to do.
const LAUNCHED_WITHIN: Duration = Duration::from_secs(45);
const STARTED_BY_USER_WITHIN: Duration = Duration::from_secs(180);

pub struct Request {
    pub data_dir: PathBuf,
    /// Wand's `trainerlib` folder.
    pub trainerlib: PathBuf,
    pub store_id: String,
    pub rule: Rule,
    /// Start the game when it is not running; `false` waits for the user to start it.
    pub launch: bool,
    pub binary_url: String,
    pub binary_hash: String,
    pub variables: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Stage {
    Downloading,
    Launching,
    WaitingForGame,
    Attaching,
}

/// Dropping the future abandons the run; a trainer that is already in the game ends with its session.
pub async fn run(server: &Server, request: Request, mut stage: impl FnMut(Stage)) -> Result<(Session, Receiver<Event>)> {
    stage(Stage::Downloading);
    let (data_dir, url, hash) = (request.data_dir.clone(), request.binary_url.clone(), request.binary_hash.clone());
    let trainer = unblock(move || binary::ensure(&data_dir, &url, &hash)).await?;

    let rule = request.rule.clone();
    let running = unblock(move || agent::find(&rule)).await?;
    let process = match running {
        Some(process) => process,
        None => wait_for_game(&request, &mut stage).await?,
    };

    stage(Stage::Attaching);
    let game = Game { process: &process, trainer: &trainer, trainer_hash: &request.binary_hash, variables: &request.variables };
    attach(server, &request.trainerlib, game).await
}

async fn wait_for_game(request: &Request, stage: &mut impl FnMut(Stage)) -> Result<Process> {
    let mut started = false;
    if request.launch {
        stage(Stage::Launching);
        let store_id = request.store_id.clone();
        started = unblock(move || launch::start(&store_id)).await?;
    }
    stage(Stage::WaitingForGame);
    let within = if started { LAUNCHED_WITHIN } else { STARTED_BY_USER_WITHIN };
    agent::wait(&request.rule, within).await
}
