//! Starting a game through its launcher, or the executable the user picked.

use std::{ffi::OsStr, io, os::windows::ffi::OsStrExt, path::Path, ptr};

use anyhow::{Result, ensure};
use windows_sys::Win32::UI::{Shell::ShellExecuteW, WindowsAndMessaging::SW_SHOWNORMAL};

/// Starts the game of `store_id`: a Steam app, or the executable the user picked, in its own folder.
/// `false`: no way to start this store's games from here, so the user starts the game and the run waits for it.
pub fn start(store_id: &str) -> Result<bool> {
    if let Some(exe) = model::custom_exe(store_id) {
        open(exe.as_os_str(), exe.parent())?;
        return Ok(true);
    }
    let Some(app) = store_id.strip_prefix("steam:") else {
        return Ok(false);
    };
    // The id comes from the catalog, a remote file: only digits reach the URI.
    ensure!(!app.is_empty() && app.bytes().all(|b| b.is_ascii_digit()), "unexpected Steam app id '{app}'");
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
    use super::*;

    #[test]
    fn other_stores_are_not_started_and_odd_ids_are_refused() {
        assert!(!start("epic:abc").unwrap());
        assert!(!start("gog:123").unwrap());
        for id in ["steam:", "steam:12 34", "steam:1&calc", "steam:../x"] {
            assert!(start(id).is_err(), "{id}");
        }
    }
}
