//! The host the overlay DLL dials: a hidden window of ours with the title the DLL looks for, on a thread of its own,
//! one per app. Every message to the DLL goes from that thread, never from a `Layers` call or inside the DLL's own send.

use std::{
    ffi::c_void,
    mem::{self, transmute},
    ptr,
    sync::{
        LazyLock, Mutex, MutexGuard, PoisonError,
        atomic::{AtomicBool, AtomicIsize, Ordering},
        mpsc::{SyncSender, sync_channel},
    },
    thread,
};

use anyhow::{Context, Result, anyhow};
use futures::{StreamExt, channel::mpsc};
use model::{Canvas, CanvasEvent, Cursor, Failure, FailureKind, Layers, Process, Rect, Texture};
use windows_sys::Win32::{
    Foundation::{ERROR_INVALID_WINDOW_HANDLE, GetLastError, HWND, LPARAM, LRESULT, WPARAM},
    System::{DataExchange::COPYDATASTRUCT, Threading::GetCurrentProcessId},
    UI::WindowsAndMessaging::{
        CallWindowProcA, CreateWindowExA, DispatchMessageA, FindWindowExA, GWLP_WNDPROC, GetMessageA, GetWindowThreadProcessId, KillTimer,
        MSG, PostMessageA, SMTO_ABORTIFHUNG, SendMessageTimeoutA, SetTimer, SetWindowLongPtrA, SetWindowTextA, TranslateMessage, WM_APP,
        WM_COPYDATA, WM_TIMER, WNDPROC, WS_POPUP,
    },
};

pub use super::state::Load;
use super::{
    state::{Bound, State},
    wire::{self, FromDll},
};

/// The DLL finds its host by this title, fixed in Wand's build.
const TITLE: &[u8] = b"x-node-FB3795FA-AE8E-4138-B7D8-621E1DEDB721\0";
const IDLE_TITLE: &[u8] = b"impulse-overlay-host-resting\0";
const CLASS: &[u8] = b"STATIC\0";
const IPC: u32 = WM_APP + 0x200;
const CONNECT: u32 = IPC + 1;
const CONNECT_ACK: u32 = IPC + 2;
const CLOSE: u32 = IPC + 3;
/// Ours: there is something to send.
const WAKE: u32 = WM_APP + 0x300;
/// The DLL drops its frames about 50 ms after it reports a new size; a timed-out send is tried again as late.
const RESEND_MS: u32 = 250;
const SEND_WITHIN_MS: u32 = 200;

static STATE: LazyLock<Mutex<State>> = LazyLock::new(Mutex::default);
static STARTING: Mutex<()> = Mutex::new(());
static ORIGINAL: AtomicIsize = AtomicIsize::new(0);
static TITLED_IDLE: AtomicBool = AtomicBool::new(false);

fn state() -> MutexGuard<'static, State> {
    STATE.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Blocking. Starts the host once; `Refused` while another (Wand's) holds the title.
pub fn start() -> Result<()> {
    let _one = STARTING.lock().unwrap_or_else(PoisonError::into_inner);
    let ours = state().window;
    if another_host(ours) {
        return Err(anyhow!("another overlay host runs (Wand's own)").context(Failure::new(FailureKind::Refused)));
    }
    // A resting host wakes on `bind` only: a failed `enter` leaves it resting.
    if ours != 0 {
        return Ok(());
    }
    let (ready, started) = sync_channel(1);
    thread::Builder::new().name("overlay host".into()).spawn(move || run(&ready)).context("cannot start the overlay host")?;
    started.recv().context("the overlay host ended")?
}

fn another_host(ours: isize) -> bool {
    let mut after = ptr::null_mut();
    loop {
        // SAFETY: a lookup by a static class and title, after a window handle it returned.
        after = unsafe { FindWindowExA(ptr::null_mut(), after, CLASS.as_ptr(), TITLE.as_ptr()) };
        if after.is_null() || after as isize != ours {
            return !after.is_null();
        }
    }
}

/// Starts resting: the first `bind` gives it the title the DLL looks for.
fn run(ready: &SyncSender<Result<()>>) {
    state().dormant = true;
    TITLED_IDLE.store(true, Ordering::Relaxed);
    // SAFETY: a hidden window, subclassed before anything can find it.
    let window = unsafe {
        let window = CreateWindowExA(
            0,
            CLASS.as_ptr(),
            IDLE_TITLE.as_ptr(),
            WS_POPUP,
            0,
            0,
            0,
            0,
            ptr::null_mut(),
            ptr::null_mut(),
            ptr::null_mut(),
            ptr::null(),
        );
        if !window.is_null() {
            ORIGINAL.store(SetWindowLongPtrA(window, GWLP_WNDPROC, proc as *const () as isize), Ordering::Relaxed);
        }
        window
    };
    if window.is_null() {
        let _ = ready.send(Err(anyhow!("cannot make the overlay host's window: {}", std::io::Error::last_os_error())));
        return;
    }
    state().window = window as isize;
    let _ = ready.send(Ok(()));
    // SAFETY: a plain message loop on the window's own thread; `msg` outlives each call.
    unsafe {
        let mut msg: MSG = std::mem::zeroed();
        while GetMessageA(&mut msg, ptr::null_mut(), 0, 0) > 0 {
            TranslateMessage(&msg);
            DispatchMessageA(&msg);
        }
    }
}

/// `Some(true)`: the DLL is in `process`, `Some(false)`: its load failed, `None`: not tried.
pub fn loaded(process: &Process) -> Option<bool> {
    let state = state();
    let (_, load) = state.mapped.get(&process.pid).filter(|(known, _)| known == process)?;
    Some(!matches!(load, Load::Failed))
}

/// Recorded before the load starts, as the DLL dials in from inside it.
pub fn record(process: &Process, load: Load) {
    state().record(process, load);
}

/// The canvas for the game `pid`, in place of an earlier one, whose events end.
pub fn bind(pid: u32) -> Canvas {
    let (events, received) = mpsc::unbounded();
    let mut state = state();
    let serial = state.bind(pid, events);
    // The last canvas may have closed since `start`.
    state.dormant = false;
    wake(&mut state);
    Canvas { layers: Box::new(Handle { pid, serial }), events: received.boxed() }
}

struct Handle {
    pid: u32,
    serial: u64,
}

impl Handle {
    fn edit(&self, edit: impl FnOnce(&mut Bound)) {
        let mut state = state();
        let Some(bound) = state.canvases.get_mut(&self.pid).filter(|bound| bound.serial == self.serial) else { return };
        edit(bound);
        wake(&mut state);
    }
}

impl Layers for Handle {
    fn show(&self, layer: u32, rect: Rect, frame: Texture) {
        self.edit(|bound| bound.want(layer, Some((rect, frame))));
    }

    fn hide(&self, layer: u32) {
        self.edit(|bound| bound.want(layer, None));
    }

    fn interact(&self, layer: u32, on: bool) {
        self.edit(|bound| bound.interact(layer, on));
    }

    fn block(&self, on: bool) {
        self.edit(|bound| bound.block = on);
    }

    fn cursor(&self, layer: u32, cursor: Cursor) {
        self.edit(|bound| bound.cursor(layer, cursor));
    }
}

impl Drop for Handle {
    fn drop(&mut self) {
        let mut state = state();
        if let Some(bound) = state.canvases.get_mut(&self.pid).filter(|bound| bound.serial == self.serial) {
            bound.closing = true;
            wake(&mut state);
        }
    }
}

fn wake(state: &mut State) {
    // SAFETY: a post to our own window.
    if !state.woken && state.window != 0 && unsafe { PostMessageA(state.window as HWND, WAKE, 0, 0) } != 0 {
        state.woken = true;
    }
}

unsafe extern "system" fn proc(window: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        CONNECT => connect(wparam as isize),
        CLOSE => closed(window, wparam as u32),
        WM_COPYDATA => {
            // SAFETY: WM_COPYDATA's lparam is a COPYDATASTRUCT valid for this call.
            let data = unsafe { &*(lparam as *const COPYDATASTRUCT) };
            if !data.lpData.is_null() && data.cbData as usize <= wire::MAX_FRAME {
                // SAFETY: the sender's buffer of `cbData` bytes, valid for this call.
                let frame = unsafe { std::slice::from_raw_parts(data.lpData as *const u8, data.cbData as usize) };
                received(data.dwData as u32, frame);
            }
            return 1;
        }
        WAKE => {
            state().woken = false;
            entitle(window);
            flush();
        }
        WM_TIMER => {
            // SAFETY: our own window's timer; its id is a game's pid.
            unsafe { KillTimer(window, wparam) };
            state().resend(wparam as u32);
            flush();
        }
        _ => {}
    }
    // SAFETY: ORIGINAL holds STATIC's window procedure, stored before the window could get a message of ours.
    unsafe { CallWindowProcA(transmute::<isize, WNDPROC>(ORIGINAL.load(Ordering::Relaxed)), window, msg, wparam, lparam) }
}

/// The window's title follows `dormant`; only this thread sets it, so the last word stands.
fn entitle(window: HWND) {
    let dormant = state().dormant;
    if TITLED_IDLE.swap(dormant, Ordering::Relaxed) != dormant {
        let title = if dormant { IDLE_TITLE } else { TITLE };
        // SAFETY: our own window, on its own thread, with a terminated title.
        unsafe { SetWindowTextA(window, title.as_ptr()) };
    }
}

/// Without a canvas left: unfindable first, then the goodbye, so the DLL retries until a host is back. Said the
/// other way round it dials in again at once, and a CONNECT lost to our exit leaves it connecting for good.
fn rest(window: HWND) {
    let Some(clients) = state().idle() else { return };
    entitle(window);
    for client in clients {
        say_goodbye(client);
    }
    log::debug!("overlay: no canvas left, the host rests");
}

fn say_goodbye(client: isize) {
    // SAFETY: a post to the DLL's window; our pid as tinyipc wants it.
    unsafe { PostMessageA(client as HWND, CLOSE, GetCurrentProcessId() as usize, 0) };
}

/// Every dial is answered at once: one left unanswered stays connecting for good, even to a later host. A resting
/// host sends it back to retrying; frames go only to a canvas `bind` made.
fn connect(client: isize) {
    if state().dormant {
        say_goodbye(client);
        return;
    }
    let mut pid = 0;
    // SAFETY: a query on a window handle; an invalid one gives 0.
    if unsafe { GetWindowThreadProcessId(client as HWND, &mut pid) } == 0 {
        return;
    }
    // A recorded game that ended gives its pid up: the link is then the new process's.
    retire_if_gone(pid);
    state().linked(pid, client);
    // SAFETY: a post to the window the DLL gave; our pid as tinyipc wants it.
    if unsafe { PostMessageA(client as HWND, CONNECT_ACK, GetCurrentProcessId() as usize, 0) } == 0 {
        // An elevated game's window takes it only if the DLL lifted UIPI's filter.
        log::warn!("overlay: the DLL in process {pid} cannot be answered: {}", std::io::Error::last_os_error());
    } else {
        log::debug!("overlay: the DLL in process {pid} linked");
    }
}

/// The DLL left (unloaded, or its game closes): the canvas there ends.
fn closed(window: HWND, pid: u32) {
    let mut state = state();
    if state.clients.remove(&pid).is_some() {
        state.canvases.remove(&pid);
        drop(state);
        log::debug!("overlay: the DLL in process {pid} closed the link");
        retire_if_gone(pid);
        rest(window);
    }
}

fn received(pid: u32, frame: &[u8]) {
    let Some((id, body)) = wire::unpack(frame) else { return };
    let mut state = state();
    let window = state.window;
    let Some(client) = state.clients.get_mut(&pid) else { return };
    let mut due = client.id.replace(id).is_none();
    let event = match wire::read(body) {
        Some(FromDll::Hooked { adapter }) => {
            client.adapter = Some(adapter);
            log::debug!("overlay: the DLL in process {pid} hooked the game on adapter {adapter}");
            due = true;
            None
        }
        Some(FromDll::Resized { width, height }) => {
            client.size = Some((width, height));
            later(window, pid);
            Some(CanvasEvent::Resized { width, height })
        }
        Some(FromDll::Input { window_id, msg, wparam, lparam }) => window_id.checked_sub(1).map(|layer| CanvasEvent::Input {
            layer,
            message: msg,
            wparam: wparam as usize,
            lparam: lparam as isize,
        }),
        Some(FromDll::Blocked { block }) => {
            log::debug!("overlay: the DLL in process {pid} blocks the game's input: {block}");
            None
        }
        None => None,
    };
    if let Some(event) = event {
        state.heard(pid, event);
    }
    if due {
        wake(&mut state);
    }
}

fn later(window: isize, pid: u32) {
    // SAFETY: our own window; the timer's id is the game's pid, never 0.
    unsafe { SetTimer(window as HWND, pid as usize, RESEND_MS, None) };
}

/// Sends what each game's canvas has waiting, outside the lock: a hung game must not hold a `Layers` call.
fn flush() {
    let (window, mut outbox) = {
        let mut state = state();
        let mut outbox = mem::take(&mut state.outbox);
        state.pending(&mut outbox);
        (state.window, outbox)
    };
    let mut gone = None;
    for (pid, client, frame) in outbox.drain(..) {
        if gone == Some(client) {
            continue;
        }
        match deliver(client, &frame) {
            Delivery::Done => {}
            Delivery::Late => {
                if let Some(link) = state().clients.get_mut(&pid).filter(|link| link.window == client) {
                    link.unsure = true;
                }
                later(window, pid);
            }
            Delivery::Gone => {
                gone = Some(client);
                let mut state = state();
                // Only this link: the pid may have a new process and link by now.
                if state.clients.get(&pid).is_some_and(|link| link.window == client) {
                    state.clients.remove(&pid);
                    state.canvases.remove(&pid);
                }
                drop(state);
                log::debug!("overlay: the DLL in process {pid} is gone");
                retire_if_gone(pid);
            }
        }
    }
    state().outbox = outbox;
    rest(window as HWND);
}

enum Delivery {
    Done,
    /// The game did not answer in time: the message may be lost.
    Late,
    Gone,
}

fn deliver(window: isize, frame: &[u8]) -> Delivery {
    // SAFETY: a query of our own pid.
    let ours = unsafe { GetCurrentProcessId() };
    let data = COPYDATASTRUCT { dwData: ours as usize, cbData: frame.len() as u32, lpData: frame.as_ptr() as *mut c_void };
    let mut answer = 0;
    // SAFETY: `data` and `frame` outlive the call; a hung game times out.
    let sent = unsafe {
        SendMessageTimeoutA(window as HWND, WM_COPYDATA, 0, &data as *const _ as LPARAM, SMTO_ABORTIFHUNG, SEND_WITHIN_MS, &mut answer)
    };
    match sent {
        // SAFETY: read right after the failed call, on this thread.
        0 if unsafe { GetLastError() } == ERROR_INVALID_WINDOW_HANDLE => Delivery::Gone,
        0 => Delivery::Late,
        _ => Delivery::Done,
    }
}

#[cfg(test)]
pub fn linked_to(pid: u32) -> bool {
    state().clients.contains_key(&pid)
}

/// Blocking. Lets go of every recorded game that ended, also one whose link and canvas closed before it did.
pub fn sweep() {
    let pids: Vec<u32> = state().mapped.keys().copied().collect();
    pids.into_iter().for_each(retire_if_gone);
}

/// Lets go of a game that ended: its DLL's image unlocks and its pid may come again as another process.
fn retire_if_gone(pid: u32) {
    let Some(process) = state().mapped.get(&pid).map(|(process, _)| process.clone()) else { return };
    // Outside the lock: it opens the process.
    if agent::of(pid).as_ref() == Some(&process) {
        return;
    }
    let mut state = state();
    if state.mapped.get(&pid).is_some_and(|(known, _)| *known == process) {
        state.mapped.remove(&pid);
        state.clients.remove(&pid);
        state.canvases.remove(&pid);
    }
}
