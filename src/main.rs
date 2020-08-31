use maxminddb::geoip2;
use maxminddb::Reader;
use nix::sys::socket::{getsockopt, sockopt::OriginalDst};
use std::net::{IpAddr, Ipv4Addr};
use std::process::exit;
use std::{io::ErrorKind, net::SocketAddrV4, os::unix::io::AsRawFd};
use tokio::fs;
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::{TcpListener, TcpStream};
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

async fn one_way_pass(mut reader: OwnedReadHalf, mut writer: OwnedWriteHalf) {
    tokio::spawn(async move {
        let mut buf = [0; 40960];

        // In a loop, read data from the socket and write the data back.
        loop {
            let n = match reader.read(&mut buf).await {
                // socket closed
                Ok(n) if n == 0 => {
                    println!("connection closed");
                    return;
                }
                Ok(n) => n,
                Err(e) => {
                    println!("failed to read from socket; err = {:?}", e);
                    return;
                }
            };

            // print!("{:?}", std::str::from_utf8(&buf[0..n]).unwrap());

            // Write the data back
            if let Err(e) = writer.write_all(&buf[0..n]).await {
                println!("failed to write to socket; err = {:?}", e);
                return;
            }
        }
    });
}

#[tokio::main]
async fn main() {
    println!("Hello, world!");

    let geo_db_file = match fs::read("data/GeoLite2-Country.mmdb").await {
        Ok(file) => file,
        Err(_) => {
            println!("failed to open geolite2 mmdb file");
            exit(-1);
        }
    };

    let reader = Reader::from_source(geo_db_file).unwrap();

    let mut server = match TcpListener::bind("0.0.0.0:3333").await {
        Ok(server) => server,
        Err(_) => {
            println!("error on tcp listening");
            exit(-1);
        }
    };

    loop {
        let (socket, addr) = match server.accept().await {
            Ok(socket) => socket,
            Err(_) => {
                println!("error on accept socket");
                continue;
            }
        };

        println!("New connection: {}", addr);

        let dst_addr = get_original_dest(&socket).unwrap();
        let dst_ip = dst_addr.ip();
        let dst_port = dst_addr.port();
        let info: geoip2::Country = match reader.lookup(IpAddr::V4(*dst_ip)) {
            Ok(info) => info,
            Err(_) => {
                println!("error on lookup addr geo info");
                // fallback to US
                reader
                    .lookup(IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8)))
                    .unwrap()
            }
        };
        let dst_country_code = info.country.unwrap().iso_code.unwrap_or_default();

        println!("dst: {}:{}, at {}", dst_ip, dst_port, dst_country_code);

        let stream = match TcpStream::connect(dst_addr).await {
            Ok(stream) => stream,
            Err(_) => {
                println!("error on connecting to destination");
                continue;
            }
        };

        let (socket_reader, socket_writter) = socket.into_split();
        let (stream_reader, stream_writter) = stream.into_split();

        one_way_pass(socket_reader, stream_writter).await;
        one_way_pass(stream_reader, socket_writter).await;
    }
}
