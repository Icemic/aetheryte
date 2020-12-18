use nix::sys::socket::getsockopt;
use nix::sys::socket::sockopt::OriginalDst;
use std::{io::ErrorKind, net::SocketAddrV4, os::unix::io::AsRawFd};
use tokio::net::TcpStream;
use tokio::prelude::*;

pub fn get_original_dest(fd: &TcpStream) -> io::Result<SocketAddrV4> {
    let addr = getsockopt(fd.as_raw_fd(), OriginalDst).map_err(|e| match e {
        nix::Error::Sys(err) => io::Error::from(err),
        _ => io::Error::new(ErrorKind::Other, e),
    })?;
    let addr = SocketAddrV4::new(
        u32::from_be(addr.sin_addr.s_addr).into(),
        u16::from_be(addr.sin_port),
    );
    Ok(addr)
}
