//! Puts `tophat_service` into a running process and starts it, as Wand's auxiliary service does.

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
        process::{Command, Stdio},
        time::Duration,
    };

    use anyhow::{Context, ensure};
    use futures::{StreamExt, executor::block_on};
    use tokio::runtime::Handle as Runtime;

    use super::{
        super::tophat::{Server, Session},
        *,
    };
    use crate::install::{PUBLISHER, SERVICE, installs};

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

    /// Needs a Wand install. The target is a throwaway `ping` we start, never a game.
    #[test]
    #[ignore = "injects the installed tophat_service into a throwaway process"]
    fn the_injected_service_dials_in_and_the_session_ends_cleanly() {
        let service = installs().first().expect("a Wand install").trainerlib().join(SERVICE);
        let service = Image::open(&service, PUBLISHER).unwrap();
        let server = Server::start().unwrap();
        let client_id = uuid::Uuid::new_v4().to_string();
        let waiting = server.expect(&client_id);
        let mut target = Command::new(r"C:\Windows\System32\PING.EXE")
            .args(["-n", "120", "127.0.0.1"])
            .stdout(Stdio::null())
            .spawn()
            .unwrap();

        let outcome = (|| -> Result<()> {
            let startup = Startup { wand_port: server.port, client_id: &client_id, trainer_session_id: &uuid::Uuid::new_v4().to_string() };
            tophat(&agent::of(target.id()).context("the throwaway process")?, &service, &startup)?;
            let channel = bounded(&server.runtime, 20, waiting)?.context("the server dropped the waiter")?;
            let (session, mut events) = Session::open(server.runtime.clone(), channel, &client_id);
            // No trainer is running, so the service may refuse the stop; the session must still end.
            let stopped = block_on(session.stop());
            println!("stop: {stopped:?}");
            ensure!(bounded(&server.runtime, 10, async move { events.next().await })?.is_none());
            Ok(())
        })();

        target.kill().unwrap();
        target.wait().unwrap();
        outcome.unwrap();
    }
}
