//! Puts `tophat_service` into a running process and starts it.

use agent::Image;
use anyhow::Result;
use model::Process;
use serde::Serialize;

const START_EXPORT: &str = "StartTophatServiceThread";

/// What the service reads to find us: it dials `wand_port` and registers as `client_id`.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Startup<'a> {
    pub wand_port: u16,
    pub client_id: &'a str,
    pub trainer_session_id: &'a str,
}

/// Returns once the service's thread runs; it registers with the server on its own.
pub fn tophat(target: &Process, service: &Image, startup: &Startup) -> Result<()> {
    let mut json = serde_json::to_vec(startup)?;
    // The service reads a C string.
    json.push(0);
    agent::load(target, service, START_EXPORT, &json)
}

#[cfg(test)]
mod tests {
    use std::{
        future::Future,
        process::{Child, Command, Stdio},
        time::Duration,
    };

    use anyhow::{Context, ensure};
    use futures::{StreamExt, executor::block_on};
    use tokio::runtime::Handle as Runtime;

    use super::{
        super::tophat::{Server, Session},
        *,
    };
    use crate::{
        install::{PUBLISHERS, binaries, installs, trainer_host},
        trainer_host::TrainerHost,
    };

    #[test]
    fn startup_is_the_json_the_service_reads() {
        let startup = Startup { wand_port: 5000, client_id: "c", trainer_session_id: "s" };
        let json = serde_json::to_string(&startup).unwrap();
        assert_eq!(json, r#"{"wandPort":5000,"clientId":"c","trainerSessionId":"s"}"#);
    }

    /// Waits for `work` from this thread, with the timer on the runtime like the app's callers.
    fn bounded<T: Send + 'static>(runtime: &Runtime, secs: u64, work: impl Future<Output = T> + Send + 'static) -> Result<T> {
        let timed = runtime.spawn(async move { tokio::time::timeout(Duration::from_secs(secs), work).await });
        block_on(timed)?.context("timed out")
    }

    /// Injects the installed service into `target`, waits for it to dial in, then ends the session.
    fn dials_in_and_ends(target: &Process) -> Result<()> {
        let [service, _] = binaries(&installs().first().expect("a Wand install").trainerlib(), true);
        let service = Image::open(&service, PUBLISHERS)?;
        let server = Server::start()?;
        let client_id = uuid::Uuid::new_v4().to_string();
        let waiting = server.expect(&client_id);
        let startup = Startup { wand_port: server.port, client_id: &client_id, trainer_session_id: &uuid::Uuid::new_v4().to_string() };
        tophat(target, &service, &startup)?;
        let channel = bounded(&server.runtime, 20, waiting)?.context("the server dropped the waiter")?;
        let (session, mut events) = Session::open(server.runtime.clone(), channel, &client_id);
        // No trainer is running, so the service may refuse the stop; the session must still end.
        let stopped = block_on(session.stop());
        println!("stop: {stopped:?}");
        ensure!(bounded(&server.runtime, 10, async move { events.next().await })?.is_none());
        Ok(())
    }

    fn ping() -> Child {
        Command::new(r"C:\Windows\System32\PING.EXE").args(["-n", "120", "127.0.0.1"]).stdout(Stdio::null()).spawn().unwrap()
    }

    /// Needs a Wand install. The target is a throwaway `ping` we start, never a game.
    #[test]
    #[ignore = "injects the installed tophat_service into a throwaway process"]
    fn the_injected_service_dials_in_and_the_session_ends_cleanly() {
        let mut target = ping();
        let outcome = agent::of(target.id()).context("the throwaway process").and_then(|target| dials_in_and_ends(&target));
        target.kill().unwrap();
        target.wait().unwrap();
        outcome.unwrap();
    }

    /// As `activate.flags` 16: the service goes into the external host for a throwaway `ping`.
    #[test]
    #[ignore = "starts Wand's TrainerHost_x64.exe and injects the installed tophat_service into it"]
    fn the_service_dials_in_from_the_external_host() {
        let mut game = ping();
        let outcome = (|| -> Result<()> {
            let exe = trainer_host(&installs().first().expect("a Wand install").trainerlib(), true);
            let host = TrainerHost::start(&exe, &agent::of(game.id()).context("the throwaway process")?)?;
            dials_in_and_ends(&host.process)
        })();
        game.kill().unwrap();
        game.wait().unwrap();
        outcome.unwrap();
    }
}
