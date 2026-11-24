//! A process's exit as a future: the system's thread pool waits, no thread of ours.

use std::{
    ffi::c_void,
    io,
    pin::Pin,
    ptr,
    sync::{Arc, Mutex, PoisonError},
    task::{Context, Poll, Waker},
};

use anyhow::{Result, ensure};
use blocking::unblock;
use model::{FailureKind, Process};
use windows_sys::Win32::{
    Foundation::{HANDLE, INVALID_HANDLE_VALUE},
    System::Threading::{INFINITE, PROCESS_SYNCHRONIZE, RegisterWaitForSingleObject, UnregisterWaitEx, WT_EXECUTEONLYONCE},
};

use crate::{handle::Handle, inject, process};

/// Resolves with the first of its processes to exit; dropping it ends the waits.
pub struct Exit {
    targets: Vec<Process>,
    shared: Arc<Mutex<Shared>>,
    _waits: Vec<Wait>,
}

#[derive(Default)]
struct Shared {
    exited: Option<usize>,
    waker: Option<Waker>,
}

/// One registered wait; unregistered before its process handle and callback context go.
struct Wait {
    wait: HANDLE,
    _process: Handle,
    // An `Arc`, not a `Box`: the callback's pointer stays valid when the `Wait` moves.
    _slot: Arc<Slot>,
}

struct Slot {
    shared: Arc<Mutex<Shared>>,
    index: usize,
}

// SAFETY: wait and process handles are not tied to the thread that opened them.
unsafe impl Send for Wait {}

/// Watches `targets` (blocking: opens each). One gone already (or its pid reused) has exited; a denied open is `Protected`.
pub fn exit(targets: &[Process]) -> Result<Exit> {
    let shared = Arc::new(Mutex::new(Shared::default()));
    let mut waits = Vec::with_capacity(targets.len());
    for (index, target) in targets.iter().enumerate() {
        let process = match process::open(target, PROCESS_SYNCHRONIZE) {
            Ok(process) => process,
            Err(e) if inject::marked(&e, FailureKind::Protected) => return Err(e),
            Err(_) => {
                shared.lock().unwrap_or_else(PoisonError::into_inner).exited.get_or_insert(index);
                break;
            }
        };
        let slot = Arc::new(Slot { shared: shared.clone(), index });
        let mut wait = ptr::null_mut();
        // SAFETY: the handle and the slot live in the `Wait`, whose drop unregisters before either goes.
        let registered = unsafe {
            RegisterWaitForSingleObject(&mut wait, process.0, Some(exited), Arc::as_ptr(&slot).cast(), INFINITE, WT_EXECUTEONLYONCE)
        };
        ensure!(registered != 0, "cannot watch process {}: {}", target.pid, io::Error::last_os_error());
        waits.push(Wait { wait, _process: process, _slot: slot });
    }
    Ok(Exit { targets: targets.to_vec(), shared, _waits: waits })
}

unsafe extern "system" fn exited(context: *mut c_void, _timed_out: bool) {
    // SAFETY: `context` is a live `Slot`: its `Wait` waits for this callback to return before it frees it.
    let slot = unsafe { &*context.cast::<Slot>() };
    let waker = {
        let mut shared = slot.shared.lock().unwrap_or_else(PoisonError::into_inner);
        shared.exited.get_or_insert(slot.index);
        shared.waker.take()
    };
    // Off this thread: a waker that polls and drops the `Exit` inline would wait here for this very callback.
    if let Some(waker) = waker {
        unblock(move || waker.wake()).detach();
    }
}

impl Future for Exit {
    type Output = Process;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Process> {
        let mut shared = self.shared.lock().unwrap_or_else(PoisonError::into_inner);
        match shared.exited {
            Some(index) => Poll::Ready(self.targets[index].clone()),
            None => {
                shared.waker = Some(cx.waker().clone());
                Poll::Pending
            }
        }
    }
}

impl Drop for Wait {
    fn drop(&mut self) {
        // SAFETY: a registered wait, unregistered once; INVALID_HANDLE_VALUE waits out a callback that is running.
        unsafe { UnregisterWaitEx(self.wait, INVALID_HANDLE_VALUE) };
    }
}

#[cfg(test)]
mod tests {
    use std::process::{Command, Stdio};

    use futures::{FutureExt, executor::block_on};

    use super::*;

    /// A throwaway `ping`, never a game.
    #[test]
    fn the_process_that_exits_ends_the_wait() {
        let me = crate::of(std::process::id()).unwrap();
        let mut ping = Command::new(r"C:\Windows\System32\PING.EXE").args(["-n", "30", "127.0.0.1"]).stdout(Stdio::null()).spawn().unwrap();
        let target = crate::of(ping.id()).unwrap();
        let mut both = exit(&[me.clone(), target.clone()]).unwrap();
        assert_eq!((&mut both).now_or_never(), None, "both run");
        drop(exit(std::slice::from_ref(&me)).unwrap());
        ping.kill().unwrap();
        ping.wait().unwrap();
        assert_eq!(block_on(both), target);
        let reused = Process { created: me.created + 1, ..me };
        assert_eq!(block_on(exit(std::slice::from_ref(&reused)).unwrap()), reused, "its pid is another's now");
    }
}
