//! The external trainer host (`activate.flags` 16): a process of its own that takes the service in instead of the game.

use std::{
    os::windows::process::CommandExt,
    path::Path,
    process::{Child, Command, Stdio},
    thread,
    time::Duration,
};

use agent::Program;
use anyhow::{Context, Result, bail};
use blocking::unblock;
use model::Process;

use crate::install::PUBLISHERS;

// The host's arguments: the game's pid, then how often it looks for it while denied a wait on it.
const POLL_MS: &str = "1000";
// The host ends this long after the session, so the service in it tears down first.
const LINGER: Duration = Duration::from_secs(2);
const LINGER_GAME_GONE: Duration = Duration::from_secs(10);
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// A running host; ended off the dropping thread a moment after it is dropped. It also exits once the game does.
pub struct TrainerHost {
    child: Option<Child>,
    pub process: Process,
    game: Process,
    /// Its file stays locked while it runs.
    _exe: Program,
}

impl TrainerHost {
    /// Starts `exe`, checked as WeMod's, for `game` (blocking).
    pub fn start(exe: &Path, game: &Process) -> Result<Self> {
        let exe = Program::open(exe, PUBLISHERS)?;
        // Our handles are not its: an inherited pipe would stay open while it runs.
        let started = Command::new(exe.path())
            .args([&game.pid.to_string(), POLL_MS])
            .creation_flags(CREATE_NO_WINDOW)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn();
        let mut child = started.with_context(|| format!("cannot start {}", exe.path().display()))?;
        let Some(process) = agent::of(child.id()) else {
            let _ = child.kill();
            bail!("{} exited as it started", exe.path().display());
        };
        Ok(Self { child: Some(child), process, game: game.clone(), _exe: exe })
    }
}

impl Drop for TrainerHost {
    fn drop(&mut self) {
        let Some(mut child) = self.child.take() else {
            return;
        };
        let game = self.game.clone();
        unblock(move || {
            let gone = agent::of(game.pid).is_none_or(|now| now != game);
            thread::sleep(if gone { LINGER_GAME_GONE } else { LINGER });
            let _ = child.kill();
            let _ = child.wait();
        })
        .detach();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::install::{installs, trainer_host};

    /// Needs a Wand install. The "game" is a throwaway `ping`: the host waits for it and exits with it.
    #[test]
    #[ignore = "starts Wand's TrainerHost_x64.exe"]
    fn the_host_runs_while_its_game_does() {
        let exe = trainer_host(&installs().first().expect("a Wand install").trainerlib(), true);
        let mut ping = Command::new(r"C:\Windows\System32\PING.EXE").args(["-n", "60", "127.0.0.1"]).stdout(Stdio::null()).spawn().unwrap();
        let game = agent::of(ping.id()).unwrap();
        let host = TrainerHost::start(&exe, &game).unwrap();
        let pid = host.process.pid;
        assert!(host.process.path.ends_with("TrainerHost_x64.exe"), "{:?}", host.process.path);
        thread::sleep(Duration::from_secs(1));
        assert!(agent::of(pid).is_some(), "it waits for the game");
        ping.kill().unwrap();
        ping.wait().unwrap();
        thread::sleep(Duration::from_secs(3));
        assert!(agent::of(pid).is_none_or(|now| now != host.process), "it exits with the game");
    }
}
