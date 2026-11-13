//! The window calls that put Impulse over the game and give the game the input back. Window handles are
//! `isize`s, as they cross threads. Call them outside GPUI updates: our own window's messages run its
//! callbacks at once.

use std::{mem, ptr};

use windows_sys::Win32::{
    Foundation::{HWND, LPARAM, RECT},
    Graphics::{
        Dwm::{DWMWA_CLOAK, DwmSetWindowAttribute},
        Gdi::{GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromWindow},
    },
    System::Threading::{AttachThreadInput, GetCurrentThreadId},
    UI::{
        HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI},
        WindowsAndMessaging::{
            EnumWindows, GW_OWNER, GWL_EXSTYLE, GetForegroundWindow, GetWindow, GetWindowLongW, GetWindowRect, GetWindowThreadProcessId,
            HWND_NOTOPMOST, HWND_TOPMOST, IsIconic, IsWindow, IsWindowVisible, LWA_ALPHA, SW_HIDE, SW_RESTORE, SW_SHOWNOACTIVATE,
            SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_SHOWWINDOW, SetForegroundWindow, SetLayeredWindowAttributes,
            SetWindowDisplayAffinity, SetWindowLongW, SetWindowPos, ShowWindow, ShowWindowAsync, WDA_EXCLUDEFROMCAPTURE, WDA_NONE,
            WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOPMOST, WS_EX_TRANSPARENT,
        },
    },
};

/// A monitor's top left and size in physical pixels, and its scale.
#[derive(Clone, Copy)]
pub struct Monitor {
    pub origin: (i32, i32),
    pub size: (i32, i32),
    pub scale: f32,
}

/// The window in front, if any.
pub fn foreground() -> Option<isize> {
    // SAFETY: no arguments.
    let window = unsafe { GetForegroundWindow() };
    (!window.is_null()).then_some(window as isize)
}

pub fn is_window(window: isize) -> bool {
    // SAFETY: any value is a valid question.
    unsafe { IsWindow(window as HWND) != 0 }
}

/// The main window of process `pid`: its largest visible top-level window without an owner.
pub fn window_of(pid: u32) -> Option<isize> {
    unsafe extern "system" fn each(window: HWND, found: LPARAM) -> i32 {
        // SAFETY: `found` is the `(pid, best)` below, alive for the whole enumeration.
        let (pid, best) = unsafe { &mut *(found as *mut (u32, Option<(i64, isize)>)) };
        let mut owner = 0;
        // SAFETY: RECT is plain integers, so all zeros is a valid value.
        let mut rect: RECT = unsafe { mem::zeroed() };
        // SAFETY: plain Win32 calls on a window Windows just named; `owner` and `rect` outlive them.
        let ours = unsafe {
            GetWindowThreadProcessId(window, &mut owner) != 0
                && owner == *pid
                && IsWindowVisible(window) != 0
                && GetWindow(window, GW_OWNER).is_null()
                && GetWindowRect(window, &mut rect) != 0
        };
        let area = i64::from(rect.right - rect.left) * i64::from(rect.bottom - rect.top);
        if ours && best.is_none_or(|(most, _)| area > most) {
            *best = Some((area, window as isize));
        }
        1
    }
    let mut found: (u32, Option<(i64, isize)>) = (pid, None);
    // SAFETY: `found` outlives the enumeration, which calls `each` synchronously.
    unsafe { EnumWindows(Some(each), (&raw mut found) as LPARAM) };
    found.1.map(|(_, window)| window)
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

/// Shows `window` topmost over the whole monitor of `over`, with the input.
pub fn raise(window: isize, over: isize) {
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
    }
}

// Windows lets only the process with the last input take it: back over an armed game that is the game's.
// Sharing the input of the window in front for the call is the usual way past that.
unsafe fn take_input(window: HWND) {
    // SAFETY: plain Win32 calls; the attachment is undone before returning.
    unsafe {
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

/// Restores `window` from the taskbar without taking the input.
pub fn unminimize(window: isize) {
    let window = window as HWND;
    // SAFETY: a gone window fails them.
    unsafe {
        if IsIconic(window) != 0 {
            ShowWindow(window, SW_SHOWNOACTIVATE);
        }
    }
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

/// Shows `window` topmost at `rect` (left, top, width, height in physical pixels), without the input.
pub fn place(window: isize, (left, top, width, height): (i32, i32, i32, i32)) {
    // SAFETY: a gone window fails it.
    unsafe { SetWindowPos(window as HWND, HWND_TOPMOST, left, top, width, height, SWP_NOACTIVATE | SWP_SHOWWINDOW) };
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
