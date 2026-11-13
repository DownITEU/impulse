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
        Request { kind: message.text(0), seq: message.u64(2), payload: message.bytes(1).to_vec() }
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
    let (plugin, trainer) = (Image::unchecked(PLUGIN_DLL), Image::unchecked(TRAINER_DLL));
    let run = Run { process_id: 1, game_version: 2, trainer, variables: &variables };
    let service = async {
        let load = fake.request().await;
        fake.reply(load.seq, table(&[(0, Field::Bool(true)), (2, Field::U64(7))]));
        let (seq, ..) = fake.command().await;
        fake.ok(seq);
        fake.event("trainer_ready", &[]);
    };
    let (started, ()) = tokio::join!(session.start(plugin, run), service);
    started.unwrap();
    (session, fake, events)
}

#[tokio::test]
async fn start_loads_the_plugin_runs_the_mod_and_waits_for_ready() {
    let (channel, mut fake) = link();
    let (session, _events) = Session::open(Handle::current(), channel, "cid");
    let variables = ["hp".to_owned(), "mp".to_owned()];
    let (plugin, trainer) = (Image::unchecked(PLUGIN_DLL), Image::unchecked(TRAINER_DLL));
    let run = Run { process_id: 4242, game_version: 7, trainer, variables: &variables };
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
    let (started, ()) = tokio::join!(session.start(plugin, run), service);
    started.unwrap();
    assert_eq!(session.phase(), Phase::Ready);
}

#[tokio::test]
async fn a_refused_plugin_reports_the_services_error() {
    let (channel, mut fake) = link();
    let (session, _events) = Session::open(Handle::current(), channel, "cid");
    let (plugin, trainer) = (Image::unchecked(PLUGIN_DLL), Image::unchecked(TRAINER_DLL));
    let run = Run { process_id: 1, game_version: 2, trainer, variables: &[] };
    let service = async {
        let load = fake.request().await;
        fake.reply(load.seq, table(&[(0, Field::Bool(false)), (1, Field::Str("no such plugin"))]));
    };
    let (started, ()) = tokio::join!(session.start(plugin, run), service);
    assert!(started.unwrap_err().to_string().contains("no such plugin"));
    // Nothing was loaded, so the failed start just ends the session.
    assert_eq!(session.phase(), Phase::Closed);
}

#[tokio::test]
async fn a_failed_run_unloads_the_plugin_and_closes_the_session() {
    let (channel, mut fake) = link();
    let (session, _events) = Session::open(Handle::current(), channel, "cid");
    let (plugin, trainer) = (Image::unchecked(PLUGIN_DLL), Image::unchecked(TRAINER_DLL));
    let run = Run { process_id: 1, game_version: 2, trainer, variables: &[] };
    let service = async {
        let load = fake.request().await;
        fake.reply(load.seq, table(&[(0, Field::Bool(true)), (2, Field::U64(7))]));
        let (seq, name, _) = fake.command().await;
        assert_eq!(name, "trainer_run_mod_json");
        fake.reply(seq, table(&[(0, Field::Bool(false)), (1, Field::Str("trainer rejected"))]));
        // The rollback is the same teardown as `stop`.
        let (seq, name, _) = fake.command().await;
        assert_eq!(name, "trainer_stop_mod");
        fake.ok(seq);
        let unload = fake.request().await;
        assert_eq!((unload.kind.as_str(), Table::root(&unload.payload).unwrap().u64(0)), ("UnloadPlugin", 7));
        fake.reply(unload.seq, table(&[(0, Field::Bool(true))]));
        let goodbye = fake.request().await;
        fake.reply(goodbye.seq, table(&[]));
    };
    let (started, ()) = tokio::join!(session.start(plugin, run), service);
    assert!(started.unwrap_err().to_string().contains("trainer rejected"));
    assert_eq!(session.phase(), Phase::Closed);
}

#[tokio::test]
async fn a_session_starts_once() {
    let (session, _fake, _events) = started().await;
    let (plugin, trainer) = (Image::unchecked(PLUGIN_DLL), Image::unchecked(TRAINER_DLL));
    let run = Run { process_id: 1, game_version: 2, trainer, variables: &[] };
    let err = session.start(plugin, run).await.unwrap_err();
    assert!(err.to_string().contains("already started"), "{err}");
    // Refusing a second start must not disturb the running one.
    assert_eq!(session.phase(), Phase::Ready);
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

impl Fake {
    /// Takes one value from the session: the trainer pushes `own`, then gives `verdict`.
    async fn take_value(&mut self, own: &[(&str, f64)], verdict: u64) -> (String, f64) {
        let (seq, _, params) = self.command().await;
        self.ok(seq);
        for (name, value) in own {
            let mut pushed = vec![0u8; 56];
            pushed[16..16 + name.len()].copy_from_slice(name.as_bytes());
            pushed[48..].copy_from_slice(&value.to_le_bytes());
            self.event("trainer_value_request", &pushed);
        }
        let mut answer = vec![0u8; 24];
        answer[8..16].copy_from_slice(&params["requestId"].as_u64().unwrap().to_le_bytes());
        answer[16..].copy_from_slice(&verdict.to_le_bytes());
        self.event("trainer_value_response", &answer);
        (params["name"].as_str().unwrap().to_owned(), params["value"].as_f64().unwrap())
    }
}

#[tokio::test]
async fn position_is_what_the_trainer_pushes_for_this_request() {
    let (session, mut fake, _events) = started().await;
    let service = async {
        let own = [("wm_coord_x", 1.5), ("wm_coord_y", -2.), ("wm_coord_z", 3.), ("wm_coord_get", 1.)];
        assert_eq!(fake.take_value(&own, 1).await, ("wm_coord_get".to_owned(), -1.));
    };
    let (position, ()) = tokio::join!(session.position(), service);
    assert_eq!(position.unwrap(), [1.5, -2., 3.]);
    // The answer to the first request does not count for the second.
    let (position, _) = tokio::join!(session.position(), fake.take_value(&[], 1));
    assert!(position.unwrap_err().to_string().contains("wm_coord_get"));
}

#[tokio::test]
async fn teleport_sets_the_axes_then_asks_the_trainer_to_move() {
    let (session, mut fake, _events) = started().await;
    let service = async {
        let mut sent = Vec::new();
        for _ in 0..3 {
            sent.push(fake.take_value(&[], 0).await);
        }
        sent.push(fake.take_value(&[("wm_coord_set", 1.)], 1).await);
        sent
    };
    let (moved, sent) = tokio::join!(session.teleport([7., 8., 9.]), service);
    moved.unwrap();
    let sent: Vec<_> = sent.iter().map(|(name, value)| (name.as_str(), *value)).collect();
    assert_eq!(sent, [("wm_coord_x", 7.), ("wm_coord_y", 8.), ("wm_coord_z", 9.), ("wm_coord_set", -1.)]);
    // A refused axis ends it before the trainer is asked to move.
    let service = async {
        for verdict in [0, 2, 0] {
            fake.take_value(&[], verdict).await;
        }
    };
    let (moved, ()) = tokio::join!(session.teleport([7., 8., 9.]), service);
    assert!(moved.unwrap_err().to_string().contains("refused"));
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

#[tokio::test]
async fn a_second_stop_waits_for_the_first_instead_of_closing_under_it() {
    let (session, mut fake, _events) = started().await;
    let service = async {
        let (seq, ..) = fake.command().await;
        fake.ok(seq);
        let unload = fake.request().await;
        fake.reply(unload.seq, table(&[(0, Field::Bool(true))]));
        let goodbye = fake.request().await;
        fake.reply(goodbye.seq, table(&[]));
    };
    let (first, second, ()) = tokio::join!(session.stop(), session.stop(), service);
    assert!(first.is_ok() && second.is_ok(), "{first:?} {second:?}");
    assert_eq!(session.phase(), Phase::Closed);
}

#[tokio::test]
async fn cancelling_stop_closes_the_session() {
    let (session, mut fake, _events) = started().await;
    {
        let mut stop = pin!(session.stop());
        tokio::select! {
            _ = &mut stop => panic!("finished without a reply"),
            _ = fake.command() => {}
        }
        assert_eq!(session.phase(), Phase::Ready);
    }
    assert_eq!(session.phase(), Phase::Closed);
}

#[tokio::test]
async fn the_images_stay_held_even_if_start_is_cancelled() {
    let (channel, mut fake) = link();
    let (session, _events) = Session::open(Handle::current(), channel, "cid");
    let (plugin, trainer) = (Image::unchecked(PLUGIN_DLL), Image::unchecked(TRAINER_DLL));
    {
        let run = Run { process_id: 1, game_version: 2, trainer, variables: &[] };
        let mut start = pin!(session.start(plugin, run));
        tokio::select! {
            _ = &mut start => panic!("finished without a reply"),
            _ = fake.request() => {}
        }
    }
    assert_eq!(session.held.lock().unwrap().len(), 2);
}
