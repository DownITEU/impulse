//! What the host knows of each game's DLL and canvas, and what it must send there next. No sending here: the host
//! thread sends outside the lock.

use std::{collections::HashMap, ptr, sync::Arc};

use agent::Image;
use futures::channel::mpsc::UnboundedSender;
use model::{CanvasEvent, Cursor, Process, Rect, Texture};
use windows_sys::Win32::{
    Foundation::{CloseHandle, INVALID_HANDLE_VALUE},
    System::{
        Memory::{CreateFileMappingA, PAGE_READWRITE},
        Threading::GetCurrentProcessId,
    },
};

use super::wire::{self, ToDll};

/// What Wand sizes a window's bitmap memory to; the DLL opens it even for texture frames.
const MEMORY: u32 = 3844 * 2164 * 4 + 4096;

#[derive(Default)]
pub struct State {
    pub window: isize,
    /// Without a canvas the window goes by another title: a DLL that finds no host retries, and none can be left
    /// connecting to us when we exit.
    pub dormant: bool,
    pub woken: bool,
    pub serials: u64,
    /// The processes the DLL was loaded into, by pid.
    pub mapped: HashMap<u32, (Process, Load)>,
    pub clients: HashMap<u32, Client>,
    /// The canvas drawing in each game, by pid.
    pub canvases: HashMap<u32, Bound>,
    /// Kept between flushes for its capacity: a frame allocates nothing.
    pub outbox: Vec<Sending>,
}

/// A frame for the DLL in a game: its pid, its window, the frame.
pub type Sending = (u32, isize, Out);

pub enum Load {
    Running,
    /// Its image stays locked while the process is recorded.
    Kept {
        _lock: Image,
    },
    /// Not tried again: a load that timed out may still land, and a second one would not start the DLL again.
    Failed,
}

pub struct Client {
    pub window: isize,
    /// tinyipc's id of the DLL's node, learned from its first frame.
    pub id: Option<i32>,
    pub adapter: Option<u32>,
    /// The game's frame it last reported: a canvas bound later learns it on `Ready`.
    pub size: Option<(u32, u32)>,
    pub configured: bool,
    /// A send timed out: what the DLL holds is unknown, so everything goes again.
    pub unsure: bool,
    /// The words of a canvas that closed, said again while sends are late.
    left: Vec<Out>,
}

impl Client {
    pub fn new(window: isize) -> Self {
        Self { window, id: None, adapter: None, size: None, configured: false, unsure: false, left: Vec::new() }
    }
}

pub struct Bound {
    pub serial: u64,
    pub events: UnboundedSender<CanvasEvent>,
    pub ready: bool,
    pub closing: bool,
    pub layers: HashMap<u32, Slot>,
    pub block: bool,
    /// The global block the DLL was last told of; `None` before the first word.
    told: Option<bool>,
}

impl Bound {
    pub fn want(&mut self, layer: u32, want: Option<(Rect, Texture)>) {
        let slot = self.layers.entry(layer).or_default();
        // Each show is a new frame: the texture's handle stays.
        slot.dirty |= want.is_some();
        slot.want = want;
    }

    pub fn interact(&mut self, layer: u32, on: bool) {
        self.layers.entry(layer).or_default().input = on;
    }

    pub fn cursor(&mut self, layer: u32, cursor: Cursor) {
        self.layers.entry(layer).or_default().cursor = cursor;
    }

    fn tell_block(&mut self, closing: bool, unsure: bool) -> Option<ToDll<'static>> {
        // Only with an interactive layer up: the game must not lose its input to nothing shown.
        let over = self.layers.values().any(|slot| slot.shown.as_ref().is_some_and(|shown| shown.input));
        let block = self.block && over && !closing;
        let due = if unsure { self.told.is_some() || block } else { self.told.unwrap_or(false) != block };
        due.then(|| {
            self.told = Some(block);
            ToDll::GlobalBlock { block }
        })
    }
}

/// A layer as asked and as the DLL has it. Kept once hidden: a hide whose send timed out goes again.
#[derive(Default)]
pub struct Slot {
    want: Option<(Rect, Texture)>,
    input: bool,
    cursor: Cursor,
    /// A frame the DLL has not been told of.
    dirty: bool,
    shown: Option<Shown>,
}

struct Shown {
    rect: Rect,
    input: bool,
    /// A window described anew has the arrow.
    cursor: Cursor,
    texture: Texture,
    /// The frame update for `texture`, made once: a frame of the same texture says the same.
    update: Arc<[u8]>,
    // Open while the DLL may read it.
    _memory: Memory,
}

/// The named memory the DLL opens for a window.
struct Memory {
    handle: isize,
    name: String,
}

impl Drop for Memory {
    fn drop(&mut self) {
        // SAFETY: our own mapping handle, closed once.
        unsafe { CloseHandle(self.handle as _) };
    }
}

/// A tinyipc frame to send.
pub type Out = Arc<[u8]>;

impl State {
    /// A process the DLL goes into; another of the same pid (the old game exited) takes its link and canvas with it.
    pub fn record(&mut self, process: &Process, load: Load) {
        if self.mapped.get(&process.pid).is_some_and(|(known, _)| known != process) {
            self.clients.remove(&process.pid);
            self.canvases.remove(&process.pid);
        }
        self.mapped.insert(process.pid, (process.clone(), load));
    }

    /// A new canvas for `pid` in place of the one there, whose events end; the old one's layers close unless shown again.
    pub fn bind(&mut self, pid: u32, events: UnboundedSender<CanvasEvent>) -> u64 {
        self.serials += 1;
        let (mut layers, told) = self.canvases.remove(&pid).map(|old| (old.layers, old.told)).unwrap_or_default();
        layers.values_mut().for_each(|slot| *slot = Slot { shown: slot.shown.take(), ..Slot::default() });
        let bound = Bound { serial: self.serials, events, ready: false, closing: false, layers, block: false, told };
        self.canvases.insert(pid, bound);
        self.serials
    }

    /// The DLL in `pid` linked again: it holds no window or block of ours.
    pub fn linked(&mut self, pid: u32, window: isize) {
        self.clients.insert(pid, Client::new(window));
        if let Some(bound) = self.canvases.get_mut(&pid) {
            bound.ready = false;
            bound.told = None;
            bound.layers.values_mut().for_each(|slot| slot.shown = None);
        }
    }

    /// Every frame goes again (the DLL dropped them on a resize).
    pub fn resend(&mut self, pid: u32) {
        if let Some(bound) = self.canvases.get_mut(&pid) {
            bound.layers.values_mut().filter(|slot| slot.shown.is_some()).for_each(|slot| slot.dirty = true);
        }
    }

    /// What goes to each DLL now, in order, into `out`; marked as sent. A closing canvas ends here, its layers closed
    /// (one whose game never linked had nothing shown); the link stays until the host rests.
    pub fn pending(&mut self, out: &mut Vec<Sending>) {
        for (&pid, client) in &mut self.clients {
            let window = client.window;
            let unsure = std::mem::take(&mut client.unsure);
            let left = std::mem::take(&mut client.left);
            if let Some(id) = client.id.filter(|_| !client.configured || unsure) {
                client.configured = true;
                out.push((pid, window, wire::pack(id, &ToDll::init().json()).into()));
            }
            // A late send may have lost a closed canvas's last words (the block would stay on): kept until a flush
            // goes through.
            if unsure {
                out.extend(left.iter().map(|frame| (pid, window, frame.clone())));
                client.left = left;
            }
            let Some(bound) = self.canvases.get_mut(&pid) else { continue };
            let start = out.len();
            let mut send = |frame: Out| out.push((pid, window, frame));
            let drawn = client.id.zip(client.adapter);
            let foreign = drawn
                .is_some_and(|(_, adapter)| bound.layers.values().any(|slot| slot.want.is_some_and(|(_, frame)| frame.adapter != adapter)));
            if foreign {
                log::warn!("overlay: the game in process {pid} draws on another GPU than ours; the HUD stays over it");
            }
            let closing = bound.closing || foreign;
            if let Some((id, _)) = drawn {
                if !bound.ready && !closing {
                    bound.ready = true;
                    let _ = bound.events.unbounded_send(CanvasEvent::Ready);
                    if let Some((width, height)) = client.size {
                        let _ = bound.events.unbounded_send(CanvasEvent::Resized { width, height });
                    }
                }
                for (&layer, slot) in &mut bound.layers {
                    let Some(window_id) = layer.checked_add(1) else { continue };
                    slot.next(pid, (id, window_id), closing, unsure, &mut send);
                }
                if let Some(block) = bound.tell_block(closing, unsure) {
                    send(wire::pack(id, &block.json()).into());
                }
            }
            if closing {
                client.left.extend(out[start..].iter().map(|(.., frame)| frame.clone()));
                self.canvases.remove(&pid);
            }
        }
        let clients = &self.clients;
        self.canvases.retain(|pid, bound| !bound.closing || clients.contains_key(pid));
    }

    /// What the DLL in `pid` said that its canvas hears; nothing before `Ready`, which brings the known size.
    pub fn heard(&self, pid: u32, event: CanvasEvent) {
        if let Some(bound) = self.canvases.get(&pid).filter(|bound| bound.ready) {
            let _ = bound.events.unbounded_send(event);
        }
    }

    /// Goes dormant with no canvas and no link of a game we loaded the DLL into: that link lasts while its game runs, as
    /// the DLL rebuilds its graphics on every new link. The links it drops are the windows to say goodbye to.
    pub fn idle(&mut self) -> Option<Vec<isize>> {
        let ours = self.clients.keys().any(|pid| self.mapped.contains_key(pid));
        if self.dormant || self.window == 0 || !self.canvases.is_empty() || ours {
            return None;
        }
        self.dormant = true;
        Some(self.clients.drain().map(|(_, client)| client.window).collect())
    }
}

impl Slot {
    /// `ids`: the DLL's client id and this layer's window id there.
    fn next(&mut self, pid: u32, (client, window_id): (i32, u32), closing: bool, unsure: bool, send: &mut impl FnMut(Out)) {
        let pack = |message: ToDll| -> Out { wire::pack(client, &message.json()).into() };
        if unsure {
            self.shown = None;
        }
        let Some((rect, texture)) = self.want.filter(|_| !closing) else {
            if self.shown.take().is_some() || unsure {
                send(pack(ToDll::CloseOffscreenWindow { window_id }));
            }
            return;
        };
        let size = |texture: &Texture| (texture.width, texture.height);
        let input = self.input;
        if self.shown.as_ref().is_none_or(|shown| shown.rect != rect || shown.input != input || size(&shown.texture) != size(&texture)) {
            let Some(memory) = memory(pid, window_id) else { return };
            send(pack(ToDll::window(window_id, &memory.name, rect, input)));
            let update = pack(ToDll::frame(window_id, texture));
            self.shown = Some(Shown { rect, input, cursor: Cursor::Arrow, texture, update, _memory: memory });
            self.dirty = true;
        }
        let Some(shown) = self.shown.as_mut() else { return };
        if shown.texture != texture {
            shown.texture = texture;
            shown.update = pack(ToDll::frame(window_id, texture));
        }
        if std::mem::take(&mut self.dirty) {
            send(shown.update.clone());
        }
        if shown.cursor != self.cursor {
            shown.cursor = self.cursor;
            send(pack(ToDll::cursor(window_id, self.cursor)));
        }
    }
}

fn memory(pid: u32, window_id: u32) -> Option<Memory> {
    // SAFETY: a query of our own pid.
    let ours = unsafe { GetCurrentProcessId() };
    let name = format!("o-{ours:08x}-win-{pid}-{window_id}");
    let terminated = format!("{name}\0");
    // SAFETY: a pagefile-backed mapping named by a terminated string that outlives the call.
    let handle = unsafe { CreateFileMappingA(INVALID_HANDLE_VALUE, ptr::null(), PAGE_READWRITE, 0, MEMORY, terminated.as_ptr()) };
    if handle.is_null() {
        log::warn!("overlay: no window memory: {}", std::io::Error::last_os_error());
        return None;
    }
    Some(Memory { handle: handle as isize, name })
}

#[cfg(test)]
mod tests;
