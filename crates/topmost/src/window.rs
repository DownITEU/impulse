//! The window calls that put Impulse over the game and give the game the input back. Window handles are
//! `isize`s, as they cross threads. Call them outside GPUI updates: our own window's messages run its
//! callbacks at once.

use std::{mem, ptr};

use windows_sys::Win32::{
    Foundation::{HWND, LPARAM, POINT, RECT},
    Graphics::{
        Dwm::{DWMWA_CLOAK, DwmSetWindowAttribute},
        Gdi::{ClientToScreen, GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromWindow},
    },
    System::Threading::{AttachThreadInput, GetCurrentThreadId},
    UI::{
        HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI},
        Input::KeyboardAndMouse::{INPUT, INPUT_MOUSE, SendInput},
        Shell::{QUNS_RUNNING_D3D_FULL_SCREEN, SHQueryUserNotificationState},
        WindowsAndMessaging::{
            EnumWindows, GW_OWNER, GWL_EXSTYLE, GetClientRect, GetForegroundWindow, GetWindow, GetWindowLongW, GetWindowRect,
            GetWindowThreadProcessId, HWND_NOTOPMOST, HWND_TOPMOST, IsIconic, IsWindow, IsWindowVisible, LWA_ALPHA, PostMessageW, SW_HIDE,
            SW_RESTORE, SW_SHOWMINNOACTIVE, SW_SHOWNOACTIVATE, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, SWP_SHOWWINDOW,
            SetForegroundWindow, SetLayeredWindowAttributes, SetWindowDisplayAffinity, SetWindowLongW, SetWindowPos, ShowWindow,
            ShowWindowAsync, WDA_EXCLUDEFROMCAPTURE, WDA_NONE, WM_MOUSEFIRST, WM_MOUSEHWHEEL, WM_MOUSELAST, WM_MOUSEWHEEL, WS_EX_LAYERED,
            WS_EX_NOACTIVATE, WS_EX_TOPMOST, WS_EX_TRANSPARENT,
        },
    },
};

/// A rect on the screen in physical pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Area {
    pub left: i32,
    pub top: i32,
    pub width: i32,
    pub height: i32,
}

/// A monitor's top left and size in physical pixels, and its scale.
#[derive(Clone, Copy)]
pub struct Monitor {
    pub origin: (i32, i32),
    pub size: (i32, i32),
    pub scale: f32,
}

pub fn foreground() -> Option<isize> {
    // SAFETY: no arguments.
    let window = unsafe { GetForegroundWindow() };
    (!window.is_null()).then_some(window as isize)
}

pub fn is_window(window: isize) -> bool {
    // SAFETY: any value is a valid question.
    unsafe { IsWindow(window as HWND) != 0 }
}

/// Minimized to the taskbar.
pub fn is_minimized(window: isize) -> bool {
    // SAFETY: any value is a valid question.
    unsafe { IsIconic(window as HWND) != 0 }
}

/// The main window of process `pid`: its largest visible top-level window without an owner, the first of equals.
pub fn window_of(pid: u32) -> Option<isize> {
    let unowned = visible_of(pid).into_iter().rev().filter(|&(_, owned, _)| !owned);
    unowned.max_by_key(|&(.., area)| area).map(|(window, ..)| window)
}

/// The first visible top-level window of process `pid`, front to back, owned or not.
pub fn first_window_of(pid: u32) -> Option<isize> {
    visible_of(pid).first().map(|&(window, ..)| window)
}

/// The visible top-level windows of process `pid`, front to back: each with whether it has an owner, and its area.
fn visible_of(pid: u32) -> Vec<(isize, bool, i64)> {
    unsafe extern "system" fn each(window: HWND, found: LPARAM) -> i32 {
        // SAFETY: `found` is the `(pid, windows)` below, alive for the whole enumeration.
        let (pid, windows) = unsafe { &mut *(found as *mut (u32, Vec<(isize, bool, i64)>)) };
        let mut owner = 0;
        // SAFETY: RECT is plain integers, so all zeros is a valid value.
        let mut rect: RECT = unsafe { mem::zeroed() };
        // SAFETY: plain Win32 calls on a window Windows just named; `owner` and `rect` outlive them.
        let ours = unsafe {
            GetWindowThreadProcessId(window, &mut owner) != 0
                && owner == *pid
                && IsWindowVisible(window) != 0
                && GetWindowRect(window, &mut rect) != 0
        };
        if ours {
            // SAFETY: a plain query on a window Windows just named.
            let owned = unsafe { !GetWindow(window, GW_OWNER).is_null() };
            windows.push((window as isize, owned, i64::from(rect.right - rect.left) * i64::from(rect.bottom - rect.top)));
        }
        1
    }
    let mut found: (u32, Vec<(isize, bool, i64)>) = (pid, Vec::new());
    // SAFETY: `found` outlives the enumeration, which calls `each` synchronously.
    unsafe { EnumWindows(Some(each), (&raw mut found) as LPARAM) };
    found.1
}

/// Whether `game` is in front in exclusive fullscreen, or in an optimized one that asked for it: a layer over it may
/// not show. Windows answers for the whole session and a few seconds late, so the game must be the window in front,
/// covering its monitor, and this is asked before our window goes over it.
pub fn exclusive(game: isize) -> bool {
    let Some(monitor) = monitor_of(game) else { return false };
    let mut state = 0;
    // SAFETY: RECT is plain integers, so all zeros is a valid value.
    let mut rect: RECT = unsafe { mem::zeroed() };
    // SAFETY: plain Win32 calls; `state` and `rect` outlive them, and a gone window fails them.
    let fullscreen = unsafe {
        GetForegroundWindow() == game as HWND
            && SHQueryUserNotificationState(&mut state) == 0
            && state == QUNS_RUNNING_D3D_FULL_SCREEN
            && GetWindowRect(game as HWND, &mut rect) != 0
    };
    let (left, top) = monitor.origin;
    fullscreen && rect.left <= left && rect.top <= top && rect.right >= left + monitor.size.0 && rect.bottom >= top + monitor.size.1
}

/// Keeps `window` off the screen while it still draws, so a new layout shows only once drawn.
pub fn cloak(window: isize, on: bool) {
    let on = i32::from(on);
    // SAFETY: `on` outlives the call; a gone window fails it.
    unsafe { DwmSetWindowAttribute(window as HWND, DWMWA_CLOAK as u32, (&raw const on).cast(), size_of::<i32>() as u32) };
}

/// Screenshots, recordings and streams show what is behind `window` instead.
pub fn exclude_from_capture(window: isize, on: bool) {
    // SAFETY: a gone window, or a Windows before 10 2004, fails it.
    unsafe { SetWindowDisplayAffinity(window as HWND, if on { WDA_EXCLUDEFROMCAPTURE } else { WDA_NONE }) };
}

/// Shows `window` topmost over the whole monitor of `over`, with the input. Whether `window` is in front after it.
pub fn raise(window: isize, over: isize) -> bool {
    let (window, over) = (window as HWND, over as HWND);
    // SAFETY: plain Win32 calls; `info` outlives them, and gone windows fail them.
    unsafe {
        let monitor = MonitorFromWindow(if IsWindow(over) != 0 { over } else { window }, MONITOR_DEFAULTTONEAREST);
        let mut info = MONITORINFO { cbSize: size_of::<MONITORINFO>() as u32, ..mem::zeroed() };
        if GetMonitorInfoW(monitor, &mut info) != 0 {
            let RECT { left, top, right, bottom } = info.rcMonitor;
            SetWindowPos(window, HWND_TOPMOST, left, top, right - left, bottom - top, SWP_SHOWWINDOW);
        }
        take_input(window);
        GetForegroundWindow() == window
    }
}

// Windows lets only the process with the last input take it: back over an armed game that is the game's.
// Sharing the input of the window in front for the call is the usual way past that.
unsafe fn take_input(window: HWND) {
    // SAFETY: plain Win32 calls; the attachment is undone before returning, and `empty` outlives its call.
    unsafe {
        // A press seen by a low-level hook gives this process no last input, as a hotkey's did; an empty input
        // sent here does (the way PowerToys Run does it, microsoft/PowerToys#4362).
        let mut empty: INPUT = mem::zeroed();
        empty.r#type = INPUT_MOUSE;
        SendInput(1, &empty, size_of::<INPUT>() as i32);
        if SetForegroundWindow(window) != 0 {
            return;
        }
        let front = GetWindowThreadProcessId(GetForegroundWindow(), ptr::null_mut());
        let ours = GetCurrentThreadId();
        if front == 0 || front == ours || AttachThreadInput(ours, front, 1) == 0 {
            return;
        }
        SetForegroundWindow(window);
        AttachThreadInput(ours, front, 0);
    }
}

/// How a window was before `restore`, to be put back with `put_away`.
#[derive(Clone, Copy, PartialEq)]
pub enum Away {
    Shown,
    Minimized,
    Hidden,
}

/// Shows `window` again from the taskbar or the tray, without taking the input.
pub fn restore(window: isize) -> Away {
    let window = window as HWND;
    // SAFETY: a gone window fails them.
    unsafe {
        let away = match (IsWindowVisible(window) != 0, IsIconic(window) != 0) {
            (false, _) => Away::Hidden,
            (true, true) => Away::Minimized,
            (true, false) => return Away::Shown,
        };
        ShowWindow(window, SW_SHOWNOACTIVATE);
        away
    }
}

pub fn put_away(window: isize, away: Away) {
    let show = match away {
        Away::Shown => return,
        Away::Minimized => SW_SHOWMINNOACTIVE,
        Away::Hidden => SW_HIDE,
    };
    // SAFETY: a gone window fails it.
    unsafe { ShowWindow(window as HWND, show) };
}

/// Moves and sizes `window`, keeping its place in the z-order.
pub fn resize(window: isize, Area { left, top, width, height }: Area) {
    // SAFETY: a gone window fails it.
    unsafe { SetWindowPos(window as HWND, ptr::null_mut(), left, top, width, height, SWP_NOZORDER | SWP_NOACTIVATE) };
}

/// Posts a mouse message to `window` as if given there; any other message, keys too (the HUD's come from the
/// keyboard hook), is dropped. A wheel's point is relative to the client area, as the other mouse messages have it:
/// Win32 wants a screen one there.
pub fn post_input(window: isize, message: u32, wparam: usize, lparam: isize) {
    let lparam = match message {
        WM_MOUSEWHEEL | WM_MOUSEHWHEEL => {
            let mut point = POINT { x: i32::from(lparam as i16), y: i32::from((lparam >> 16) as i16) };
            // SAFETY: `point` outlives the call; a gone window fails it.
            unsafe { ClientToScreen(window as HWND, &mut point) };
            (point.x as u16 as isize) | ((point.y as u16 as isize) << 16)
        }
        WM_MOUSEFIRST..=WM_MOUSELAST => lparam,
        _ => return,
    };
    // SAFETY: a post; a gone window fails it.
    unsafe { PostMessageW(window as HWND, message, wparam, lparam) };
}

/// Takes `window` out of the topmost band, right behind `below`.
pub fn lower(window: isize, below: Option<isize>) {
    let window = window as HWND;
    let flags = SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE;
    // SAFETY: plain Win32 calls; gone windows fail them.
    unsafe {
        SetWindowPos(window, HWND_NOTOPMOST, 0, 0, 0, 0, flags);
        // Behind a topmost window it would turn topmost itself; out of the band it is behind that already.
        if let Some(below) =
            below.map(|below| below as HWND).filter(|&below| GetWindowLongW(below, GWL_EXSTYLE) as u32 & WS_EX_TOPMOST == 0)
        {
            SetWindowPos(window, below, 0, 0, 0, 0, flags);
        }
    }
}

/// The process that owns `window`; 0 for a gone one.
pub fn owner(window: isize) -> u32 {
    let mut pid = 0;
    // SAFETY: `pid` outlives the call; a gone window fails it.
    unsafe { GetWindowThreadProcessId(window as HWND, &mut pid) };
    pid
}

/// The monitor `window` is on, or the nearest.
pub fn monitor_of(window: isize) -> Option<Monitor> {
    // SAFETY: plain Win32 calls; `info` and the DPIs outlive them.
    unsafe {
        let monitor = MonitorFromWindow(window as HWND, MONITOR_DEFAULTTONEAREST);
        let mut info = MONITORINFO { cbSize: size_of::<MONITORINFO>() as u32, ..mem::zeroed() };
        let (mut dpi, mut unused) = (0, 0);
        if GetMonitorInfoW(monitor, &mut info) == 0 || GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut dpi, &mut unused) != 0 {
            return None;
        }
        let RECT { left, top, right, bottom } = info.rcMonitor;
        Some(Monitor { origin: (left, top), size: (right - left, bottom - top), scale: dpi as f32 / 96. })
    }
}

/// The client area of `window` on the screen; `None` for a gone window.
pub fn client_area(window: isize) -> Option<Area> {
    let window = window as HWND;
    let mut origin = POINT { x: 0, y: 0 };
    // SAFETY: RECT is plain integers, so all zeros is a valid value.
    let mut rect: RECT = unsafe { mem::zeroed() };
    // SAFETY: plain Win32 calls; `origin` and `rect` outlive them, and a gone window fails them.
    let known = unsafe { GetClientRect(window, &mut rect) != 0 && ClientToScreen(window, &mut origin) != 0 };
    known.then_some(Area { left: origin.x, top: origin.y, width: rect.right - rect.left, height: rect.bottom - rect.top })
}

/// Lets every click and key through `window` to what is under it; it never takes the input.
pub fn pass_through(window: isize) {
    let window = window as HWND;
    // SAFETY: plain Win32 calls; a gone window fails them.
    unsafe {
        let style = GetWindowLongW(window, GWL_EXSTYLE) as u32 | WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_NOACTIVATE;
        SetWindowLongW(window, GWL_EXSTYLE, style as i32);
        // A layered window shows nothing until it has an opacity.
        SetLayeredWindowAttributes(window, 0, 255, LWA_ALPHA);
    }
}

/// Shows `window` topmost at `area`, without the input.
pub fn place(window: isize, Area { left, top, width, height }: Area) {
    // SAFETY: a gone window fails it.
    unsafe { SetWindowPos(window as HWND, HWND_TOPMOST, left, top, width, height, SWP_NOACTIVATE | SWP_SHOWWINDOW) };
}

/// Shows `window` at `area` out of the topmost band, without the input.
pub fn show(window: isize, Area { left, top, width, height }: Area) {
    // SAFETY: a gone window fails it.
    unsafe { SetWindowPos(window as HWND, HWND_NOTOPMOST, left, top, width, height, SWP_NOACTIVATE | SWP_SHOWWINDOW) };
}

pub fn hide(window: isize) {
    // SAFETY: a gone window fails it.
    unsafe { ShowWindow(window as HWND, SW_HIDE) };
}

/// Gives the input back to `game`, restored if it minimized itself.
pub fn give_back(game: isize) {
    let game = game as HWND;
    // SAFETY: plain Win32 calls on a window that may be gone, which they report as failures.
    unsafe {
        if IsWindow(game) == 0 {
            return;
        }
        if IsIconic(game) != 0 {
            // A hung game must not hang the caller.
            ShowWindowAsync(game, SW_RESTORE);
        }
        SetForegroundWindow(game);
    }
}
