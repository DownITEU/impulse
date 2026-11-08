//! A whole run: the trainer prepared, the game's process found or started, the trainer inside it.

use std::{sync::Arc, time::Duration};

use anyhow::Result;
use blocking::unblock;
use model::{Live, Process, Rule, Runner};

use super::launch;

// Wand's wait after it starts a game itself; when the user has to start it there is more to do.
const LAUNCHED_WITHIN: Duration = Duration::from_secs(45);
const STARTED_BY_USER_WITHIN: Duration = Duration::from_secs(180);

pub struct Request {
    pub runner: Arc<dyn Runner>,
    pub store_id: String,
    pub rule: Rule,
    /// Start the game when it is not running; `false` waits for the user to start it.
    pub launch: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Stage {
    Preparing,
    Launching,
    WaitingForGame,
    Attaching,
}

/// The game's process and the live trainer. Dropping the future abandons the run; a trainer that is
/// already in the game ends with its session.
pub async fn run(request: Request, mut stage: impl FnMut(Stage)) -> Result<(Process, Live)> {
    stage(Stage::Preparing);
    let prepared = request.runner.prepare().await?;

    let rule = request.rule.clone();
    let running = unblock(move || agent::find(&rule)).await?;
    let process = match running {
        Some(process) => process,
        None => wait_for_game(&request, &mut stage).await?,
    };

    stage(Stage::Attaching);
    Ok((process.clone(), prepared.attach(process).await?))
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
