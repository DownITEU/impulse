//! The keyboard and mouse on a thread of their own: `WH_KEYBOARD_LL`, `WH_MOUSE_LL` (on when asked) and
//! the message loop that runs them.

use std::{
    cell::RefCell,
    io, mem, ptr,
    sync::mpsc,
    thread::{self, JoinHandle},
};

use model::mouse::{LBUTTON, MBUTTON, RBUTTON, XBUTTON1, XBUTTON2};
use windows_sys::Win32::{
    Foundation::{LPARAM, LRESULT, WPARAM},
    System::{LibraryLoader::GetModuleHandleW, Threading::GetCurrentThreadId},
    UI::Input::KeyboardAndMouse::{GetAsyncKeyState, GetKeyState, VK_CAPITAL},
    UI::WindowsAndMessaging::{
        CallNextHookEx, GetForegroundWindow, GetMessageW, GetWindowThreadProcessId, HC_ACTION, HHOOK, KBDLLHOOKSTRUCT, MSG, MSLLHOOKSTRUCT,
        PM_NOREMOVE, PeekMessageW, PostThreadMessageW, SetWindowsHookExW, UnhookWindowsHookEx, WH_KEYBOARD_LL, WH_MOUSE_LL, WM_KEYDOWN,
        WM_MOUSEMOVE, WM_QUIT, WM_SYSKEYDOWN, WM_USER,
    },
};

use crate::mouse;

type Handler = Box<dyn FnMut(Stroke) -> bool>;

// Asks the hook thread to install (wParam 1) or remove (0) the mouse hook.
const SET_MOUSE: u32 = WM_USER + 1;

/// One key or button change, as the hook saw it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Stroke {
    /// A virtual key, or `model::mouse`'s code for a button or a wheel notch.
    pub vk: u8,
    pub down: bool,
    /// Autorepeat of a key whose first down was shown to the handler.
    pub repeat: bool,
    /// A key already down as the hook started (`down` too): held from now on, nothing fires on it.
    pub held: bool,
    /// The keyboard's scan code; 0 for a mouse code.
    pub scan: u16,
}

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
    /// `handler(stroke)` runs on the hook thread for every change, and for the repeats of a key whose first
    /// down it did not hide (`repeat`, its answer ignored). The keys down as it starts come first (`held`),
    /// then only at their release. Returning true on a down hides that key, and its repeats, from every other
    /// app. It must return at once (Windows times it out): forward to a channel. With `mouse`, the buttons
    /// and wheel notches come too, in `mouse`'s codes; see `set_mouse`.
    pub fn start(mouse: bool, handler: impl FnMut(Stroke) -> bool + Send + 'static) -> io::Result<Self> {
        let (ready, started) = mpsc::sync_channel(1);
        let join = thread::Builder::new().name("keyhook".into()).spawn(move || {
            KEYS.set(Some(Keys { handler: Box::new(handler), state: [State::Up; 256] }));
            // SAFETY: plain Win32 calls on this thread; `msg` outlives them.
            unsafe {
                let mut msg: MSG = mem::zeroed();
                // Creates the queue `Drop` posts to.
                PeekMessageW(&mut msg, ptr::null_mut(), WM_USER, WM_USER, PM_NOREMOVE);
                let keys = SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard), GetModuleHandleW(ptr::null()), 0);
                // After the hook and before the loop that runs it: a release from now on reaches it.
                for vk in held_keys() {
                    KEYS.with_borrow_mut(|keys| keys.as_mut().map(|keys| keys.seed(vk)));
                }
                let mut pointer_hook: HHOOK = ptr::null_mut();
                let started = if keys.is_null() { Err(io::Error::last_os_error()) } else { set_pointer(&mut pointer_hook, mouse) };
                let ok = started.is_ok();
                let _ = ready.send(started.map(|()| GetCurrentThreadId()));
                while ok && GetMessageW(&mut msg, ptr::null_mut(), 0, 0) > 0 {
                    if msg.message == SET_MOUSE {
                        let on = msg.wParam != 0;
                        // A mouse hook refused leaves the buttons unseen: their chords do not fire.
                        let _ = set_pointer(&mut pointer_hook, on);
                        if !on {
                            release_buttons();
                        }
                    }
                }
                let _ = set_pointer(&mut pointer_hook, false);
                if !keys.is_null() {
                    UnhookWindowsHookEx(keys);
                }
            }
        })?;
        let thread = started.recv().map_err(|_| io::Error::other("the hook thread ended"))??;
        Ok(Self { thread, join: Some(join) })
    }

    /// Installs or removes the mouse hook, on the hook thread and without waiting for it; it runs on every
    /// move, so keep it off unless a binding uses the mouse. A button held when it goes off is released.
    pub fn set_mouse(&self, on: bool) -> io::Result<()> {
        // SAFETY: posts to the hook thread's queue, made before `start` returned.
        if unsafe { PostThreadMessageW(self.thread, SET_MOUSE, usize::from(on), 0) } == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }
}

// The keyboard keys down now, by the sided codes the hook reports (no side-less modifier, no button).
fn held_keys() -> impl Iterator<Item = u8> {
    // SAFETY: a plain query of the system's key state.
    (0x08..=0xFE).filter(|vk| !matches!(vk, 0x0A | 0x0B | 0x10..=0x12)).filter(|&vk| unsafe { GetAsyncKeyState(i32::from(vk)) } < 0)
}

/// Whether Caps Lock is on, as the system has it; read on a thread that takes no keyboard messages (the hook's).
pub fn caps_lock() -> bool {
    // SAFETY: a plain query of this thread's key state.
    unsafe { GetKeyState(i32::from(VK_CAPITAL)) & 1 != 0 }
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

/// Idempotent: `hook` holds the mouse hook while it is on.
fn set_pointer(hook: &mut HHOOK, on: bool) -> io::Result<()> {
    if on != hook.is_null() {
        return Ok(());
    }
    // SAFETY: plain Win32 calls on the hook thread; `hook` is ours, set only here.
    unsafe {
        if on {
            let made = SetWindowsHookExW(WH_MOUSE_LL, Some(pointer), GetModuleHandleW(ptr::null()), 0);
            if made.is_null() {
                return Err(io::Error::last_os_error());
            }
            *hook = made;
        } else {
            UnhookWindowsHookEx(*hook);
            *hook = ptr::null_mut();
        }
    }
    Ok(())
}

// A button held when the mouse hook goes off would never be seen up.
fn release_buttons() {
    for vk in [LBUTTON, RBUTTON, MBUTTON, XBUTTON1, XBUTTON2] {
        feed(vk, false, 0);
    }
}

unsafe extern "system" fn keyboard(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code == HC_ACTION as i32 {
        // SAFETY: for HC_ACTION, `lparam` points to the event.
        let info = unsafe { &*(lparam as *const KBDLLHOOKSTRUCT) };
        let down = matches!(wparam as u32, WM_KEYDOWN | WM_SYSKEYDOWN);
        // Injected input can put anything in `vkCode`.
        if let Ok(vk @ 1..) = u8::try_from(info.vkCode)
            && feed(vk, down, info.scanCode as u16)
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
            let hide = feed(vk, down, 0);
            if mouse::is_notch(vk) {
                feed(vk, false, 0);
            }
            if hide {
                return 1;
            }
        }
    }
    // SAFETY: passes the event on unchanged.
    unsafe { CallNextHookEx(ptr::null_mut(), code, wparam, lparam) }
}

fn feed(vk: u8, down: bool, scan: u16) -> bool {
    KEYS.with_borrow_mut(|keys| keys.as_mut().is_some_and(|keys| keys.key(vk, down, scan)))
}

#[derive(Clone, Copy, PartialEq)]
enum State {
    Up,
    Shown,
    Hidden,
    /// Down before the hook: its repeats go nowhere, its release comes.
    Held,
}

/// The hook reports autorepeat as more downs: a repeat keeps its first down's verdict, and only a shown
/// key's repeat reaches the handler.
struct Keys {
    handler: Handler,
    state: [State; 256],
}

impl Keys {
    fn key(&mut self, vk: u8, down: bool, scan: u16) -> bool {
        if down && self.state[usize::from(vk)] == State::Up {
            self.release_stale();
        }
        let state = &mut self.state[usize::from(vk)];
        let stroke = |repeat| Stroke { vk, down, repeat, held: false, scan };
        match (down, *state) {
            (true, State::Up) => {
                let hide = (self.handler)(stroke(false));
                *state = if hide { State::Hidden } else { State::Shown };
                hide
            }
            (true, State::Shown) => {
                (self.handler)(stroke(true));
                false
            }
            (true, State::Hidden) => true,
            (true, State::Held) | (false, State::Up) => false,
            (false, _) => {
                *state = State::Up;
                (self.handler)(stroke(false));
                false
            }
        }
    }

    fn seed(&mut self, vk: u8) {
        self.state[usize::from(vk)] = State::Held;
        (self.handler)(Stroke { vk, down: true, repeat: false, held: true, scan: 0 });
    }

    /// A key let go where the hook does not see (the lock screen, a UAC prompt) would stay held. Only the
    /// shown ones: a hidden down never reached the system's state.
    fn release_stale(&mut self) {
        // SAFETY: a plain query of the system's key state.
        let up = |vk: u8| unsafe { GetAsyncKeyState(i32::from(vk)) } >= 0;
        for vk in 1..=u8::MAX {
            let state = &mut self.state[usize::from(vk)];
            if matches!(*state, State::Shown | State::Held) && !model::mouse::is_mouse(vk) && up(vk) {
                *state = State::Up;
                (self.handler)(Stroke { vk, down: false, repeat: false, held: false, scan: 0 });
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
        let handler = move |stroke: Stroke| {
            log.lock().unwrap().push((stroke.vk, stroke.down, stroke.repeat));
            stroke.vk == 0x70
        };
        let mut keys = Keys { handler: Box::new(handler), state: [State::Up; 256] };
        let verdicts: Vec<bool> = [(0x41, false), (0x70, true), (0x70, true), (0x70, false), (0x41, true), (0x41, true)]
            .into_iter()
            .map(|(vk, down)| keys.key(vk, down, 0))
            .collect();
        assert_eq!(verdicts, [false, true, true, false, false, false]);
        assert_eq!(*seen.lock().unwrap(), [(0x70, true, false), (0x70, false, false), (0x41, true, false), (0x41, true, true)]);
    }

    // Each stroke's key, down and held.
    type Log = Arc<Mutex<Vec<(u8, bool, bool)>>>;

    fn logged() -> (Keys, Log) {
        let seen = Arc::new(Mutex::new(Vec::new()));
        let log = seen.clone();
        let handler = move |stroke: Stroke| {
            log.lock().unwrap().push((stroke.vk, stroke.down, stroke.held));
            false
        };
        (Keys { handler: Box::new(handler), state: [State::Up; 256] }, seen)
    }

    #[test]
    fn a_key_held_before_the_hook_comes_as_held_then_only_at_its_release() {
        let (mut keys, seen) = logged();
        keys.seed(0x41);
        assert_eq!([keys.key(0x41, true, 0), keys.key(0x41, false, 0)], [false, false]);
        assert_eq!(*seen.lock().unwrap(), [(0x41, true, true), (0x41, false, false)]);
    }

    #[test]
    fn a_shown_key_the_system_has_up_is_released_at_the_next_press() {
        // Nobody holds F23 or F24 while the tests run.
        let (mut keys, seen) = logged();
        keys.key(0x86, true, 0);
        keys.key(0x87, true, 0);
        assert_eq!(*seen.lock().unwrap(), [(0x86, true, false), (0x86, false, false), (0x87, true, false)]);
    }

    #[test]
    fn the_hook_starts_and_stops() {
        drop(Hook::start(true, |_| false).unwrap());
    }

    #[test]
    fn the_mouse_hook_comes_and_goes_when_asked() {
        let hook = Hook::start(false, |_| false).unwrap();
        for on in [true, true, false, false] {
            hook.set_mouse(on).unwrap();
        }
        drop(hook);
    }

    /// Injects F24 and a wheel notch up; the hook hides the press and the notch, and the lone F24 release
    /// reaches nothing bound to it. Run with `cargo test -p keyhook -- --ignored`.
    #[test]
    #[ignore = "injects input"]
    fn the_hook_sees_injected_keys_and_notches() {
        use std::time::Duration;
        use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
            INPUT, INPUT_0, INPUT_KEYBOARD, INPUT_MOUSE, KEYBDINPUT, KEYEVENTF_KEYUP, MOUSEEVENTF_WHEEL, MOUSEINPUT, SendInput, VK_F24,
        };

        const OURS: [u8; 2] = [VK_F24 as u8, 0x0A];
        let (tx, seen) = mpsc::channel();
        let hook = Hook::start(true, move |Stroke { vk, down, .. }| {
            let ours = OURS.contains(&vk);
            if ours {
                let _ = tx.send((vk, down));
            }
            ours
        })
        .unwrap();
        let key = |flags| INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 { ki: KEYBDINPUT { wVk: VK_F24, wScan: 0, dwFlags: flags, time: 0, dwExtraInfo: 0 } },
        };
        let notch = MOUSEINPUT { dx: 0, dy: 0, mouseData: 120, dwFlags: MOUSEEVENTF_WHEEL, time: 0, dwExtraInfo: 0 };
        let inputs = [key(0), key(KEYEVENTF_KEYUP), INPUT { r#type: INPUT_MOUSE, Anonymous: INPUT_0 { mi: notch } }];
        // SAFETY: a valid array of `INPUT`.
        assert_eq!(unsafe { SendInput(3, inputs.as_ptr(), mem::size_of::<INPUT>() as i32) }, 3);
        let seen: Vec<_> = (0..4).map_while(|_| seen.recv_timeout(Duration::from_secs(2)).ok()).collect();
        assert_eq!(seen, [(OURS[0], true), (OURS[0], false), (OURS[1], true), (OURS[1], false)]);
        drop(hook);
    }
}
