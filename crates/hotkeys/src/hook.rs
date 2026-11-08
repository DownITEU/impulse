//! The keyboard on a thread of its own: `WH_KEYBOARD_LL` and the message loop that runs it.

use std::{
    cell::RefCell,
    io, mem, ptr,
    sync::mpsc,
    thread::{self, JoinHandle},
};

use windows_sys::Win32::{
    Foundation::{LPARAM, LRESULT, WPARAM},
    System::{LibraryLoader::GetModuleHandleW, Threading::GetCurrentThreadId},
    UI::WindowsAndMessaging::{
        CallNextHookEx, GetForegroundWindow, GetMessageW, GetWindowThreadProcessId, HC_ACTION, KBDLLHOOKSTRUCT, MSG, PM_NOREMOVE, PeekMessageW, PostThreadMessageW,
        SetWindowsHookExW, UnhookWindowsHookEx, WH_KEYBOARD_LL, WM_KEYDOWN, WM_QUIT, WM_SYSKEYDOWN, WM_USER,
    },
};

type Handler = Box<dyn FnMut(u8, bool) -> bool>;

thread_local! {
    static KEYS: RefCell<Option<Keys>> = const { RefCell::new(None) };
}

/// Not the tao thread: that one runs WebView2 work, and Windows removes a hook that stalls.
/// Dropping it removes the hook. Hooks started later see keys first. Windows skips it while this
/// process is in front if the process registered raw keyboard input.
pub struct Hook {
    thread: u32,
    join: Option<JoinHandle<()>>,
}

impl Hook {
    /// `handler(vk, down)` runs on the hook thread for every change, never for a repeat or the release of
    /// a key pressed before the hook. Returning true on a down hides that key, and its repeats, from every
    /// other app. It must return at once (Windows times it out): forward to a channel.
    pub fn start(handler: impl FnMut(u8, bool) -> bool + Send + 'static) -> io::Result<Self> {
        let (ready, started) = mpsc::sync_channel(1);
        let join = thread::Builder::new().name("hotkeys".into()).spawn(move || {
            KEYS.set(Some(Keys { handler: Box::new(handler), state: [State::Up; 256] }));
            // SAFETY: plain Win32 calls on this thread; `msg` outlives them.
            unsafe {
                let mut msg: MSG = mem::zeroed();
                // Creates the queue `Drop` posts to.
                PeekMessageW(&mut msg, ptr::null_mut(), WM_USER, WM_USER, PM_NOREMOVE);
                let hook = SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard), GetModuleHandleW(ptr::null()), 0);
                if hook.is_null() {
                    let _ = ready.send(Err(io::Error::last_os_error()));
                    return;
                }
                let _ = ready.send(Ok(GetCurrentThreadId()));
                while GetMessageW(&mut msg, ptr::null_mut(), 0, 0) > 0 {}
                UnhookWindowsHookEx(hook);
            }
        })?;
        let thread = started.recv().map_err(|_| io::Error::other("the hook thread ended"))??;
        Ok(Self { thread, join: Some(join) })
    }
}

impl Drop for Hook {
    fn drop(&mut self) {
        // SAFETY: posts to the hook thread's queue, made before `start` returned.
        unsafe { PostThreadMessageW(self.thread, WM_QUIT, 0, 0) };
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

/// The process that owns the foreground window; 0 for none.
pub fn foreground_pid() -> u32 {
    let mut pid = 0;
    // SAFETY: plain Win32 calls; `pid` outlives them.
    unsafe { GetWindowThreadProcessId(GetForegroundWindow(), &mut pid) };
    pid
}

unsafe extern "system" fn keyboard(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code == HC_ACTION as i32 {
        // SAFETY: for HC_ACTION, `lparam` points to the event.
        let info = unsafe { &*(lparam as *const KBDLLHOOKSTRUCT) };
        let down = matches!(wparam as u32, WM_KEYDOWN | WM_SYSKEYDOWN);
        // Injected input can put anything in `vkCode`.
        if let Ok(vk @ 1..) = u8::try_from(info.vkCode)
            && KEYS.with_borrow_mut(|keys| keys.as_mut().is_some_and(|keys| keys.key(vk, down)))
        {
            return 1;
        }
    }
    // SAFETY: passes the event on unchanged.
    unsafe { CallNextHookEx(ptr::null_mut(), code, wparam, lparam) }
}

#[derive(Clone, Copy, PartialEq)]
enum State {
    Up,
    Shown,
    Hidden,
}

/// The hook reports autorepeat as more downs: a repeat keeps its first down's verdict.
struct Keys {
    handler: Handler,
    state: [State; 256],
}

impl Keys {
    fn key(&mut self, vk: u8, down: bool) -> bool {
        let state = &mut self.state[usize::from(vk)];
        match (down, *state) {
            (true, State::Up) => {
                let hide = (self.handler)(vk, true);
                *state = if hide { State::Hidden } else { State::Shown };
                hide
            }
            (true, held) => held == State::Hidden,
            (false, State::Up) => false,
            (false, _) => {
                *state = State::Up;
                (self.handler)(vk, false);
                false
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use super::*;

    #[test]
    fn repeats_keep_the_first_verdict_and_stray_releases_are_dropped() {
        let seen = Arc::new(Mutex::new(Vec::new()));
        let log = seen.clone();
        let handler = move |vk, down| {
            log.lock().unwrap().push((vk, down));
            vk == 0x70
        };
        let mut keys = Keys { handler: Box::new(handler), state: [State::Up; 256] };
        let verdicts: Vec<bool> = [(0x41, false), (0x70, true), (0x70, true), (0x70, false), (0x41, true), (0x41, true)]
            .into_iter()
            .map(|(vk, down)| keys.key(vk, down))
            .collect();
        assert_eq!(verdicts, [false, true, true, false, false, false]);
        assert_eq!(*seen.lock().unwrap(), [(0x70, true), (0x70, false), (0x41, true)]);
    }

    #[test]
    fn the_hook_starts_and_stops() {
        drop(Hook::start(|_, _| false).unwrap());
    }

    /// Injects F24; the hook hides the press, and the lone release reaches nothing bound to it.
    /// Run with `cargo test -p hotkeys -- --ignored`.
    #[test]
    #[ignore = "injects input"]
    fn the_hook_sees_injected_keys() {
        use std::time::Duration;
        use windows_sys::Win32::UI::Input::KeyboardAndMouse::{INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, SendInput, VK_F24};

        let (tx, seen) = mpsc::channel();
        let hook = Hook::start(move |vk, down| {
            let ours = vk == VK_F24 as u8;
            if ours {
                let _ = tx.send(down);
            }
            ours
        })
        .unwrap();
        let key = |flags| INPUT { r#type: INPUT_KEYBOARD, Anonymous: INPUT_0 { ki: KEYBDINPUT { wVk: VK_F24, wScan: 0, dwFlags: flags, time: 0, dwExtraInfo: 0 } } };
        let inputs = [key(0), key(KEYEVENTF_KEYUP)];
        // SAFETY: a valid array of `INPUT`.
        assert_eq!(unsafe { SendInput(2, inputs.as_ptr(), mem::size_of::<INPUT>() as i32) }, 2);
        let wait = || seen.recv_timeout(Duration::from_secs(2));
        assert_eq!((wait(), wait()), (Ok(true), Ok(false)));
        drop(hook);
    }
}
