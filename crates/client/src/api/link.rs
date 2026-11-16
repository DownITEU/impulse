//! Between a server and the app: what the server asks (`Ask`), and one board of the views the app publishes,
//! newest only, so a slow device skips to the latest. The app writes it (`Views`), the server reads it (`Feed`).

use std::{
    collections::VecDeque,
    sync::{Arc, Mutex, MutexGuard},
};

use futures::{
    SinkExt, Stream,
    channel::{mpsc, oneshot},
};

use super::{Command, View};

/// Asks waiting for the app; a full queue holds the asking socket back.
const ASKS: usize = 64;
/// Notes kept until their device reads them.
const NOTES: usize = 16;
/// Kept views, in the order a device needs them: values name the sheet the trainer view brought.
const KINDS: usize = 5;

pub enum Ask {
    /// A device trades a pairing code for its token.
    Pair {
        code: String,
        user_agent: String,
        reply: oneshot::Sender<Option<String>>,
    },
    /// The device a token belongs to.
    Device {
        token: String,
        reply: oneshot::Sender<Option<u32>>,
    },
    Command {
        device: u32,
        command: Command,
    },
    /// A game's cover, from the cache only.
    Art {
        game: u32,
        reply: oneshot::Sender<Option<Vec<u8>>>,
    },
}

pub struct ServerSide {
    asks: mpsc::Sender<Ask>,
    feed: Feed,
}

impl ServerSide {
    /// Waits for room in the app's queue; `false`: the app is gone.
    pub async fn ask(&self, ask: Ask) -> bool {
        self.asks.clone().send(ask).await.is_ok()
    }

    pub fn feed(&self) -> &Feed {
        &self.feed
    }
}

/// The server's side, and the app's: the views it publishes and the asks it answers.
pub fn link() -> (ServerSide, Views, impl Stream<Item = Ask> + Unpin) {
    let (sender, receiver) = mpsc::channel(ASKS);
    let board = Arc::new(Mutex::new(Board::default()));
    (ServerSide { asks: sender, feed: Feed(board.clone()) }, Views(board), receiver)
}

/// The app's hand on the board.
#[derive(Clone)]
pub struct Views(Arc<Mutex<Board>>);

/// The server's.
pub struct Feed(Arc<Mutex<Board>>);

#[derive(Default)]
struct Board {
    latest: [Option<Arc<str>>; KINDS],
    versions: [u64; KINDS],
    // By number, for one device each.
    notes: VecDeque<(u64, u32, Arc<str>)>,
    noted: u64,
    // Revoked devices: their sockets close.
    closed: Vec<u32>,
    // One per socket; a full one already has a wake waiting.
    wakes: Vec<mpsc::Sender<()>>,
}

/// What a socket has sent.
pub struct Seen {
    versions: [u64; KINDS],
    note: u64,
}

impl Views {
    /// Keeps `view` as the newest of its kind (a note goes to its device only) and wakes every socket.
    pub fn publish(&self, view: &View) {
        let Ok(json) = serde_json::to_string(view) else { return };
        let kind = match view {
            View::Hello { .. } => 0,
            View::Library { .. } => 1,
            View::Trainer { .. } => 2,
            View::Values { .. } => 3,
            View::Run { .. } => 4,
            View::Note { .. } => return log::warn!("a note is for a device: Views::note"),
        };
        let mut board = lock(&self.0);
        board.latest[kind] = Some(json.into());
        board.versions[kind] += 1;
        board.wake();
    }

    pub fn note(&self, device: u32, text: &str) {
        let Ok(json) = serde_json::to_string(&View::Note { text: text.to_owned() }) else { return };
        let mut board = lock(&self.0);
        board.noted += 1;
        let number = board.noted;
        board.notes.push_back((number, device, json.into()));
        if board.notes.len() > NOTES {
            board.notes.pop_front();
        }
        board.wake();
    }

    /// The device's sockets close, now and at their next connection's first look.
    pub fn close(&self, device: u32) {
        let mut board = lock(&self.0);
        board.closed.push(device);
        board.wake();
    }
}

impl Feed {
    /// A new socket's wake on every change, and what it has seen: every view is new to it, no older note is.
    pub fn watch(&self) -> (mpsc::Receiver<()>, Seen) {
        let (sender, receiver) = mpsc::channel(0);
        let mut board = lock(&self.0);
        board.wakes.retain(|wake| !wake.is_closed());
        board.wakes.push(sender);
        (receiver, Seen { versions: [0; KINDS], note: board.noted })
    }

    /// What `device` has not been sent, in order; `None`: it was revoked.
    pub fn since(&self, device: u32, seen: &mut Seen) -> Option<Vec<Arc<str>>> {
        let board = lock(&self.0);
        if board.closed.contains(&device) {
            return None;
        }
        let mut out = Vec::new();
        for kind in 0..KINDS {
            if seen.versions[kind] != board.versions[kind] {
                seen.versions[kind] = board.versions[kind];
                out.extend(board.latest[kind].clone());
            }
        }
        let notes = board.notes.iter().filter(|(number, to, _)| *number > seen.note && *to == device);
        out.extend(notes.map(|(_, _, json)| json.clone()));
        seen.note = board.noted;
        Some(out)
    }
}

fn lock(board: &Mutex<Board>) -> MutexGuard<'_, Board> {
    // A panic while holding it left plain data: still usable.
    board.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

impl Board {
    fn wake(&mut self) {
        // A closed socket's receiver is gone: its sender goes too.
        self.wakes.retain_mut(|wake| !matches!(wake.try_send(()), Err(e) if e.is_disconnected()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::{RunState, Sheet};

    #[test]
    fn a_device_gets_the_newest_of_each_view_and_its_own_new_notes() {
        let (server, views, _) = link();
        let run = |game| View::Run { state: RunState::Idle, game: Some(game), since: None };
        views.note(7, "before");
        let (_wake, mut seen) = server.feed().watch();
        views.publish(&run(1));
        views.publish(&View::Trainer { sheet: 1, game: None, trainer: Sheet::None });
        views.publish(&run(2));
        views.note(7, "refused");
        views.note(8, "other");
        let sent = server.feed().since(7, &mut seen).unwrap();
        assert_eq!(sent.len(), 3);
        assert!(sent[0].contains(r#""type":"trainer""#) && sent[1].contains(r#""game":2"#) && sent[2].contains("refused"));
        assert!(server.feed().since(7, &mut seen).unwrap().is_empty());
        views.close(7);
        assert!(server.feed().since(7, &mut seen).is_none());
    }

    #[test]
    fn a_wake_waits_once_however_many_changes() {
        let (server, views, _) = link();
        let (mut wake, _) = server.feed().watch();
        views.publish(&View::Library { games: Vec::new() });
        views.publish(&View::Library { games: Vec::new() });
        assert!(wake.try_recv().is_ok() && wake.try_recv().is_err());
        drop(wake);
        drop(server.feed().watch());
        assert_eq!(lock(&server.feed.0).wakes.len(), 1);
        views.publish(&View::Library { games: Vec::new() });
        assert!(lock(&server.feed.0).wakes.is_empty());
    }
}
