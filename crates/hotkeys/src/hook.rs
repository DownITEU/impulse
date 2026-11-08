//! The keyboard and mouse on a thread of their own: `WH_KEYBOARD_LL`, `WH_MOUSE_LL` and the message
//! loop that runs them.

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
        CallNextHookEx, GetForegroundWindow, GetMessageW, GetWindowThreadProcessId, HC_ACTION, HHOOK, HOOKPROC, KBDLLHOOKSTRUCT, MSG, MSLLHOOKSTRUCT, PM_NOREMOVE,
        PeekMessageW, PostThreadMessageW, SetWindowsHookExW, UnhookWindowsHookEx, WH_KEYBOARD_LL, WH_MOUSE_LL, WINDOWS_HOOK_ID, WM_KEYDOWN, WM_MOUSEMOVE, WM_QUIT,
        WM_SYSKEYDOWN, WM_USER,
    },
};

use crate::mouse;

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
    /// other app. It must return at once (Windows times it out): forward to a channel. With `mouse`, the
    /// buttons and wheel notches come too, in `mouse`'s codes; that hook runs on every move, so only
    /// when a binding needs it.
    pub fn start(mouse: bool, handler: impl FnMut(u8, bool) -> bool + Send + 'static) -> io::Result<Self> {
        let (ready, started) = mpsc::sync_channel(1);
        let join = thread::Builder::new().name("hotkeys".into()).spawn(move || {
            KEYS.set(Some(Keys { handler: Box::new(handler), state: [State::Up; 256] }));
            // SAFETY: plain Win32 calls on this thread; `msg` outlives them.
            unsafe {
                let mut msg: MSG = mem::zeroed();
                // Creates the queue `Drop` posts to.
                PeekMessageW(&mut msg, ptr::null_mut(), WM_USER, WM_USER, PM_NOREMOVE);
                let procs: [(WINDOWS_HOOK_ID, HOOKPROC); 2] = [(WH_KEYBOARD_LL, Some(keyboard)), (WH_MOUSE_LL, Some(pointer))];
                let wanted = &procs[..1 + usize::from(mouse)];
                let mut hooks: Vec<HHOOK> = Vec::new();
                for &(id, proc) in wanted {
                    let hook = SetWindowsHookExW(id, proc, GetModuleHandleW(ptr::null()), 0);
                    if hook.is_null() {
                        let _ = ready.send(Err(io::Error::last_os_error()));
                        break;
                    }
                    hooks.push(hook);
                }
                if hooks.len() == wanted.len() {
                    let _ = ready.send(Ok(GetCurrentThreadId()));
                    while GetMessageW(&mut msg, ptr::null_mut(), 0, 0) > 0 {}
                }
                for hook in hooks {
                    UnhookWindowsHookEx(hook);
                }
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
            && feed(vk, down)
        {
            return 1;
        }
    }
    // SAFETY: passes the event on unchanged.
    unsafe { CallNextHookEx(ptr::null_mut(), code, wparam, lparam) }
}

unsafe extern "system" fn pointer(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    // Moves are most of the traffic: they leave first.
    if code == HC_ACTION as i32 && wparam as u32 != WM_MOUSEMOVE {
        // SAFETY: for HC_ACTION, `lparam` points to the event.
        let info = unsafe { &*(lparam as *const MSLLHOOKSTRUCT) };
        if let Some((vk, down)) = mouse::input(wparam as u32, info.mouseData) {
            let hide = feed(vk, down);
            if mouse::is_notch(vk) {
                feed(vk, false);
            }
            if hide {
                return 1;
            }
        }
    }
    // SAFETY: passes the event on unchanged.
    unsafe { CallNextHookEx(ptr::null_mut(), code, wparam, lparam) }
}

fn feed(vk: u8, down: bool) -> bool {
    KEYS.with_borrow_mut(|keys| keys.as_mut().is_some_and(|keys| keys.key(vk, down)))
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
        drop(Hook::start(true, |_, _| false).unwrap());
    }

    /// Injects F24 and a wheel notch up; the hook hides the press and the notch, and the lone F24 release
    /// reaches nothing bound to it. Run with `cargo test -p hotkeys -- --ignored`.
    #[test]
    #[ignore = "injects input"]
    fn the_hook_sees_injected_keys_and_notches() {
        use std::time::Duration;
        use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
            INPUT, INPUT_0, INPUT_KEYBOARD, INPUT_MOUSE, KEYBDINPUT, KEYEVENTF_KEYUP, MOUSEEVENTF_WHEEL, MOUSEINPUT, SendInput, VK_F24,
        };

        const OURS: [u8; 2] = [VK_F24 as u8, 0x0A];
        let (tx, seen) = mpsc::channel();
        let hook = Hook::start(true, move |vk, down| {
            let ours = OURS.contains(&vk);
            if ours {
                let _ = tx.send((vk, down));
            }
            ours
        })
        .unwrap();
        let key = |flags| INPUT { r#type: INPUT_KEYBOARD, Anonymous: INPUT_0 { ki: KEYBDINPUT { wVk: VK_F24, wScan: 0, dwFlags: flags, time: 0, dwExtraInfo: 0 } } };
        let notch = MOUSEINPUT { dx: 0, dy: 0, mouseData: 120, dwFlags: MOUSEEVENTF_WHEEL, time: 0, dwExtraInfo: 0 };
        let inputs = [key(0), key(KEYEVENTF_KEYUP), INPUT { r#type: INPUT_MOUSE, Anonymous: INPUT_0 { mi: notch } }];
        // SAFETY: a valid array of `INPUT`.
        assert_eq!(unsafe { SendInput(3, inputs.as_ptr(), mem::size_of::<INPUT>() as i32) }, 3);
        let seen: Vec<_> = (0..4).map_while(|_| seen.recv_timeout(Duration::from_secs(2)).ok()).collect();
        assert_eq!(seen, [(OURS[0], true), (OURS[0], false), (OURS[1], true), (OURS[1], false)]);
        drop(hook);
    }
}
