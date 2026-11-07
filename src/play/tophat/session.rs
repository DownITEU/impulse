//! A Tophat session as Wand drives it: load the trainer plugin, run the mod, set values, stop.
//! Timers run on the server's runtime; the async methods can be awaited from any executor.

use std::{
    collections::HashMap,
    future::Future,
    path::Path,
    pin::pin,
    sync::{
        Arc, Mutex, MutexGuard,
        atomic::{AtomicU64, Ordering},
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
    pub trainer: &'a Path,
    pub variables: &'a [String],
}

pub struct Session {
    shared: Arc<Shared>,
    runtime: Handle,
    phase: watch::Receiver<Phase>,
    plugin: Mutex<Option<u64>>,
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
        };
        (session, events)
    }

    pub fn phase(&self) -> Phase {
        *self.phase.borrow()
    }

    /// Loads the trainer plugin, runs the mod and waits until the trainer reports ready.
    pub async fn start(&self, plugin: &Path, run: &Run<'_>) -> Result<()> {
        let path = plugin.to_str().ok_or_else(|| anyhow!("plugin path is not UTF-8"))?;
        let trainer = run.trainer.to_str().ok_or_else(|| anyhow!("trainer path is not UTF-8"))?;
        let reply = self
            .call("LoadPlugin", &proto::load_plugin(PLUGIN, path), LOAD_PLUGIN_WITHIN)
            .await?;
        *self.plugin.lock().unwrap() = Some(proto::plugin_handle(&reply).map_err(|err| anyhow!("LoadPlugin: {err}"))?);
        let params = json!({
            "processId": run.process_id,
            "gameVersion": run.game_version,
            "trainerDllPath": trainer,
            "initialVars": run.variables,
        });
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
    /// step runs even if an earlier one failed; the first error is returned.
    pub async fn stop(&self) -> Result<()> {
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
        self.shared.close();
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
        match future::select(pin!(work), timer).await {
            Either::Left((value, timer)) => {
                timer.abort();
                Some(value)
            }
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

#[cfg(test)]
mod tests {
    use futures::{channel::mpsc::unbounded, executor::block_on};

    use super::{
        super::{
            Server,
            proto::from_service,
            wire::{Field, Table, table},
        },
        *,
    };

    // Generous: the paused-clock tests wait through a heartbeat interval.
    async fn soon<T>(work: impl Future<Output = T>) -> T {
        tokio::time::timeout(Duration::from_secs(30), work).await.expect("timed out")
    }

    /// The service's end of a session.
    struct Fake {
        to_session: UnboundedSender<Vec<u8>>,
        from_session: UnboundedReceiver<Vec<u8>>,
    }

    struct Request {
        kind: String,
        seq: u64,
        payload: Vec<u8>,
    }

    fn link() -> (Channel, Fake) {
        let (to_session, incoming) = unbounded();
        let (outgoing, from_session) = unbounded();
        (Channel { incoming, outgoing }, Fake { to_session, from_session })
    }

    impl Fake {
        async fn request(&mut self) -> Request {
            let raw = soon(self.from_session.next()).await.expect("session closed");
            let message = Table::root(&raw).unwrap();
            assert_eq!((message.text(4), message.bool(5)), ("server".to_owned(), true));
            Request {
                kind: message.text(0),
                seq: message.u64(2),
                payload: message.bytes(1).to_vec(),
            }
        }

        /// A command request: its name and JSON parameters.
        async fn command(&mut self) -> (u64, String, Value) {
            let request = self.request().await;
            assert_eq!(request.kind, "ExecuteCommand");
            let command = Table::root(&request.payload).unwrap();
            let json = command.bytes(1);
            let params = match json.split_last() {
                Some((0, json)) => serde_json::from_slice(json).unwrap(),
                _ => Value::Null,
            };
            (request.seq, command.text(0), params)
        }

        fn reply(&self, seq: u64, payload: Vec<u8>) {
            self.to_session.unbounded_send(from_service("command_response", &payload, seq)).unwrap();
        }

        fn ok(&self, seq: u64) {
            self.reply(seq, table(&[(0, Field::Bool(true)), (6, Field::U8(0))]));
        }

        fn event(&self, kind: &str, payload: &[u8]) {
            self.to_session.unbounded_send(from_service(kind, payload, 0)).unwrap();
        }
    }

    const PLUGIN_DLL: &str = r"C:\wand\Plugin.dll";
    const TRAINER_DLL: &str = r"C:\t\Trainer_1.dll";

    async fn started() -> (Session, Fake, Receiver<Event>) {
        let (channel, mut fake) = link();
        let (session, events) = Session::open(Handle::current(), channel, "cid");
        let variables = ["hp".to_owned()];
        let run = Run { process_id: 1, game_version: 2, trainer: Path::new(TRAINER_DLL), variables: &variables };
        let service = async {
            let load = fake.request().await;
            fake.reply(load.seq, table(&[(0, Field::Bool(true)), (2, Field::U64(7))]));
            let (seq, ..) = fake.command().await;
            fake.ok(seq);
            fake.event("trainer_ready", &[]);
        };
        let (started, ()) = tokio::join!(session.start(Path::new(PLUGIN_DLL), &run), service);
        started.unwrap();
        (session, fake, events)
    }

    #[tokio::test]
    async fn start_loads_the_plugin_runs_the_mod_and_waits_for_ready() {
        let (channel, mut fake) = link();
        let (session, _events) = Session::open(Handle::current(), channel, "cid");
        let variables = ["hp".to_owned(), "mp".to_owned()];
        let run = Run { process_id: 4242, game_version: 7, trainer: Path::new(TRAINER_DLL), variables: &variables };
        let service = async {
            let load = fake.request().await;
            assert_eq!(load.kind, "LoadPlugin");
            let load_args = Table::root(&load.payload).unwrap();
            assert_eq!((load_args.text(0), load_args.text(1)), ("trainerlib".to_owned(), PLUGIN_DLL.to_owned()));
            fake.reply(load.seq, table(&[(0, Field::Bool(true)), (2, Field::U64(7))]));

            let (seq, name, params) = fake.command().await;
            assert_eq!(name, "trainer_run_mod_json");
            let expected = json!({"processId": 4242, "gameVersion": 7, "trainerDllPath": TRAINER_DLL, "initialVars": ["hp", "mp"]});
            assert_eq!(params, expected);
            fake.ok(seq);
            fake.event("trainer_ready", &[]);
        };
        let (started, ()) = tokio::join!(session.start(Path::new(PLUGIN_DLL), &run), service);
        started.unwrap();
        assert_eq!(session.phase(), Phase::Ready);
    }

    #[tokio::test]
    async fn a_refused_plugin_reports_the_services_error() {
        let (channel, mut fake) = link();
        let (session, _events) = Session::open(Handle::current(), channel, "cid");
        let run = Run { process_id: 1, game_version: 2, trainer: Path::new(TRAINER_DLL), variables: &[] };
        let service = async {
            let load = fake.request().await;
            fake.reply(load.seq, table(&[(0, Field::Bool(false)), (1, Field::Str("no such plugin"))]));
        };
        let (started, ()) = tokio::join!(session.start(Path::new(PLUGIN_DLL), &run), service);
        assert!(started.unwrap_err().to_string().contains("no such plugin"));
    }

    #[tokio::test]
    async fn set_value_returns_the_trainers_verdict() {
        let (session, mut fake, _events) = started().await;
        let service = async {
            let (seq, name, params) = fake.command().await;
            assert_eq!(name, "trainer_push_client_req_from_json");
            assert_eq!((&params["name"], &params["value"]), (&json!("hp"), &json!(1.0)));
            fake.ok(seq);
            let mut verdict = vec![0u8; 24];
            verdict[8..16].copy_from_slice(&params["requestId"].as_u64().unwrap().to_le_bytes());
            verdict[16..].copy_from_slice(&1u64.to_le_bytes());
            fake.event("trainer_value_response", &verdict);
        };
        let (outcome, ()) = tokio::join!(session.set_value("hp", 1.0), service);
        assert_eq!(outcome.unwrap(), Outcome::Changed);
    }

    #[tokio::test]
    async fn the_services_events_reach_the_consumer() {
        let (channel, fake) = link();
        let (_session, mut events) = Session::open(Handle::current(), channel, "cid");
        fake.event("trainer_status", b"loading\0");
        fake.event("trainer_initializing", &[]);
        fake.event("trainer_log", b"hello");
        fake.event("something_new", &[]);
        let mut seen = Vec::new();
        for _ in 0..3 {
            seen.push(soon(events.next()).await.unwrap());
        }
        assert_eq!(seen, [Event::Status("loading".into()), Event::Initializing, Event::Log("hello".into())]);
    }

    #[tokio::test]
    async fn a_vanishing_service_fails_pending_calls_and_ends_events() {
        let (channel, mut fake) = link();
        let (session, mut events) = Session::open(Handle::current(), channel, "cid");
        let service = async {
            fake.request().await;
            drop(fake);
        };
        let (outcome, ()) = tokio::join!(session.set_value("hp", 1.0), service);
        assert!(outcome.unwrap_err().to_string().contains("closed"));
        assert!(soon(events.next()).await.is_none());
        assert_eq!(session.phase(), Phase::Closed);
        assert!(session.set_value("hp", 1.0).await.is_err());
    }

    #[tokio::test]
    async fn stop_follows_wands_order_and_closes_the_stream() {
        let (session, mut fake, _events) = started().await;
        let service = async {
            let (seq, name, _) = fake.command().await;
            assert_eq!(name, "trainer_stop_mod");
            fake.ok(seq);
            let unload = fake.request().await;
            assert_eq!(unload.kind, "UnloadPlugin");
            assert_eq!(Table::root(&unload.payload).unwrap().u64(0), 7);
            fake.reply(unload.seq, table(&[(0, Field::Bool(true))]));
            let goodbye = fake.request().await;
            assert_eq!(goodbye.kind, "Disconnect");
            fake.reply(goodbye.seq, table(&[]));
            assert!(soon(fake.from_session.next()).await.is_none());
        };
        let (stopped, ()) = tokio::join!(session.stop(), service);
        stopped.unwrap();
        assert_eq!(session.phase(), Phase::Closed);
    }

    #[tokio::test(start_paused = true)]
    async fn answered_heartbeats_keep_the_session_and_silence_ends_it() {
        let (channel, mut fake) = link();
        let (session, mut events) = Session::open(Handle::current(), channel, "cid");
        for _ in 0..2 {
            let beat = fake.request().await;
            assert_eq!(beat.kind, "Heartbeat");
            assert!(Table::root(&beat.payload).unwrap().u64(0) > 0);
            fake.reply(beat.seq, table(&[(0, Field::Bool(true))]));
        }
        assert_eq!(session.phase(), Phase::Starting);
        assert_eq!(fake.request().await.kind, "Heartbeat");
        // Unanswered: the next ticks pass the 60 s limit and the session closes.
        assert!(events.next().await.is_none());
        assert_eq!(session.phase(), Phase::Closed);
    }

    #[tokio::test]
    async fn a_cancelled_call_leaves_no_waiter_behind() {
        let (channel, mut fake) = link();
        let (session, _events) = Session::open(Handle::current(), channel, "cid");
        {
            let mut set = pin!(session.set_value("hp", 1.0));
            tokio::select! {
                _ = &mut set => panic!("finished without a reply"),
                _ = fake.command() => {}
            }
            assert!(!session.shared.waiting().replies.is_empty());
        }
        let waiting = session.shared.waiting();
        assert!(waiting.replies.is_empty() && waiting.verdicts.is_empty());
    }

    #[tokio::test]
    async fn a_consumer_that_falls_behind_loses_events_instead_of_growing_memory() {
        let (channel, fake) = link();
        let (session, mut events) = Session::open(Handle::current(), channel, "cid");
        for _ in 0..EVENT_BACKLOG * 2 {
            fake.event("trainer_log", b"noise");
        }
        fake.event("trainer_ready", &[]);
        // Ready comes last, so everything before it has been dispatched.
        soon(async {
            while session.phase() != Phase::Ready {
                tokio::task::yield_now().await;
            }
        })
        .await;
        let mut kept = 0;
        while events.try_recv().is_ok() {
            kept += 1;
        }
        assert!(kept > 0 && kept <= EVENT_BACKLOG + 1, "kept {kept}");
    }

    // The runtime is the server's, so calls come from threads (GPUI's) that have none.
    #[test]
    fn calls_work_from_a_thread_without_a_runtime() {
        let server = Server::start().unwrap();
        let (channel, mut fake) = link();
        let (session, _events) = Session::open(server.runtime.clone(), channel, "cid");
        let runtime = server.runtime.clone();
        let service = std::thread::spawn(move || {
            let _enter = runtime.enter();
            let (seq, _, params) = block_on(fake.command());
            fake.ok(seq);
            let mut verdict = vec![0u8; 24];
            verdict[8..16].copy_from_slice(&params["requestId"].as_u64().unwrap().to_le_bytes());
            fake.event("trainer_value_response", &verdict);
        });
        assert_eq!(block_on(session.set_value("hp", 1.0)).unwrap(), Outcome::Set);
        service.join().unwrap();
    }

    #[test]
    fn dropping_the_server_closes_its_sessions() {
        let server = Server::start().unwrap();
        let (channel, _fake) = link();
        let (session, _events) = Session::open(server.runtime.clone(), channel, "cid");
        assert_eq!(session.phase(), Phase::Starting);
        drop(server);
        for _ in 0..100 {
            if session.phase() == Phase::Closed {
                return;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        panic!("the session still reports {:?}", session.phase());
    }

    #[tokio::test(start_paused = true)]
    async fn the_first_heartbeat_has_longer_to_be_answered() {
        let (channel, mut fake) = link();
        let (session, mut events) = Session::open(Handle::current(), channel, "cid");
        assert_eq!(fake.request().await.kind, "Heartbeat");
        tokio::time::sleep(Duration::from_secs(100)).await;
        assert_eq!(session.phase(), Phase::Starting);
        assert!(events.next().await.is_none());
    }
}
