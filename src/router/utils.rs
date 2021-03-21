use nix::sys::socket::getsockopt;
use nix::sys::socket::sockopt::OriginalDst;
use std::{io::ErrorKind, net::IpAddr, os::unix::io::AsRawFd};
use tokio::io;
use tokio::net::TcpStream;

const AF_INET: u16 = libc::AF_INET as u16;
const AF_INET6: u16 = libc::AF_INET6 as u16;

pub fn get_original_dest(fd: &TcpStream) -> io::Result<(IpAddr, u16)> {
    let addr = getsockopt(fd.as_raw_fd(), OriginalDst).map_err(|e| match e {
        nix::Error::Sys(err) => io::Error::from(err),
        _ => io::Error::new(ErrorKind::Other, e),
    })?;
    // libc::IP6T_SO_ORIGINAL_DST
    libc::SOL_IPV6;
    let ip = match addr.sin_family {
        AF_INET => {
            IpAddr::V4(u32::from_be(addr.sin_addr.s_addr).into())
        }
        AF_INET6 => {
            IpAddr::V6(u128::from_be(addr.sin_addr.s_addr).into())
        }
    };
    let port = u16::from_be(addr.sin_port);
    Ok((ip, port))
}
