//! Impulse's icon in the notification area and its menu, the window hidden behind it, and the one Impulse
//! of a session. Windows only.
//! The icon lives on a hidden window of the thread that makes it, whose message loop runs its events.

use std::{cell::RefCell, io, mem, ptr, thread, time::Duration};

use windows_sys::Win32::{
    Foundation::{CloseHandle, ERROR_ACCESS_DENIED, ERROR_ALREADY_EXISTS, GetLastError, HWND, LPARAM, LRESULT, POINT, WPARAM},
    System::{
        LibraryLoader::GetModuleHandleW,
        Threading::{CreateMutexW, OpenProcess, PROCESS_SYNCHRONIZE, WaitForSingleObject},
    },
    UI::{
        Shell::{NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NOTIFYICONDATAW, Shell_NotifyIconW},
        WindowsAndMessaging::{
            AllowSetForegroundWindow, AppendMenuW, ChangeWindowMessageFilterEx, CreatePopupMenu, CreateWindowExW, DefWindowProcW,
            DestroyMenu, DestroyWindow, FindWindowW, GWLP_USERDATA, GetCursorPos, GetWindowLongPtrW, GetWindowThreadProcessId, IsIconic,
            LoadIconW, MF_CHECKED, MF_GRAYED, MF_SEPARATOR, MF_STRING, MSGFLT_ALLOW, PostMessageW, RegisterClassW, RegisterWindowMessageW,
            SW_HIDE, SW_RESTORE, SW_SHOW, SetForegroundWindow, SetMenuDefaultItem, SetWindowLongPtrW, ShowWindow, TPM_RETURNCMD,
            TPM_RIGHTBUTTON, TrackPopupMenu, WM_APP, WM_LBUTTONUP, WM_NULL, WM_RBUTTONUP, WNDCLASSW,
        },
    },
};

/// The hidden window's class: a second Impulse finds the first by it.
const CLASS: &str = "ImpulseTray";
const CALLBACK: u32 = WM_APP + 1;
const WAKE: u32 = WM_APP + 2;
// An Impulse started by an update waits this long at most for the one it replaces to end.
const HANDOVER_WAIT: u32 = 15_000;
// Menu ids; 0 is no pick.
const OPEN: usize = 1;
const STOP: usize = 2;
const OVERLAY: usize = 3;
const QUIT: usize = 4;

pub enum Event {
    Open,
    Stop,
    Overlay,
    Quit,
}

/// What the menu shows, read when it opens.
#[derive(Clone, Default)]
pub struct Menu {
    /// The game a trainer runs in.
    pub playing: Option<String>,
    pub over: bool,
    pub words: Words,
}

#[derive(Clone, Default)]
pub struct Words {
    pub open: String,
    pub stop: String,
    pub overlay: String,
    pub quit: String,
}

struct Inner {
    on_event: Box<dyn Fn(Event)>,
    menu: RefCell<Menu>,
    icon: NOTIFYICONDATAW,
    // Explorer's restart drops every icon and says so.
    taskbar_created: u32,
}

pub struct Tray {
    window: HWND,
    inner: *mut Inner,
}

impl Tray {
    /// Adds the icon, the executable's icon resource 1; `on_event` runs on this thread.
    pub fn new(tip: &str, on_event: impl Fn(Event) + 'static) -> io::Result<Self> {
        let class = wide(CLASS);
        // SAFETY: plain Win32 calls; the strings outlive them, and `inner` is freed only in `drop`, after the window.
        unsafe {
            let instance = GetModuleHandleW(ptr::null());
            let mut window_class: WNDCLASSW = mem::zeroed();
            window_class.lpfnWndProc = Some(procedure);
            window_class.hInstance = instance;
            window_class.lpszClassName = class.as_ptr();
            // Fails harmlessly once the class exists.
            RegisterClassW(&window_class);
            let window =
                CreateWindowExW(0, class.as_ptr(), class.as_ptr(), 0, 0, 0, 0, 0, ptr::null_mut(), ptr::null_mut(), instance, ptr::null());
            if window.is_null() {
                return Err(io::Error::last_os_error());
            }
            let mut icon: NOTIFYICONDATAW = mem::zeroed();
            icon.cbSize = mem::size_of::<NOTIFYICONDATAW>() as u32;
            icon.hWnd = window;
            icon.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP;
            icon.uCallbackMessage = CALLBACK;
            icon.hIcon = LoadIconW(instance, 1 as _);
            for (at, unit) in tip.encode_utf16().take(icon.szTip.len() - 1).enumerate() {
                icon.szTip[at] = unit;
            }
            if Shell_NotifyIconW(NIM_ADD, &icon) == 0 {
                DestroyWindow(window);
                return Err(io::Error::other("the notification area refused the icon"));
            }
            let taskbar_created = RegisterWindowMessageW(wide("TaskbarCreated").as_ptr());
            // An elevated Impulse still hears Explorer and a second, plain Impulse.
            for message in [taskbar_created, WAKE] {
                ChangeWindowMessageFilterEx(window, message, MSGFLT_ALLOW, ptr::null_mut());
            }
            let inner = Box::into_raw(Box::new(Inner { on_event: Box::new(on_event), menu: RefCell::default(), icon, taskbar_created }));
            SetWindowLongPtrW(window, GWLP_USERDATA, inner as isize);
            Ok(Self { window, inner })
        }
    }

    pub fn set_menu(&self, menu: Menu) {
        // SAFETY: `inner` lives as long as `self`, on this thread.
        unsafe { &*self.inner }.menu.replace(menu);
    }
}

impl Drop for Tray {
    fn drop(&mut self) {
        // SAFETY: the window goes first, so no message reaches `inner` once it is freed.
        unsafe {
            Shell_NotifyIconW(NIM_DELETE, &(*self.inner).icon);
            DestroyWindow(self.window);
            drop(Box::from_raw(self.inner));
        }
    }
}

/// Claims the one Impulse of this session. `false`: another runs, and was asked to show its window.
/// `after` is the pid of an Impulse that is ending (an update's restart): it is waited for, not woken.
pub fn claim(after: Option<u32>) -> bool {
    if let Some(pid) = after {
        wait_for_exit(pid);
    }
    // SAFETY: a plain call with a NUL-terminated name. The handle stays open for the process's life.
    let other = unsafe {
        let mutex = CreateMutexW(ptr::null(), 0, wide(r"Local\Impulse").as_ptr());
        // An elevated Impulse's mutex is there but closed to a plain one.
        let error = GetLastError();
        error == ERROR_ALREADY_EXISTS || mutex.is_null() && error == ERROR_ACCESS_DENIED
    };
    if other {
        // Its icon's window may not be there yet.
        for _ in 0..50 {
            if wake() {
                break;
            }
            thread::sleep(Duration::from_millis(100));
        }
    }
    !other
}

fn wait_for_exit(pid: u32) {
    // SAFETY: plain calls; the handle is closed here, and a pid that is gone only fails the open.
    unsafe {
        let process = OpenProcess(PROCESS_SYNCHRONIZE, 0, pid);
        if !process.is_null() {
            WaitForSingleObject(process, HANDOVER_WAIT);
            CloseHandle(process);
        }
    }
}

/// Asks the Impulse running to show its window. `false`: no icon of one yet.
fn wake() -> bool {
    // SAFETY: plain Win32 calls with a NUL-terminated class name.
    unsafe {
        let running = FindWindowW(wide(CLASS).as_ptr(), ptr::null());
        if running.is_null() {
            return false;
        }
        // The one the user just started may hand it the foreground.
        let mut pid = 0;
        GetWindowThreadProcessId(running, &mut pid);
        AllowSetForegroundWindow(pid);
        PostMessageW(running, WAKE, 0, 0) != 0
    }
}

pub fn hide(window: isize) {
    // SAFETY: a stale handle only fails the call.
    unsafe { ShowWindow(window as HWND, SW_HIDE) };
}

/// Shows the window in front, restored if it was minimized.
pub fn show(window: isize) {
    let window = window as HWND;
    // SAFETY: a stale handle only fails the calls.
    unsafe {
        ShowWindow(window, if IsIconic(window) != 0 { SW_RESTORE } else { SW_SHOW });
        SetForegroundWindow(window);
    }
}

unsafe extern "system" fn procedure(window: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    // SAFETY: the pointer is set right after the window is made and the window goes before it is freed.
    let Some(inner) = (unsafe { (GetWindowLongPtrW(window, GWLP_USERDATA) as *const Inner).as_ref() }) else {
        // SAFETY: forwards the arguments Windows gave this procedure, for the window it is running for.
        return unsafe { DefWindowProcW(window, message, wparam, lparam) };
    };
    match message {
        CALLBACK if lparam as u32 == WM_LBUTTONUP => (inner.on_event)(Event::Open),
        CALLBACK if lparam as u32 == WM_RBUTTONUP => {
            // A copy: the menu's loop runs GPUI, which may set the next one meanwhile.
            let menu = inner.menu.borrow().clone();
            if let Some(event) = pick(window, &menu) {
                (inner.on_event)(event);
            }
        }
        CALLBACK => {}
        WAKE => (inner.on_event)(Event::Open),
        // SAFETY: the icon's data is whole.
        _ if message == inner.taskbar_created => drop(unsafe { Shell_NotifyIconW(NIM_ADD, &inner.icon) }),
        // SAFETY: forwards the arguments Windows gave this procedure, for the window it is running for.
        _ => return unsafe { DefWindowProcW(window, message, wparam, lparam) },
    }
    0
}

/// Shows the menu at the cursor and waits for a pick.
fn pick(window: HWND, menu: &Menu) -> Option<Event> {
    // SAFETY: plain Win32 calls; the menu is destroyed before returning and the strings outlive the calls.
    let id = unsafe {
        let popup = CreatePopupMenu();
        let item = |flags, id, text: &str| AppendMenuW(popup, flags, id, wide(text).as_ptr());
        let Words { open, stop, overlay, quit } = &menu.words;
        item(MF_STRING, OPEN, open);
        SetMenuDefaultItem(popup, OPEN as u32, 0);
        if let Some(game) = &menu.playing {
            item(MF_SEPARATOR, 0, "");
            item(MF_STRING | MF_GRAYED, 0, game);
            item(MF_STRING, STOP, stop);
            item(MF_STRING | if menu.over { MF_CHECKED } else { 0 }, OVERLAY, overlay);
        }
        item(MF_SEPARATOR, 0, "");
        item(MF_STRING, QUIT, quit);
        let mut at = POINT { x: 0, y: 0 };
        GetCursorPos(&mut at);
        // Without the foreground the menu would not close on a click beside it.
        SetForegroundWindow(window);
        let id = TrackPopupMenu(popup, TPM_RETURNCMD | TPM_RIGHTBUTTON, at.x, at.y, 0, window, ptr::null());
        PostMessageW(window, WM_NULL, 0, 0);
        DestroyMenu(popup);
        id as usize
    };
    match id {
        OPEN => Some(Event::Open),
        STOP => Some(Event::Stop),
        OVERLAY => Some(Event::Overlay),
        QUIT => Some(Event::Quit),
        _ => None,
    }
}

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain([0]).collect()
}
