//! Starting a game through its launcher.

use std::{io, ptr};

use anyhow::{Result, ensure};
use windows_sys::Win32::UI::{Shell::ShellExecuteW, WindowsAndMessaging::SW_SHOWNORMAL};

/// Starts the game of `store_id` (`steam:<appid>`). `false`: no way to start this store's games
/// from here, so the user starts the game and the run waits for it.
pub fn start(store_id: &str) -> Result<bool> {
    let Some(app) = store_id.strip_prefix("steam:") else {
        return Ok(false);
    };
    // The id comes from the catalog, a remote file: only digits reach the URI.
    ensure!(!app.is_empty() && app.bytes().all(|b| b.is_ascii_digit()), "unexpected Steam app id '{app}'");
    open(&format!("steam://rungameid/{app}"))?;
    Ok(true)
}

fn open(uri: &str) -> Result<()> {
    let wide: Vec<u16> = uri.encode_utf16().chain([0]).collect();
    let verb: Vec<u16> = "open\0".encode_utf16().collect();
    // SAFETY: both strings are NUL-terminated and outlive the call.
    let result = unsafe { ShellExecuteW(ptr::null_mut(), verb.as_ptr(), wide.as_ptr(), ptr::null(), ptr::null(), SW_SHOWNORMAL) };
    // Values up to 32 are error codes.
    ensure!(result as usize > 32, "cannot open {uri}: {}", io::Error::last_os_error());
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
