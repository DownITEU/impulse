//! The remote panel's server: the built panel, pairing and the WebSocket, on this machine's LAN address. It keeps no
//! app state: pairing, tokens, commands and covers go to the app through `client::api::ServerSide`.

mod address;
mod http;
mod socket;

use std::{
    future::Future,
    io,
    net::{SocketAddr, TcpListener as StdListener},
    time::Duration,
};

use async_io::Timer;
use async_net::TcpListener;
use client::api::ServerSide;
use futures::{FutureExt, StreamExt, future, select, stream::FuturesUnordered};

/// Connections open at once.
const CONNECTIONS: usize = 16;
/// Ports tried from the asked one up.
const PORTS: u16 = 20;
/// Reading a request, writing an answer, waiting for the app.
const DEADLINE: Duration = Duration::from_secs(10);

pub struct Server {
    listener: TcpListener,
    address: SocketAddr,
}

impl Server {
    /// Listens on this machine's LAN address, on `port` or the next free one.
    pub fn bind(port: u16) -> io::Result<Self> {
        let ip = address::lan()?;
        let free = (port..port.saturating_add(PORTS)).find_map(|port| StdListener::bind((ip, port)).ok());
        Self::over(free.ok_or_else(|| io::Error::new(io::ErrorKind::AddrInUse, "no free port"))?)
    }

    fn over(listener: StdListener) -> io::Result<Self> {
        let address = listener.local_addr()?;
        Ok(Self { listener: listener.try_into()?, address })
    }

    pub fn address(&self) -> SocketAddr {
        self.address
    }

    /// Serves until dropped; dropping it closes every connection.
    pub async fn serve(self, side: ServerSide) {
        let mut connections = FuturesUnordered::new();
        loop {
            // Full: the rest wait in the backlog.
            let full = connections.len() >= CONNECTIONS;
            let accept = async { if full { future::pending().await } else { self.listener.accept().await } };
            select! {
                accepted = accept.fuse() => match accepted {
                    Ok((stream, peer)) if address::private(peer.ip()) => {
                        connections.push(http::connection(stream, self.address, &side));
                    }
                    Ok(_) => {}
                    Err(e) => {
                        log::warn!("remote: accept: {e}");
                        Timer::after(Duration::from_millis(200)).await;
                    }
                },
                () = connections.select_next_some() => {}
            }
        }
    }
}

/// `work`, unless it takes longer than `DEADLINE`.
async fn within<T>(work: impl Future<Output = T>) -> Option<T> {
    select! {
        done = work.fuse() => Some(done),
        _ = FutureExt::fuse(Timer::after(DEADLINE)) => None,
    }
}

#[cfg(test)]
mod tests;
