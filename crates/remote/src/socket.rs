//! The WebSocket of a device: the library's handshake over the bytes already read, then views out and commands in.

use std::{
    io,
    pin::Pin,
    task::{Context, Poll},
    time::{Duration, Instant},
};

use async_io::Timer;
use async_net::TcpStream;
use async_tungstenite::{
    WebSocketStream, accept_async_with_config,
    tungstenite::{Message, protocol::WebSocketConfig},
};
use client::api::{Ask, Command, ServerSide};
use futures::{AsyncRead, AsyncWrite, FutureExt, StreamExt, select};

use crate::within;

const MESSAGE: usize = 64 * 1024;
const FRAME: usize = 16 * 1024;
/// Unsent bytes a socket may hold before it ends.
const WRITTEN: usize = 1 << 20;
/// A socket silent for two pings is a phone gone without closing it.
const PING: Duration = Duration::from_secs(30);

/// Serves `device`'s socket until it closes, stalls or the device is revoked.
pub async fn serve(stream: TcpStream, read: Vec<u8>, device: u32, side: &ServerSide) {
    let config = WebSocketConfig::default().max_message_size(Some(MESSAGE)).max_frame_size(Some(FRAME)).max_write_buffer_size(WRITTEN);
    let Some(Ok(mut socket)) = within(accept_async_with_config(Replay { read, at: 0, stream }, Some(config))).await else {
        return;
    };
    let (mut wake, mut seen) = side.feed().watch();
    let (mut ping, mut heard) = (Timer::interval(PING), Instant::now());
    // The first look sends every view.
    let mut look = true;
    loop {
        if look {
            let Some(views) = side.feed().since(device, &mut seen) else { break };
            for json in views {
                if !sent(&mut socket, Message::text(json.as_ref())).await {
                    return;
                }
            }
            look = false;
        }
        select! {
            message = socket.next().fuse() => {
                heard = Instant::now();
                match message {
                    Some(Ok(Message::Text(text))) => match serde_json::from_str::<Command>(text.as_str()) {
                        // One command at a time: a stalled app ends the socket.
                        Ok(command) => {
                            if within(side.ask(Ask::Command { device, command })).await != Some(true) {
                                break;
                            }
                        }
                        // The error quotes the message.
                        Err(_) => log::debug!("remote: a command that does not read"),
                    },
                    // The next read sends the library's reply.
                    Some(Ok(Message::Close(_))) => return drop(within(socket.next()).await),
                    Some(Err(_)) | None => break,
                    Some(Ok(_)) => {}
                }
            }
            woken = wake.next() => match woken {
                Some(()) => look = true,
                None => break,
            },
            _ = ping.next().fuse() => {
                if heard.elapsed() > 2 * PING || !sent(&mut socket, Message::Ping(Default::default())).await {
                    return;
                }
            }
        }
    }
    let _ = within(socket.close(None)).await;
}

/// A phone that stops reading is let go.
async fn sent(socket: &mut WebSocketStream<Replay>, message: Message) -> bool {
    matches!(within(socket.send(message)).await, Some(Ok(())))
}

/// The connection, with the bytes already read in front.
struct Replay {
    read: Vec<u8>,
    at: usize,
    stream: TcpStream,
}

impl AsyncRead for Replay {
    fn poll_read(mut self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &mut [u8]) -> Poll<io::Result<usize>> {
        if self.at < self.read.len() {
            let count = buf.len().min(self.read.len() - self.at);
            buf[..count].copy_from_slice(&self.read[self.at..self.at + count]);
            self.at += count;
            return Poll::Ready(Ok(count));
        }
        Pin::new(&mut self.stream).poll_read(cx, buf)
    }
}

impl AsyncWrite for Replay {
    fn poll_write(mut self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &[u8]) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.stream).poll_write(cx, buf)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.stream).poll_flush(cx)
    }

    fn poll_close(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.stream).poll_close(cx)
    }
}
