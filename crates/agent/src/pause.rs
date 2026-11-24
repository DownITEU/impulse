//! Holding a game still while a trainer sets up.

use std::time::Duration;

use anyhow::{Result, ensure};
use model::Process;
use windows_sys::Win32::{
    Foundation::{HANDLE, NTSTATUS},
    System::Threading::{PROCESS_SUSPEND_RESUME, PROCESS_SYNCHRONIZE, WaitForInputIdle},
};

use crate::process;

#[link(name = "ntdll", kind = "raw-dylib")]
unsafe extern "system" {
    fn NtSuspendProcess(process: HANDLE) -> NTSTATUS;
    fn NtResumeProcess(process: HANDLE) -> NTSTATUS;
}

/// Waits up to `within` for the game to take input (blocking). `false`: still busy then, or no window to take any.
pub fn idle(target: &Process, within: Duration) -> Result<bool> {
    let process = process::open(target, PROCESS_SYNCHRONIZE)?;
    let ms = u32::try_from(within.as_millis()).unwrap_or(u32::MAX - 1);
    // SAFETY: a live handle with SYNCHRONIZE. Any WAIT_FAILED is `false`: no GUI means no input to wait for.
    Ok(unsafe { WaitForInputIdle(process.0, ms) } == 0)
}

/// Suspends every thread of `target` (blocking); `resume` undoes it.
pub fn suspend(target: &Process) -> Result<()> {
    call(target, NtSuspendProcess, "suspend")
}

pub fn resume(target: &Process) -> Result<()> {
    call(target, NtResumeProcess, "resume")
}

fn call(target: &Process, operation: unsafe extern "system" fn(HANDLE) -> NTSTATUS, what: &str) -> Result<()> {
    let process = process::open(target, PROCESS_SUSPEND_RESUME)?;
    // SAFETY: a live handle with PROCESS_SUSPEND_RESUME, the one right these calls need.
    let status = unsafe { operation(process.0) };
    ensure!(status >= 0, "cannot {what} process {}: NTSTATUS {:#x}", target.pid, status as u32);
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::process::{Command, Stdio};

    use super::*;

    /// A throwaway `ping`, never a game: it has no window, and its output stops while it is held.
    #[test]
    fn a_process_is_held_and_let_go() {
        let mut ping = Command::new(r"C:\Windows\System32\PING.EXE").args(["-n", "30", "127.0.0.1"]).stdout(Stdio::null()).spawn().unwrap();
        let target = crate::of(ping.id()).unwrap();
        let outcome = (|| -> Result<()> {
            ensure!(!idle(&target, Duration::from_millis(100))?, "no window: no input to wait for");
            suspend(&target)?;
            resume(&target)?;
            let reused = Process { created: target.created + 1, ..target.clone() };
            ensure!(suspend(&reused).is_err(), "another process under the same id");
            Ok(())
        })();
        ping.kill().unwrap();
        ping.wait().unwrap();
        outcome.unwrap();
    }
}
