//! The one call from "this game process" to "a trainer running in it".

use std::{path::Path, time::Duration};

use agent::Image;
use anyhow::{Context, Result, anyhow, ensure};
use blocking::unblock;
use futures::{
    channel::mpsc::Receiver,
    future::{self, Either},
};
use model::{Failure, FailureKind, Process};
use uuid::Uuid;

use crate::{
    binary,
    inject::{self, Startup},
    install::{PUBLISHERS, binaries, trainer_host},
    texts::{SERVICE_SILENT, text},
    tophat::{Event, Run, Server, Session, within},
    trainer::Activate,
    trainer_host::TrainerHost,
};

const DIAL_IN_WITHIN: Duration = Duration::from_secs(45);
// The wait for the last session's service to leave the process.
const TEARDOWN_WITHIN: Duration = Duration::from_secs(30);
// The wait for the game to take input before any trainer goes in.
const IDLE_WITHIN: Duration = Duration::from_secs(10);

pub struct Game<'a> {
    /// As found: injection refuses a process that is no longer this one.
    pub process: &'a Process,
    /// The game's version for the trainer: the install's build, 0 when unknown.
    pub version: u32,
    pub trainer: &'a Path,
    /// SHA-256 the trainer must have: checked on the locked file, not on the download.
    pub trainer_hash: &'a str,
    pub variables: &'a [String],
    pub activate: Activate,
}

/// A trainer running in the game, and the external host when it asked for one. On drop the session ends first,
/// then the host.
pub struct Attached {
    pub session: Session,
    pub _host: Option<TrainerHost>,
}

/// `trainerlib` is the directory of Wand's service and plugin DLLs. Every DLL is checked before
/// it enters the game. Dropping the returned session ends it; `stop` is the clean way.
pub async fn attach(server: &Server, trainerlib: &Path, game: Game<'_>) -> Result<(Attached, Receiver<Event>)> {
    let (service, plugin, trainer, host) = {
        let (trainerlib, trainer, process) = (trainerlib.to_owned(), game.trainer.to_owned(), game.process.clone());
        let (hash, external) = (game.trainer_hash.to_owned(), game.activate.external_host());
        unblock(move || {
            let trainer = Image::open(&trainer, PUBLISHERS)?;
            // The download was checked earlier; the file may have been swapped before it was locked.
            let found = binary::hash_of(trainer.path()).context("cannot read the trainer")?;
            ensure!(found.eq_ignore_ascii_case(&hash), "the trainer on disk does not match its hash");
            // Of the trainer's architecture, as Wand picks them; the injection checks it against the game's.
            let [service, plugin] = binaries(&trainerlib, trainer.x64());
            let (service, plugin) = (Image::open(&service, PUBLISHERS)?, Image::open(&plugin, PUBLISHERS)?);
            let host = external.then(|| TrainerHost::start(&trainer_host(&trainerlib, trainer.x64()), &process)).transpose()?;
            Ok::<_, anyhow::Error>((service, plugin, trainer, host))
        })
        .await?
    };
    // The service goes into the host when there is one; the trainer always works on the game.
    let target = host.as_ref().map_or(game.process, |host| &host.process).clone();

    // A trainer stopped a moment ago may still be tearing its service down in this process. Apart from the
    // injection: a run dropped while it waits injects nothing.
    let torn_down = format!(r"Local\TophatService_TeardownComplete_{}", target.pid);
    if let Err(e) = unblock(move || agent::signaled(&torn_down, TEARDOWN_WITHIN)).await {
        log::warn!("{e:#}");
    }
    let held = hold(game.process, game.activate).await?;

    let client_id = Uuid::new_v4().to_string();
    // Before the injection: the service dials in as soon as it starts.
    let waiting = server.expect(&client_id);
    let (port, id) = (server.port, client_id.clone());
    unblock(move || {
        let startup = Startup { wand_port: port, client_id: &id, trainer_session_id: &Uuid::new_v4().to_string() };
        inject::tophat(&target, &service, &startup)
    })
    .await?;

    // The game or the host gone ends the wait for the service; the game first, as the host goes with it.
    let watched: Vec<Process> = [Some(game.process), host.as_ref().map(|host| &host.process)].into_iter().flatten().cloned().collect();
    let exit = unblock(move || agent::exit(&watched)).await?;
    let channel = match within(&server.runtime, DIAL_IN_WITHIN, future::select(waiting, exit)).await {
        Some(Either::Left((channel, _))) => channel.context("the server dropped the service")?,
        Some(Either::Right((process, _))) => {
            let gone = anyhow!("{} exited before the trainer's service dialed in", process.path.display());
            return Err(if process == *game.process { gone.context(Failure::new(FailureKind::GameNotFound)) } else { gone });
        }
        // Seen with an outdated Wand.
        None => {
            let silent = Failure::new(FailureKind::SetupNeeded).saying(text(SERVICE_SILENT));
            return Err(anyhow!("the service from {} did not dial in", trainerlib.display()).context(silent));
        }
    };
    let (session, events) = Session::open(server.runtime.clone(), channel, &client_id);
    let run = Run { process_id: game.process.pid, game_version: game.version, trainer, variables: game.variables };
    session.start(plugin, run).await?;
    if let Some(held) = held {
        held.ready(game.activate.resume()).await.context("cannot let the game go after the trainer's setup")?;
    }
    Ok((Attached { session, _host: host }, events))
}

/// Wand's step before every trainer: the game is let take input first, then held still if the trainer asks.
async fn hold(game: &Process, activate: Activate) -> Result<Option<Held>> {
    let game = game.clone();
    unblock(move || {
        if !agent::idle(&game, IDLE_WITHIN)? {
            log::debug!("the game took no input in {IDLE_WITHIN:?}; the trainer goes in anyway");
        }
        if !activate.suspend() {
            return Ok(None);
        }
        // Wand's service holds an elevated game too; ours would need a prompt for it, so the trainer loads as it did.
        Ok(agent::suspend(&game)
            .inspect_err(|e| log::warn!("the game runs on while the trainer sets up: {e:#}"))
            .ok()
            .map(|()| Held(Some(game))))
    })
    .await
}

/// A game held still, let go when this drops unless `ready` says the trainer keeps it so.
struct Held(Option<Process>);

impl Held {
    /// Disarmed only in the worker: a run dropped before it runs still resumes the game, once.
    async fn ready(mut self, resume: bool) -> Result<()> {
        unblock(move || match self.0.take() {
            Some(game) if resume => agent::resume(&game),
            _ => Ok(()),
        })
        .await
    }
}

impl Drop for Held {
    fn drop(&mut self) {
        if let Some(game) = self.0.take()
            && let Err(e) = agent::resume(&game)
        {
            log::warn!("{e:#}");
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{env, fs, path::PathBuf, thread::sleep};

    use futures::executor::block_on;
    use serde_json::Value;

    use super::*;

    fn var(name: &str) -> Option<String> {
        env::var(format!("IMPULSE_E2E_{name}")).ok()
    }

    /// Hand-run against a game at its main menu; the variables are in the ignore reason below.
    /// Sets the values and stops the trainer cleanly. The game keeps running.
    #[test]
    #[ignore = "injects into a running game; needs IMPULSE_E2E_{PID,GAME_EXE,TRAINER,BLUEPRINT}, optional _VERSION, _FLAGS (activate.flags), _SET=name=1;name=2 and _HOLD=secs"]
    fn a_running_game_takes_values() {
        let pid: u32 = var("PID").expect("IMPULSE_E2E_PID").parse().unwrap();
        let exe = PathBuf::from(var("GAME_EXE").expect("IMPULSE_E2E_GAME_EXE"));
        let trainer = PathBuf::from(var("TRAINER").expect("IMPULSE_E2E_TRAINER"));
        // A wrong pid would put the service into some other program.
        let process = agent::of(pid).expect("a process with that id");
        assert!(process.path.to_string_lossy().eq_ignore_ascii_case(&exe.to_string_lossy()), "process {pid} is not {}", exe.display());

        let blueprint = fs::read_to_string(var("BLUEPRINT").expect("IMPULSE_E2E_BLUEPRINT")).unwrap();
        let blueprint: Value = serde_json::from_str(blueprint.trim_start_matches('\u{feff}')).unwrap();
        let mut variables: Vec<String> = Vec::new();
        for target in blueprint["cheats"].as_array().unwrap().iter().filter_map(|cheat| cheat["target"].as_str()) {
            if !variables.iter().any(|known| known == target) {
                variables.push(target.to_owned());
            }
        }

        let server = Server::start().unwrap();
        let trainerlib = crate::install::installs().first().expect("a Wand install").trainerlib();
        let hash = binary::hash_of(&trainer).unwrap();
        let version = var("VERSION").map_or(0, |version| version.parse().unwrap());
        let activate = Activate(var("FLAGS").map_or(0, |flags| flags.parse().unwrap()));
        let game = Game { process: &process, version, trainer: &trainer, trainer_hash: &hash, variables: &variables, activate };
        let (Attached { session, _host: host }, mut events) = block_on(attach(&server, &trainerlib, game)).unwrap();
        println!("attached, phase {:?}, host {:?}", session.phase(), host.as_ref().map(|host| host.process.pid));

        for pair in var("SET").iter().flat_map(|sets| sets.split(';')) {
            let (name, value) = pair.split_once('=').expect("name=value");
            println!("{name}={value}: {:?}", block_on(session.set_value(name, value.parse().unwrap())));
        }
        sleep(Duration::from_secs(var("HOLD").map_or(0, |secs| secs.parse().unwrap())));
        println!("stop: {:?}", block_on(session.stop()));
        while let Ok(event) = events.try_recv() {
            match event {
                Event::Buffer { kind, data } => println!("buffer {kind}, {} bytes", data.len()),
                event => println!("{event:?}"),
            }
        }
        // The host ends a moment after the session, off this thread: the test outlives that.
        let hosted = host.as_ref().map(|host| host.process.clone());
        drop((session, host));
        if let Some(hosted) = hosted {
            sleep(Duration::from_secs(3));
            assert!(agent::of(hosted.pid).is_none_or(|now| now != hosted), "the host outlived its session");
        }
    }
}
