//! Impulse over the game in front: the system hotkey and the foreground window's changes that call it, and
//! the window calls that put it over the game with the input and give the input back.

mod key;
mod window;

use std::{
    cell::RefCell,
    io, mem, ptr,
    sync::mpsc,
    thread::{self, JoinHandle},
};

pub use key::Key;
pub use window::{
    Monitor, cloak, exclude_from_capture, exclusive, foreground, give_back, hide, is_window, lower, monitor_of, owner, pass_through, place,
    raise, restore, window_of,
};
use windows_sys::Win32::{
    Foundation::HWND,
    System::Threading::GetCurrentThreadId,
    UI::{
        Accessibility::{HWINEVENTHOOK, SetWinEventHook, UnhookWinEvent},
        Input::KeyboardAndMouse::{MOD_NOREPEAT, RegisterHotKey, UnregisterHotKey},
        WindowsAndMessaging::{
            EVENT_SYSTEM_FOREGROUND, GetMessageW, MSG, PM_NOREMOVE, PeekMessageW, PostThreadMessageW, WINEVENT_OUTOFCONTEXT, WM_HOTKEY,
            WM_QUIT, WM_USER,
        },
    },
};

// Hotkey ids: the overlay's, then the extras' (`EXTRAS` of them, set by the thread message).
const OVERLAY: usize = 1;
const FIRST_EXTRA: usize = 2;
/// How many extra hotkeys a `Watch` holds, numbered from 0.
pub const EXTRAS: usize = 3;
const LAST_EXTRA: usize = FIRST_EXTRA + EXTRAS;
const SET_EXTRA: u32 = WM_USER + 1;

pub enum Event {
    Hotkey,
    /// The extra hotkey of this number was pressed.
    Extra(usize),
    /// Another window came in front; read which with `foreground`, as the event may be late.
    Foreground,
}

type Sink = Box<dyn FnMut(Event)>;

thread_local! {
    static SINK: RefCell<Option<Sink>> = const { RefCell::new(None) };
}

/// Dropping it frees the hotkey and stops the watch.
pub struct Watch {
    thread: u32,
    join: Option<JoinHandle<()>>,
    // `set_extra`'s answers.
    set: mpsc::Receiver<io::Result<()>>,
}

impl Watch {
    /// A registered hotkey, not a hook: its press lets this process take the foreground. `on_event` runs on
    /// the watch's thread: forward it to a channel.
    pub fn start(key: Key, on_event: impl FnMut(Event) + Send + 'static) -> io::Result<Self> {
        let (ready, started) = mpsc::sync_channel(1);
        let (answer, set) = mpsc::channel();
        let join = thread::Builder::new().name("overlay".into()).spawn(move || {
            SINK.set(Some(Box::new(on_event)));
            // SAFETY: plain Win32 calls on this thread; `msg` outlives them.
            unsafe {
                let mut msg: MSG = mem::zeroed();
                // Creates the queue `Drop` posts to.
                PeekMessageW(&mut msg, ptr::null_mut(), WM_USER, WM_USER, PM_NOREMOVE);
                if RegisterHotKey(ptr::null_mut(), OVERLAY as i32, key.modifiers | MOD_NOREPEAT, key.vk.into()) == 0 {
                    let _ = ready.send(Err(io::Error::last_os_error()));
                    return;
                }
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
                    UnregisterHotKey(ptr::null_mut(), OVERLAY as i32);
                    let _ = ready.send(Err(io::Error::other("cannot watch the foreground window")));
                    return;
                }
                let _ = ready.send(Ok(GetCurrentThreadId()));
                while GetMessageW(&mut msg, ptr::null_mut(), 0, 0) > 0 {
                    match (msg.message, msg.wParam) {
                        (WM_HOTKEY, OVERLAY) => emit(Event::Hotkey),
                        (WM_HOTKEY, id @ FIRST_EXTRA..LAST_EXTRA) => emit(Event::Extra(id - FIRST_EXTRA)),
                        (SET_EXTRA, packed) => {
                            let id = (FIRST_EXTRA + msg.lParam as usize) as i32;
                            UnregisterHotKey(ptr::null_mut(), id);
                            let taken = Key::unpack(packed)
                                .is_some_and(|key| RegisterHotKey(ptr::null_mut(), id, key.modifiers | MOD_NOREPEAT, key.vk.into()) == 0);
                            let _ = answer.send(if taken { Err(io::Error::last_os_error()) } else { Ok(()) });
                        }
                        _ => {}
                    }
                }
                UnhookWinEvent(hook);
                for id in OVERLAY..LAST_EXTRA {
                    UnregisterHotKey(ptr::null_mut(), id as i32);
                }
            }
        })?;
        let thread = started.recv().map_err(|_| io::Error::other("the overlay thread ended"))??;
        Ok(Self { thread, join: Some(join), set })
    }

    /// Extra hotkey `number` (below `EXTRAS`), or none; fails when another app holds it, which leaves none.
    pub fn set_extra(&self, number: usize, key: Option<Key>) -> io::Result<()> {
        if number >= EXTRAS {
            return Err(io::Error::other("no such extra hotkey"));
        }
        // SAFETY: posts to the watch thread's queue, made before `start` returned.
        if unsafe { PostThreadMessageW(self.thread, SET_EXTRA, key.map_or(0, Key::pack), number as isize) } == 0 {
            return Err(io::Error::last_os_error());
        }
        self.set.recv().map_err(|_| io::Error::other("the overlay thread ended"))?
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
    emit(Event::Foreground);
}

fn emit(event: Event) {
    SINK.with_borrow_mut(|sink| {
        if let Some(sink) = sink {
            sink(event);
        }
    });
}
