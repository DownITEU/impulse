//! The gRPC endpoint `tophat_service` dials into: raw flatbuffers messages in gRPC frames over h2.

use std::{
    collections::HashMap,
    future::poll_fn,
    net::{Ipv4Addr, TcpListener as StdListener},
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};

use anyhow::{Result, bail, ensure};
use bytes::{Buf, BufMut, Bytes, BytesMut};
use futures::{
    StreamExt,
    channel::{
        mpsc::{UnboundedReceiver, UnboundedSender, unbounded},
        oneshot,
    },
};
use h2::{
    RecvStream, SendStream,
    server::{self, SendResponse},
};
use http::{HeaderMap, Request, Response};
use tokio::{
    net::{TcpListener, TcpStream},
    runtime::Handle,
};

use super::wire::Table;

const SESSION: &str = "/tophat.TophatService/Session";
const SIDE_STREAMS: [&str; 2] = [
    "/tophat.TophatService/TelemetrySession",
    "/tophat.TophatService/EventsSession",
];
// A trainer's buffer messages are small; this only bounds what a local peer can make us hold.
const MAX_MESSAGE: usize = 16 << 20;
// Before `register` the peer is unauthenticated, so it gets a tiny budget: a few streams, a small
// first message and a deadline.
const REGISTER_MAX: usize = 4 << 10;
const REGISTER_WITHIN: Duration = Duration::from_secs(30);
const MAX_STREAMS: u32 = 8;

const OK: u16 = 0;
const CANCELLED: u16 = 1;
const INVALID_ARGUMENT: u16 = 3;
const PERMISSION_DENIED: u16 = 7;
const UNIMPLEMENTED: u16 = 12;

/// One service's message stream: whole flatbuffers messages each way.
/// Dropping `outgoing` ends the session; `incoming` ends when the service goes away.
pub struct Channel {
    pub incoming: UnboundedReceiver<Vec<u8>>,
    pub outgoing: UnboundedSender<Vec<u8>>,
}

type Waiters = Arc<Mutex<HashMap<String, oneshot::Sender<Channel>>>>;

/// Dropping the server stops it and drops every stream.
pub struct Server {
    pub port: u16,
    /// The runtime the server's tasks and sessions' timers run on.
    pub runtime: Handle,
    waiters: Waiters,
    _stop: oneshot::Sender<()>,
}

impl Server {
    pub fn start() -> Result<Self> {
        let listener = StdListener::bind((Ipv4Addr::LOCALHOST, 0))?;
        listener.set_nonblocking(true)?;
        let port = listener.local_addr()?.port();
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;
        let handle = runtime.handle().clone();
        let waiters = Waiters::default();
        let (stop, stopped) = oneshot::channel();
        let shared = waiters.clone();
        thread::Builder::new()
            .name("tophat-server".into())
            .spawn(move || {
                runtime.block_on(async {
                    match TcpListener::from_std(listener) {
                        Ok(listener) => accept(listener, shared, stopped).await,
                        Err(err) => log::error!("tophat server: {err}"),
                    }
                });
            })?;
        Ok(Self {
            port,
            runtime: handle,
            waiters,
            _stop: stop,
        })
    }

    /// Resolves with the stream whose first message registers as `client_id`. The id is
    /// random per launch, so it also keeps other local processes off the port.
    pub fn expect(&self, client_id: &str) -> oneshot::Receiver<Channel> {
        let (waiter, channel) = oneshot::channel();
        let mut waiters = self.waiters.lock().unwrap();
        waiters.retain(|_, waiter| !waiter.is_canceled());
        waiters.insert(client_id.to_owned(), waiter);
        channel
    }
}

async fn accept(listener: TcpListener, waiters: Waiters, mut stopped: oneshot::Receiver<()>) {
    loop {
        tokio::select! {
            _ = &mut stopped => return,
            accepted = listener.accept() => match accepted {
                Ok((socket, _)) => {
                    tokio::spawn(connection(socket, waiters.clone()));
                }
                Err(err) => {
                    log::warn!("tophat accept: {err}");
                    tokio::time::sleep(Duration::from_millis(50)).await;
                }
            },
        }
    }
}

async fn connection(socket: TcpStream, waiters: Waiters) {
    let handshake = server::Builder::new()
        .max_concurrent_streams(MAX_STREAMS)
        .handshake(socket);
    let mut conn = match handshake.await {
        Ok(conn) => conn,
        Err(err) => return log::debug!("tophat handshake: {err}"),
    };
    while let Some(accepted) = conn.accept().await {
        match accepted {
            Ok((request, respond)) => {
                tokio::spawn(route(request, respond, waiters.clone()));
            }
            Err(err) => return log::debug!("tophat connection: {err}"),
        }
    }
}

async fn route(request: Request<RecvStream>, respond: SendResponse<Bytes>, waiters: Waiters) {
    let path = request.uri().path().to_owned();
    let body = request.into_body();
    let result = match path.as_str() {
        SESSION => session(body, respond, &waiters).await,
        side if SIDE_STREAMS.contains(&side) => drain(body, respond).await,
        _ => reject(respond, UNIMPLEMENTED),
    };
    if let Err(err) = result {
        log::debug!("tophat stream {path}: {err}");
    }
}

async fn session(body: RecvStream, mut respond: SendResponse<Bytes>, waiters: &Waiters) -> Result<()> {
    let mut inbound = Inbound::new(body, REGISTER_MAX);
    let first = tokio::time::timeout(REGISTER_WITHIN, inbound.next()).await;
    let client_id = match first {
        Ok(Ok(Some(message))) => registered(&message),
        _ => None,
    };
    let Some(client_id) = client_id else {
        return reject(respond, INVALID_ARGUMENT);
    };
    let Some(waiter) = waiters.lock().unwrap().remove(&client_id) else {
        return reject(respond, PERMISSION_DENIED);
    };
    let (to_consumer, incoming) = unbounded();
    let (outgoing, from_consumer) = unbounded();
    if waiter.send(Channel { incoming, outgoing }).is_err() {
        return reject(respond, CANCELLED);
    }
    inbound.max = MAX_MESSAGE;
    let mut send = respond.send_response(grpc_response()?, false)?;
    tokio::select! {
        () = read_loop(inbound, to_consumer) => {}
        () = write_loop(&mut send, from_consumer) => {}
    }
    // Whichever side ended first, the call completes normally for the peer.
    let _ = send.send_trailers(status(OK));
    Ok(())
}

/// The client id of a `register` message; its `source` field.
fn registered(message: &[u8]) -> Option<String> {
    let table = Table::root(message)?;
    (table.text(0) == "register").then(|| table.text(4))
}

async fn read_loop(mut inbound: Inbound, to_consumer: UnboundedSender<Vec<u8>>) {
    loop {
        match inbound.next().await {
            Ok(Some(message)) => {
                if to_consumer.unbounded_send(message).is_err() {
                    return;
                }
            }
            Ok(None) => return,
            Err(err) => return log::debug!("tophat read: {err}"),
        }
    }
}

async fn write_loop(send: &mut SendStream<Bytes>, mut from_consumer: UnboundedReceiver<Vec<u8>>) {
    while let Some(message) = from_consumer.next().await {
        if let Err(err) = write(send, frame(&message)).await {
            return log::debug!("tophat write: {err}");
        }
    }
}

/// Accepted and ignored: the trainer needs neither telemetry nor events. The bytes are not
/// even framed, so an unauthenticated peer holds nothing here.
async fn drain(mut body: RecvStream, mut respond: SendResponse<Bytes>) -> Result<()> {
    let mut send = respond.send_response(grpc_response()?, false)?;
    while let Some(chunk) = body.data().await {
        body.flow_control().release_capacity(chunk?.len())?;
    }
    send.send_trailers(status(OK))?;
    Ok(())
}

fn grpc_response() -> Result<Response<()>> {
    Ok(Response::builder()
        .status(200)
        .header("content-type", "application/grpc")
        .body(())?)
}

/// A gRPC "trailers-only" response: the call failed before any message.
fn reject(mut respond: SendResponse<Bytes>, code: u16) -> Result<()> {
    let response = Response::builder()
        .status(200)
        .header("content-type", "application/grpc")
        .header("grpc-status", code)
        .body(())?;
    respond.send_response(response, true)?;
    Ok(())
}

fn status(code: u16) -> HeaderMap {
    let mut trailers = HeaderMap::new();
    trailers.insert("grpc-status", code.into());
    trailers
}

/// A request body read as gRPC frames; partial frames stay buffered between calls.
struct Inbound {
    body: RecvStream,
    buf: BytesMut,
    /// The largest message accepted.
    max: usize,
}

impl Inbound {
    fn new(body: RecvStream, max: usize) -> Self {
        Self {
            body,
            buf: BytesMut::new(),
            max,
        }
    }

    /// The next whole message, or `None` once the peer ended the stream between messages.
    async fn next(&mut self) -> Result<Option<Vec<u8>>> {
        loop {
            if let Some(message) = take_message(&mut self.buf, self.max)? {
                return Ok(Some(message));
            }
            let Some(chunk) = self.body.data().await else {
                ensure!(self.buf.is_empty(), "stream ended inside a message");
                return Ok(None);
            };
            let chunk = chunk?;
            self.body.flow_control().release_capacity(chunk.len())?;
            self.buf.extend_from_slice(&chunk);
        }
    }
}

fn take_message(buf: &mut BytesMut, max: usize) -> Result<Option<Vec<u8>>> {
    let Some(head) = buf.first_chunk::<5>() else {
        return Ok(None);
    };
    ensure!(head[0] == 0, "compressed message");
    let len = u32::from_be_bytes([head[1], head[2], head[3], head[4]]) as usize;
    ensure!(len <= max, "message of {len} bytes");
    if buf.len() < 5 + len {
        return Ok(None);
    }
    buf.advance(5);
    Ok(Some(buf.split_to(len).to_vec()))
}

fn frame(message: &[u8]) -> Bytes {
    let mut frame = BytesMut::with_capacity(5 + message.len());
    frame.put_u8(0);
    frame.put_u32(message.len() as u32);
    frame.extend_from_slice(message);
    frame.freeze()
}

/// Sends `data` as the flow-control window opens.
async fn write(send: &mut SendStream<Bytes>, mut data: Bytes) -> Result<()> {
    while !data.is_empty() {
        send.reserve_capacity(data.len());
        let Some(granted) = poll_fn(|cx| send.poll_capacity(cx)).await else {
            bail!("stream closed");
        };
        send.send_data(data.split_to(granted?.min(data.len())), false)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::future::Future;

    use h2::client::{self, ResponseFuture};

    use super::{
        super::wire::{Field, table},
        *,
    };

    async fn soon<T>(work: impl Future<Output = T>) -> T {
        tokio::time::timeout(Duration::from_secs(5), work)
            .await
            .expect("timed out")
    }

    fn register(source: &str) -> Vec<u8> {
        table(&[(0, Field::Str("register")), (4, Field::Str(source))])
    }

    fn put(send: &mut SendStream<Bytes>, data: Bytes) {
        send.send_data(data, false).unwrap();
    }

    async fn call(server: &Server, path: &str) -> (SendStream<Bytes>, ResponseFuture) {
        let socket = TcpStream::connect((Ipv4Addr::LOCALHOST, server.port))
            .await
            .unwrap();
        let (sender, connection) = client::handshake(socket).await.unwrap();
        tokio::spawn(connection);
        let request = Request::post(format!("http://127.0.0.1{path}"))
            .body(())
            .unwrap();
        let mut sender = sender.ready().await.unwrap();
        let (response, send) = sender.send_request(request, false).unwrap();
        (send, response)
    }

    async fn session(server: &Server, id: &str) -> (SendStream<Bytes>, RecvStream, Channel) {
        let waiting = server.expect(id);
        let (mut send, response) = call(server, SESSION).await;
        put(&mut send, frame(&register(id)));
        let channel = soon(waiting).await.unwrap();
        let body = soon(response).await.unwrap().into_body();
        (send, body, channel)
    }

    async fn trailer_status(body: &mut RecvStream) -> String {
        assert!(soon(body.data()).await.is_none());
        let trailers = soon(body.trailers()).await.unwrap().unwrap();
        trailers["grpc-status"].to_str().unwrap().to_owned()
    }

    #[tokio::test]
    async fn a_registered_session_carries_messages_both_ways() {
        let server = Server::start().unwrap();
        let (mut send, body, mut channel) = session(&server, "client-1").await;
        let mut inbound = Inbound::new(body, MAX_MESSAGE);

        channel.outgoing.unbounded_send(b"to service".to_vec()).unwrap();
        assert_eq!(soon(inbound.next()).await.unwrap().unwrap(), b"to service");

        // One message across two data frames, then one right after it.
        let framed = frame(b"split across chunks");
        put(&mut send, framed.slice(..3));
        put(&mut send, framed.slice(3..));
        put(&mut send, frame(b"next"));
        assert_eq!(soon(channel.incoming.next()).await.unwrap(), b"split across chunks");
        assert_eq!(soon(channel.incoming.next()).await.unwrap(), b"next");
    }

    #[tokio::test]
    async fn a_closing_service_ends_incoming_and_the_call_completes() {
        let server = Server::start().unwrap();
        let (mut send, mut body, mut channel) = session(&server, "client-1").await;
        send.send_data(Bytes::new(), true).unwrap();
        assert!(soon(channel.incoming.next()).await.is_none());
        assert_eq!(trailer_status(&mut body).await, "0");
    }

    #[tokio::test]
    async fn dropping_the_channel_ends_the_stream_with_ok() {
        let server = Server::start().unwrap();
        let (_send, mut body, channel) = session(&server, "client-1").await;
        drop(channel.outgoing);
        assert_eq!(trailer_status(&mut body).await, "0");
    }

    #[tokio::test]
    async fn side_streams_are_accepted_and_ignored() {
        let server = Server::start().unwrap();
        for path in SIDE_STREAMS {
            let (mut send, response) = call(&server, path).await;
            put(&mut send, frame(b"ignored"));
            send.send_data(Bytes::new(), true).unwrap();
            let mut body = soon(response).await.unwrap().into_body();
            assert_eq!(trailer_status(&mut body).await, "0");
        }
    }

    #[tokio::test]
    async fn refuses_what_it_does_not_serve() {
        let server = Server::start().unwrap();
        drop(server.expect("gone"));
        let cases = [
            (SESSION, register("stranger"), "7"),
            (SESSION, register("gone"), "1"),
            (SESSION, table(&[(0, Field::Str("heartbeat"))]), "3"),
            (SESSION, vec![0; REGISTER_MAX + 1], "3"),
            ("/tophat.TophatService/Other", register("expected"), "12"),
        ];
        for (path, first, code) in cases {
            let (mut send, response) = call(&server, path).await;
            put(&mut send, frame(&first));
            let response = soon(response).await.unwrap();
            assert_eq!(response.headers()["grpc-status"], code, "{path}");
        }
    }

    #[test]
    fn expecting_forgets_abandoned_waiters() {
        let server = Server::start().unwrap();
        drop(server.expect("abandoned"));
        let _kept = server.expect("kept");
        assert_eq!(server.waiters.lock().unwrap().len(), 1);
    }

    #[test]
    fn frames_wait_for_their_bytes_and_refuse_odd_ones() {
        let framed = frame(b"abc");
        let mut buf = BytesMut::from(&framed[..4]);
        assert_eq!(take_message(&mut buf, MAX_MESSAGE).unwrap(), None);
        buf.extend_from_slice(&framed[4..]);
        assert_eq!(take_message(&mut buf, MAX_MESSAGE).unwrap().unwrap(), b"abc");
        assert!(buf.is_empty());

        assert!(take_message(&mut BytesMut::from(&[1u8, 0, 0, 0, 0][..]), MAX_MESSAGE).is_err());
        let mut huge = BytesMut::from(&[0u8][..]);
        huge.put_u32(MAX_MESSAGE as u32 + 1);
        assert!(take_message(&mut huge, MAX_MESSAGE).is_err());
    }
}

