//! The address the server listens on, and the peers it answers.

use std::{
    io,
    net::{IpAddr, Ipv4Addr, UdpSocket},
};

/// The address this machine reaches its LAN from: a UDP connect picks the route and sends nothing.
pub fn lan() -> io::Result<IpAddr> {
    let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0))?;
    // TEST-NET-1: routed like any outside address, never reached.
    socket.connect((Ipv4Addr::new(192, 0, 2, 1), 9))?;
    let ip = socket.local_addr()?.ip();
    match private(ip) && !ip.is_loopback() {
        true => Ok(ip),
        false => Err(io::Error::new(io::ErrorKind::AddrNotAvailable, "no LAN address")),
    }
}

/// A peer on a private network or this machine.
pub fn private(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => ip.is_private() || ip.is_link_local() || ip.is_loopback(),
        IpAddr::V6(ip) => ip.is_loopback() || ip.is_unique_local() || ip.is_unicast_link_local(),
    }
}
