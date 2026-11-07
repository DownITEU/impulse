//! The one call from "this game process" to "a trainer running in it".

use std::{path::Path, time::Duration};

use anyhow::{Context, Result};
use blocking::unblock;
use futures::channel::mpsc::Receiver;
use uuid::Uuid;

use super::{
    image::Image,
    inject::{self, Startup},
    tophat::{Event, Run, Server, Session},
};
use crate::{
    pe,
    wand::install::{PLUGIN, SERVICE},
};

const DIAL_IN_WITHIN: Duration = Duration::from_secs(15);

pub struct Game<'a> {
    pub pid: u32,
    pub exe: &'a Path,
    pub trainer: &'a Path,
    pub variables: &'a [String],
}

/// `trainerlib` is the directory of Wand's service and plugin DLLs. Every DLL is checked before
/// it enters the game. Dropping the returned session ends it; `stop` is the clean way.
pub async fn attach(server: &Server, trainerlib: &Path, game: Game<'_>) -> Result<(Session, Receiver<Event>)> {
    let version = pe::header(game.exe).with_context(|| format!("{} is not a PE image", game.exe.display()))?.timestamp;
    let (service, plugin, trainer) = {
        let (service, plugin, trainer) = (trainerlib.join(SERVICE), trainerlib.join(PLUGIN), game.trainer.to_owned());
        unblock(move || Ok::<_, anyhow::Error>((Image::open(&service)?, Image::open(&plugin)?, Image::open(&trainer)?))).await?
    };

    let client_id = Uuid::new_v4().to_string();
    // Before the injection: the service dials in as soon as it starts.
    let waiting = server.expect(&client_id);
    let (pid, port, id) = (game.pid, server.port, client_id.clone());
    unblock(move || {
        let startup = Startup { wand_port: port, client_id: &id, trainer_session_id: &Uuid::new_v4().to_string() };
        inject::tophat(pid, &service, &startup)
    })
    .await?;

    let dialed = server.runtime.spawn(async move { tokio::time::timeout(DIAL_IN_WITHIN, waiting).await }).await?;
    let channel = dialed
        .context("the service did not dial in")?
        .context("the server dropped the service")?;
    let (session, events) = Session::open(server.runtime.clone(), channel, &client_id);
    let run = Run { process_id: game.pid, game_version: version, trainer, variables: game.variables };
    session.start(plugin, run).await?;
    Ok((session, events))
}

#[cfg(test)]
mod tests {
    use std::{env, fs, path::PathBuf, thread::sleep};

    use futures::executor::block_on;
    use serde_json::Value;
    use windows_sys::Win32::{
        Foundation::CloseHandle,
        System::Threading::{OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW},
    };

    use super::*;

    fn var(name: &str) -> Option<String> {
        env::var(format!("IMPULSE_E2E_{name}")).ok()
    }

    fn image_of(pid: u32) -> PathBuf {
        // SAFETY: plain Win32 calls; the buffer is valid for the length passed and the handle is closed once.
        unsafe {
            let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
            assert!(!process.is_null(), "cannot open process {pid}");
            let mut name = vec![0u16; 32_768];
            let mut len = name.len() as u32;
            let named = QueryFullProcessImageNameW(process, 0, name.as_mut_ptr(), &mut len);
            CloseHandle(process);
            assert!(named != 0, "cannot read the image of process {pid}");
            PathBuf::from(String::from_utf16_lossy(&name[..len as usize]))
        }
    }

    /// Hand-run against a game at its main menu; the variables are in the ignore reason below.
    /// Sets the values and stops the trainer cleanly. The game keeps running.
    #[test]
    #[ignore = "injects into a running game; needs IMPULSE_E2E_{PID,GAME_EXE,TRAINER,BLUEPRINT}, optional _SET=name=1;name=2 and _HOLD=secs"]
    fn a_running_game_takes_values() {
        let pid: u32 = var("PID").expect("IMPULSE_E2E_PID").parse().unwrap();
        let exe = PathBuf::from(var("GAME_EXE").expect("IMPULSE_E2E_GAME_EXE"));
        let trainer = PathBuf::from(var("TRAINER").expect("IMPULSE_E2E_TRAINER"));
        // A wrong pid would put the service into some other program.
        assert!(image_of(pid).to_string_lossy().eq_ignore_ascii_case(&exe.to_string_lossy()), "process {pid} is not {}", exe.display());

        let blueprint = fs::read_to_string(var("BLUEPRINT").expect("IMPULSE_E2E_BLUEPRINT")).unwrap();
        let blueprint: Value = serde_json::from_str(blueprint.trim_start_matches('\u{feff}')).unwrap();
        let mut variables: Vec<String> = Vec::new();
        for target in blueprint["cheats"].as_array().unwrap().iter().filter_map(|cheat| cheat["target"].as_str()) {
            if !variables.iter().any(|known| known == target) {
                variables.push(target.to_owned());
            }
        }

        let server = Server::start().unwrap();
        let trainerlib = crate::wand::install::installs().first().expect("a Wand install").trainerlib();
        let game = Game { pid, exe: &exe, trainer: &trainer, variables: &variables };
        let (session, mut events) = block_on(attach(&server, &trainerlib, game)).unwrap();
        println!("attached, phase {:?}", session.phase());

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
    }
}
