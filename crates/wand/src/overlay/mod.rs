//! Wand's overlay inside a game: its DLL from the install, loaded once per game process, which hooks the game's
//! frames and dials our host for the textures to draw and sends the game's input over an interactive one back.

mod host;
mod state;
mod wire;

use std::{
    path::Path,
    sync::{Mutex, PoisonError},
};

use agent::Image;
use anyhow::{Context, Result, anyhow};
use host::Load;
use model::{Canvas, Failure, FailureKind, Process, Ref};

use crate::{catalog, folder::Setting, install::PUBLISHERS};

// Two first `enter`s into one game must not both load the DLL.
static LOADING: Mutex<()> = Mutex::new(());

/// Blocking. `Unsupported` for a game Wand does not draw in, `Refused` while Wand's own host runs.
pub fn enter(data_dir: &Path, folder: &Setting, game: &Ref, process: &Process) -> Result<Canvas> {
    let unsupported = || Failure::new(FailureKind::Unsupported);
    if !catalog::draws_overlay(data_dir, game.id()) {
        return Err(anyhow!("Wand's overlay does not draw in {game}").context(unsupported()));
    }
    host::start()?;
    host::sweep();
    let _one = LOADING.lock().unwrap_or_else(PoisonError::into_inner);
    match host::loaded(process) {
        Some(true) => {}
        Some(false) => return Err(anyhow!("the overlay failed to enter process {} before", process.pid).context(unsupported())),
        None => {
            let install = folder.install().context("Wand's folder is not set").context(Failure::new(FailureKind::SetupNeeded))?;
            let image = Image::open(&install.overlay(agent::x64(process)?), PUBLISHERS)?;
            host::record(process, Load::Running);
            if let Err(e) = agent::map(process, &image) {
                host::record(process, Load::Failed);
                return Err(e);
            }
            host::record(process, Load::Kept { _lock: image });
            log::debug!("overlay: the DLL went into process {}", process.pid);
        }
    }
    Ok(host::bind(process.pid))
}

#[cfg(test)]
mod tests {
    use std::{env, path::PathBuf, sync::mpsc, thread, time::Duration};

    use futures::{StreamExt, executor::block_on};
    use model::CanvasEvent;

    use super::*;

    /// Needs RimWorld running (Wand's id 1739) with its pid in `IMPULSE_OVERLAY_PID`, Wand closed, Impulse's catalog
    /// and Wand's install on disk. Loads Wand's overlay DLL into the game; a DLL left connecting to a gone host needs
    /// the game restarted.
    #[test]
    #[ignore]
    fn the_dll_in_a_running_game_reports_ready_and_stays_linked_for_the_next_canvas() {
        let pid = env::var("IMPULSE_OVERLAY_PID").expect("IMPULSE_OVERLAY_PID").parse().unwrap();
        let process = agent::of(pid).expect("the game runs");
        let data_dir = PathBuf::from(env::var_os("LOCALAPPDATA").unwrap()).join("Impulse");
        let game = Ref::new(&crate::source(), "1739");
        let open = || enter(&data_dir, &Setting::load(&data_dir), &game, &process).unwrap();
        let until_ready_and_closed = |canvas: Canvas, within| {
            let Canvas { layers, mut events } = canvas;
            let (tx, rx) = mpsc::channel();
            thread::spawn(move || {
                while let Some(event) = block_on(events.next()) {
                    let _ = tx.send(event);
                }
            });
            assert_eq!(rx.recv_timeout(within), Ok(CanvasEvent::Ready));
            drop(layers);
            let end = loop {
                match rx.recv_timeout(Duration::from_secs(5)) {
                    Ok(CanvasEvent::Resized { .. }) => {}
                    other => break other,
                }
            };
            assert_eq!(end, Err(mpsc::RecvTimeoutError::Disconnected), "its events end");
            thread::sleep(Duration::from_secs(1));
            assert!(host::linked_to(pid), "the link lasts while the game runs");
        };
        until_ready_and_closed(open(), Duration::from_secs(40));
        until_ready_and_closed(open(), Duration::from_secs(5));
    }
}
