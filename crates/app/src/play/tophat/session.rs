//! A Tophat session as Wand drives it: load the trainer plugin, run the mod, set values, stop.
//! Timers run on the server's runtime; the async methods can be awaited from any executor.

use std::{
    collections::HashMap,
    future::Future,
    pin::pin,
    sync::{
        Arc, Mutex, MutexGuard,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::Duration,
};

use anyhow::{Result, anyhow, bail, ensure};
use futures::{
    StreamExt,
    channel::{
        mpsc::{self, Receiver, Sender, UnboundedReceiver, UnboundedSender},
        oneshot,
    },
    future::{self, Either},
    lock::Mutex as AsyncMutex,
};
use serde_json::{Value, json};
use tokio::{
    runtime::Handle,
    sync::watch,
    time::{Instant, MissedTickBehavior, interval_at},
};

use super::{
    proto::{self, Event, Incoming},
    server::Channel,
};
use crate::play::image::Image;

#[cfg(test)]
mod tests;

const PLUGIN: &str = "trainerlib";
// The service gets Wand's timings: a beat every 5 s, 120 s to answer the first, 60 s after.
const BEAT: Duration = Duration::from_secs(5);
const FIRST_BEAT_LIMIT: Duration = Duration::from_secs(120);
const BEAT_LIMIT: Duration = Duration::from_secs(60);
const LOAD_PLUGIN_WITHIN: Duration = Duration::from_secs(30);
const COMMAND_WITHIN: Duration = Duration::from_secs(5);
const READY_WITHIN: Duration = Duration::from_secs(60);
const STOP_WITHIN: Duration = Duration::from_secs(30);
const TEARDOWN_WITHIN: Duration = Duration::from_secs(5);
// The service answers a command at its own timeout, so the wait is a little longer.
const COMMAND_SLACK: Duration = Duration::from_secs(10);
// A consumer that falls behind loses events instead of growing memory.
const EVENT_BACKLOG: usize = 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Starting,
    Ready,
    Closed,
}

/// The trainer's verdict on a value we pushed.
#[derive(Debug, PartialEq, Eq)]
pub enum Outcome {
    Set,
    Changed,
    Failed,
}

pub struct Run<'a> {
    pub process_id: u32,
    pub game_version: u32,
    pub trainer: Image,
    pub variables: &'a [String],
}

pub struct Session {
    shared: Arc<Shared>,
    runtime: Handle,
    phase: watch::Receiver<Phase>,
    plugin: Mutex<Option<u64>>,
    started: AtomicBool,
    // The DLLs handed to the service stay locked for the session's life, so a cancelled or
    // timed-out `start` cannot unlock a file the service is still opening.
    held: Mutex<Vec<Image>>,
    // One teardown at a time: a second `stop` must not close the stream under the first.
    teardown: AsyncMutex<()>,
}

#[derive(Default)]
struct Waiting {
    replies: HashMap<u64, oneshot::Sender<Vec<u8>>>,
    verdicts: HashMap<u64, oneshot::Sender<u64>>,
}

/// What the session handle and its driver task both touch.
struct Shared {
    client_id: String,
    outgoing: Mutex<Option<UnboundedSender<Vec<u8>>>>,
    waiting: Mutex<Waiting>,
    phase: watch::Sender<Phase>,
    // Sequence numbers start at 1 and do not skip: they are only for requests.
    seq: AtomicU64,
    ids: AtomicU64,
}

impl Session {
    /// `client_id` is the one the service registered with. Events end when the session closes.
    pub fn open(runtime: Handle, channel: Channel, client_id: &str) -> (Self, Receiver<Event>) {
        let (phase, watching) = watch::channel(Phase::Starting);
        let shared = Arc::new(Shared {
            client_id: client_id.to_owned(),
            outgoing: Mutex::new(Some(channel.outgoing)),
            waiting: Mutex::default(),
            phase,
            seq: AtomicU64::new(0),
            ids: AtomicU64::new(0),
        });
        let (events_out, events) = mpsc::channel(EVENT_BACKLOG);
        let task_shared = shared.clone();
        let close = Forget(move || task_shared.close());
        runtime.spawn(drive(shared.clone(), channel.incoming, events_out, close));
        let session = Self {
            shared,
            runtime,
            phase: watching,
            plugin: Mutex::new(None),
            started: AtomicBool::new(false),
            held: Mutex::default(),
            teardown: AsyncMutex::new(()),
        };
        (session, events)
    }

    pub fn phase(&self) -> Phase {
        *self.phase.borrow()
    }

    /// Loads the trainer plugin, runs the mod and waits until the trainer reports ready. Once per
    /// session: a failure rolls the session back and closes it, so a retry needs a new one. To
    /// cancel, drop the session; the service tears down by itself when its stream closes.
    pub async fn start(&self, plugin: Image, run: Run<'_>) -> Result<()> {
        let plugin_path = plugin.path().to_str().ok_or_else(|| anyhow!("plugin path is not UTF-8"))?.to_owned();
        let trainer = run.trainer.path().to_str().ok_or_else(|| anyhow!("trainer path is not UTF-8"))?;
        let params = json!({
            "processId": run.process_id,
            "gameVersion": run.game_version,
            "trainerDllPath": trainer,
            "initialVars": run.variables,
        });
        ensure!(!self.started.swap(true, Ordering::Relaxed), "the session was already started");
        // Before the first await, so cancelling this future cannot drop them.
        self.held.lock().unwrap().extend([plugin, run.trainer]);
        let started = self.load_and_run(&plugin_path, params).await;
        if started.is_err() {
            if self.plugin.lock().unwrap().is_some() {
                let _ = self.stop().await;
            } else {
                // Nothing known to unload, so there is no teardown to ask for.
                self.shared.close();
            }
        }
        started
    }

    async fn load_and_run(&self, plugin_path: &str, params: Value) -> Result<()> {
        let reply = self
            .call("LoadPlugin", &proto::load_plugin(PLUGIN, plugin_path), LOAD_PLUGIN_WITHIN)
            .await?;
        *self.plugin.lock().unwrap() = Some(proto::plugin_handle(&reply).map_err(|err| anyhow!("LoadPlugin: {err}"))?);
        self.execute("trainer_run_mod_json", Some(params), COMMAND_WITHIN).await?;
        let mut phase = self.phase.clone();
        let Some(settled) = self.within(READY_WITHIN, phase.wait_for(|phase| *phase != Phase::Starting)).await
        else {
            bail!("the trainer was not ready in {READY_WITHIN:?}");
        };
        let settled = settled.map_err(|_| closed())?;
        ensure!(*settled == Phase::Ready, "the service left before the trainer was ready");
        Ok(())
    }

    pub async fn set_value(&self, name: &str, value: f64) -> Result<Outcome> {
        let id = self.shared.ids.fetch_add(1, Ordering::Relaxed) + 1;
        let (verdict, verdicts) = oneshot::channel();
        self.shared.waiting().verdicts.insert(id, verdict);
        let _forget = Forget(|| {
            self.shared.waiting().verdicts.remove(&id);
        });
        let params = json!({"requestId": id, "name": name, "value": value});
        self.execute("trainer_push_client_req_from_json", Some(params), COMMAND_WITHIN).await?;
        let code = self
            .within(COMMAND_WITHIN, verdicts)
            .await
            .ok_or_else(|| anyhow!("no verdict for {name} in {COMMAND_WITHIN:?}"))?;
        Ok(match code.map_err(|_| closed())? {
            0 => Outcome::Set,
            1 => Outcome::Changed,
            _ => Outcome::Failed,
        })
    }

    /// Wand's order: stop the mod, unload the plugin, say goodbye, then drop the stream. Every
    /// step runs even if an earlier one failed; the first error is returned. Cancelling it
    /// closes the session, so the service tears down by itself.
    pub async fn stop(&self) -> Result<()> {
        let _turn = self.teardown.lock().await;
        if self.phase() == Phase::Closed {
            return Ok(());
        }
        let _close = Forget(|| self.shared.close());
        let stopped = self.execute("trainer_stop_mod", None, STOP_WITHIN).await;
        let handle = self.plugin.lock().unwrap().take();
        let unloaded = match handle {
            Some(handle) => self
                .call("UnloadPlugin", &proto::unload_plugin(handle), TEARDOWN_WITHIN)
                .await
                .and_then(|reply| proto::checked(&reply).map(drop))
                .map_err(|err| anyhow!("UnloadPlugin: {err}")),
            None => Ok(()),
        };
        // The service's answer to this one is not needed.
        let _ = self.call("Disconnect", &proto::disconnect(), TEARDOWN_WITHIN).await;
        stopped.and(unloaded)
    }

    async fn execute(&self, command: &str, params: Option<Value>, timeout: Duration) -> Result<()> {
        let payload = proto::command(command, params.as_ref(), &self.shared.client_id, timeout.as_millis() as u32);
        let reply = self.call("ExecuteCommand", &payload, timeout + COMMAND_SLACK).await?;
        proto::checked(&reply).map_err(|err| anyhow!("{command}: {err}"))?;
        Ok(())
    }

    async fn call(&self, kind: &str, payload: &[u8], limit: Duration) -> Result<Vec<u8>> {
        let (seq, reply) = self.shared.ask(kind, payload)?;
        let _forget = Forget(|| {
            self.shared.waiting().replies.remove(&seq);
        });
        let Some(reply) = self.within(limit, reply).await else {
            bail!("{kind} got no reply in {limit:?}");
        };
        reply.map_err(|_| closed())
    }

    /// `None` when `limit` passed first.
    async fn within<T>(&self, limit: Duration, work: impl Future<Output = T>) -> Option<T> {
        // Built inside the task: `sleep` panics on a caller with no runtime, like GPUI's threads.
        let timer = self.runtime.spawn(async move { tokio::time::sleep(limit).await });
        // Also stops the timer when the caller cancels this future instead of letting it run out.
        let abort = timer.abort_handle();
        let _abort = Forget(move || abort.abort());
        match future::select(pin!(work), timer).await {
            Either::Left((value, _)) => Some(value),
            Either::Right(_) => None,
        }
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        self.shared.close();
    }
}

fn closed() -> anyhow::Error {
    anyhow!("the Tophat session is closed")
}

/// Runs a cleanup on drop, so it also runs when the caller cancels or the runtime drops a task.
struct Forget<F: Fn()>(F);

impl<F: Fn()> Drop for Forget<F> {
    fn drop(&mut self) {
        (self.0)();
    }
}

impl Shared {
    fn waiting(&self) -> MutexGuard<'_, Waiting> {
        self.waiting.lock().unwrap()
    }

    fn next_seq(&self) -> u64 {
        self.seq.fetch_add(1, Ordering::Relaxed) + 1
    }

    fn send(&self, message: Vec<u8>) -> Result<()> {
        let outgoing = self.outgoing.lock().unwrap();
        let outgoing = outgoing.as_ref().ok_or_else(closed)?;
        outgoing.unbounded_send(message).map_err(|_| closed())
    }

    /// Sends a request; the receiver resolves with the reply payload.
    fn ask(&self, kind: &str, payload: &[u8]) -> Result<(u64, oneshot::Receiver<Vec<u8>>)> {
        let seq = self.next_seq();
        let (reply, replies) = oneshot::channel();
        self.waiting().replies.insert(seq, reply);
        if let Err(err) = self.send(proto::request(kind, payload, seq)) {
            self.waiting().replies.remove(&seq);
            return Err(err);
        }
        Ok((seq, replies))
    }

    /// Ends our side of the stream and fails everything still waiting.
    fn close(&self) {
        drop(self.outgoing.lock().unwrap().take());
        *self.waiting() = Waiting::default();
        self.phase.send_replace(Phase::Closed);
    }

    fn dispatch(&self, raw: &[u8], events: &mut Sender<Event>, pulse: &mut Pulse) {
        match proto::incoming(raw) {
            Some(Incoming::Reply { seq, payload }) => {
                if pulse.answered(seq) {
                    return;
                }
                if let Some(reply) = self.waiting().replies.remove(&seq) {
                    let _ = reply.send(payload.to_vec());
                }
            }
            Some(Incoming::Applied { id, code }) => {
                if let Some(verdict) = self.waiting().verdicts.remove(&id) {
                    let _ = verdict.send(code);
                }
            }
            Some(Incoming::Event(event)) => {
                if event == Event::Ready {
                    self.phase.send_if_modified(|phase| {
                        let starting = *phase == Phase::Starting;
                        if starting {
                            *phase = Phase::Ready;
                        }
                        starting
                    });
                }
                let _ = events.try_send(event);
            }
            Some(Incoming::Unknown(kind)) => log::debug!("tophat: unhandled message {kind}"),
            None => log::debug!("tophat: malformed message"),
        }
    }

    /// Sends the next heartbeat; false once the service has left one unanswered too long.
    fn beat(&self, pulse: &mut Pulse) -> bool {
        if let Some((_, sent)) = pulse.outstanding {
            let limit = if pulse.count == 1 { FIRST_BEAT_LIMIT } else { BEAT_LIMIT };
            return sent.elapsed() < limit;
        }
        pulse.count += 1;
        let seq = self.next_seq();
        pulse.outstanding = Some((seq, Instant::now()));
        self.send(proto::request("Heartbeat", &proto::heartbeat(pulse.count), seq)).is_ok()
    }
}

#[derive(Default)]
struct Pulse {
    count: u32,
    outstanding: Option<(u64, Instant)>,
}

impl Pulse {
    fn answered(&mut self, seq: u64) -> bool {
        let answered = self.outstanding.is_some_and(|(sent, _)| sent == seq);
        if answered {
            self.outstanding = None;
        }
        answered
    }
}

/// Reads the service's messages and keeps the heartbeat until the stream ends.
async fn drive(
    shared: Arc<Shared>,
    mut incoming: UnboundedReceiver<Vec<u8>>,
    mut events: Sender<Event>,
    // An argument, so it also closes the session when a shutting-down runtime drops the task unpolled.
    _close: Forget<impl Fn()>,
) {
    let mut beats = interval_at(Instant::now() + BEAT, BEAT);
    beats.set_missed_tick_behavior(MissedTickBehavior::Delay);
    let mut pulse = Pulse::default();
    loop {
        tokio::select! {
            raw = incoming.next() => match raw {
                Some(raw) => shared.dispatch(&raw, &mut events, &mut pulse),
                None => break,
            },
            _ = beats.tick() => if !shared.beat(&mut pulse) {
                log::warn!("tophat: the service stopped answering heartbeats");
                break;
            },
        }
    }
}

