//! Starting a game through its launcher, or the executable the user picked.

use std::{ffi::OsStr, io, os::windows::ffi::OsStrExt, path::Path, ptr};

use anyhow::{Context, Result, ensure};
use model::Origin;
use windows_sys::Win32::UI::{Shell::ShellExecuteW, WindowsAndMessaging::SW_SHOWNORMAL};

/// Starts a Steam app, or the executable the user picked in its own folder (blocking).
/// `false`: no way to start this store's games from here, so the user starts the game and the run waits for it.
pub fn start(origin: &Origin) -> Result<bool> {
    let id = match origin {
        Origin::Picked(exe) => {
            open(exe.as_os_str(), exe.parent())?;
            return Ok(true);
        }
        Origin::Launcher(id) if id.store() == "steam" => id,
        Origin::Launcher(_) => return Ok(false),
    };
    // The id comes from the catalog, a remote file: only a number reaches the URI.
    let app = id.steam_app().with_context(|| format!("unexpected Steam app id '{}'", id.id()))?;
    open(OsStr::new(&format!("steam://rungameid/{app}")), None)?;
    Ok(true)
}

fn open(target: &OsStr, dir: Option<&Path>) -> Result<()> {
    let wide = |text: &OsStr| text.encode_wide().chain([0]).collect::<Vec<u16>>();
    let (target_w, verb, dir) = (wide(target), wide(OsStr::new("open")), dir.map(|dir| wide(dir.as_os_str())));
    let dir_w = dir.as_ref().map_or(ptr::null(), |dir| dir.as_ptr());
    // SAFETY: the strings are NUL-terminated and outlive the call.
    let result = unsafe { ShellExecuteW(ptr::null_mut(), verb.as_ptr(), target_w.as_ptr(), ptr::null(), dir_w, SW_SHOWNORMAL) };
    // Values up to 32 are error codes.
    ensure!(result as usize > 32, "cannot open {}: {}", target.display(), io::Error::last_os_error());
    Ok(())
}

#[cfg(test)]
mod tests {
    use model::StoreId;

    use super::*;

    #[test]
    fn other_stores_are_not_started_and_odd_ids_are_refused() {
        let start = |id: &str| start(&Origin::Launcher(StoreId::parse(id).unwrap()));
        assert!(!start("epic:abc").unwrap());
        assert!(!start("gog:123").unwrap());
        for id in ["steam:", "steam:12 34", "steam:1&calc", "steam:../x"] {
            assert!(start(id).is_err(), "{id}");
        }
    }
}
