//! Named events a game's module sets, such as a service saying it has torn down.

use std::{ffi::OsStr, io, os::windows::ffi::OsStrExt, time::Duration};

use anyhow::{Result, bail, ensure};
use windows_sys::Win32::{
    Foundation::{ERROR_FILE_NOT_FOUND, WAIT_OBJECT_0, WAIT_TIMEOUT},
    System::Threading::{OpenEventW, SYNCHRONIZATION_SYNCHRONIZE, WaitForSingleObject},
};

use crate::handle::Handle;

/// Waits up to `within` for the named event to be set (blocking). `Ok(false)`: no process holds an event of that
/// name, so nothing is underway; an event not set in time is an error.
pub fn signaled(name: &str, within: Duration) -> Result<bool> {
    let wide: Vec<u16> = OsStr::new(name).encode_wide().chain([0]).collect();
    // SAFETY: the name is NUL-terminated and outlives the call.
    let handle = unsafe { OpenEventW(SYNCHRONIZATION_SYNCHRONIZE, 0, wide.as_ptr()) };
    if handle.is_null() {
        let error = io::Error::last_os_error();
        // Held but denied to us is not absent.
        ensure!(error.raw_os_error() == Some(ERROR_FILE_NOT_FOUND as i32), "cannot open {name}: {error}");
        return Ok(false);
    }
    let event = Handle(handle);
    let ms = u32::try_from(within.as_millis()).unwrap_or(u32::MAX - 1);
    // SAFETY: the guard holds a live event handle.
    match unsafe { WaitForSingleObject(event.0, ms) } {
        WAIT_OBJECT_0 => Ok(true),
        WAIT_TIMEOUT => bail!("{name} was not set in {within:?}"),
        _ => bail!("cannot wait for {name}: {}", io::Error::last_os_error()),
    }
}

#[cfg(test)]
mod tests {
    use windows_sys::Win32::System::Threading::{CreateEventW, SetEvent};

    use super::*;

    #[test]
    fn an_event_is_waited_for_only_while_one_is_held() {
        let name = format!("Local\\ImpulseTest_{}", std::process::id());
        assert!(!signaled(&name, Duration::from_secs(5)).unwrap(), "none held: no wait");

        let wide: Vec<u16> = OsStr::new(&name).encode_wide().chain([0]).collect();
        // SAFETY: a manual-reset event, not set, with a NUL-terminated name; the guard closes it.
        let held = Handle(unsafe { CreateEventW(std::ptr::null(), 1, 0, wide.as_ptr()) });
        assert!(signaled(&name, Duration::from_millis(50)).is_err(), "held, not set");
        // SAFETY: a live event handle.
        unsafe { SetEvent(held.0) };
        assert!(signaled(&name, Duration::from_secs(5)).unwrap());
    }
}
