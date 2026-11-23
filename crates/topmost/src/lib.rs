//! Impulse over the game in front: the foreground window's changes, and the window calls that put it over the
//! game with the input and give the input back. The keys come from the app's keyboard hook, not from here.

mod typed;
mod window;

use std::{
    cell::RefCell,
    io, mem, ptr,
    sync::mpsc,
    thread::{self, JoinHandle},
};

pub use typed::{Held, Typed, typed};
pub use window::{
    Away, Monitor, client_area, cloak, exclude_from_capture, exclusive, foreground, give_back, hide, is_window, lower, monitor_of, owner,
    pass_through, place, post_input, put_away, raise, resize, restore, show, window_of,
};
use windows_sys::Win32::{
    Foundation::HWND,
    System::Threading::GetCurrentThreadId,
    UI::{
        Accessibility::{HWINEVENTHOOK, SetWinEventHook, UnhookWinEvent},
        WindowsAndMessaging::{
            EVENT_SYSTEM_FOREGROUND, GetMessageW, MSG, PM_NOREMOVE, PeekMessageW, PostThreadMessageW, WINEVENT_OUTOFCONTEXT, WM_QUIT,
            WM_USER,
        },
    },
};

type Sink = Box<dyn FnMut()>;

thread_local! {
    static SINK: RefCell<Option<Sink>> = const { RefCell::new(None) };
}

/// Dropping it stops the watch.
pub struct Watch {
    thread: u32,
    join: Option<JoinHandle<()>>,
}

impl Watch {
    /// Watches the foreground window. `on_change` runs on the watch's thread on every change: forward it to a
    /// channel, and read which window with `foreground`, as the event may be late.
    pub fn start(on_change: impl FnMut() + Send + 'static) -> io::Result<Self> {
        let (ready, started) = mpsc::sync_channel(1);
        let join = thread::Builder::new().name("topmost".into()).spawn(move || {
            SINK.set(Some(Box::new(on_change)));
            // SAFETY: plain Win32 calls on this thread; `msg` outlives them.
            unsafe {
                let mut msg: MSG = mem::zeroed();
                // Creates the queue `Drop` posts to.
                PeekMessageW(&mut msg, ptr::null_mut(), WM_USER, WM_USER, PM_NOREMOVE);
                let hook = SetWinEventHook(
                    EVENT_SYSTEM_FOREGROUND,
                    EVENT_SYSTEM_FOREGROUND,
                    ptr::null_mut(),
                    Some(foreground_changed),
                    0,
                    0,
                    WINEVENT_OUTOFCONTEXT,
                );
                if hook.is_null() {
                    let _ = ready.send(Err(io::Error::other("cannot watch the foreground window")));
                    return;
                }
                let _ = ready.send(Ok(GetCurrentThreadId()));
                // The hook is called from inside this loop.
                while GetMessageW(&mut msg, ptr::null_mut(), 0, 0) > 0 {}
                UnhookWinEvent(hook);
            }
        })?;
        let thread = started.recv().map_err(|_| io::Error::other("the topmost thread ended"))??;
        Ok(Self { thread, join: Some(join) })
    }
}

impl Drop for Watch {
    fn drop(&mut self) {
        // SAFETY: posts to the watch thread's queue, made before `start` returned.
        unsafe { PostThreadMessageW(self.thread, WM_QUIT, 0, 0) };
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

// Out of context: Windows calls it on the watch's thread, from its message loop.
unsafe extern "system" fn foreground_changed(_: HWINEVENTHOOK, _: u32, _: HWND, _: i32, _: i32, _: u32, _: u32) {
    SINK.with_borrow_mut(|sink| {
        if let Some(sink) = sink {
            sink();
        }
    });
}
