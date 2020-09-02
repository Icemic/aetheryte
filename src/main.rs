use futures::future::try_join;
use futures::FutureExt;
use maxminddb::geoip2;
use maxminddb::Reader;
use nix::sys::socket::{getsockopt, sockopt::OriginalDst};
use std::error::Error;
use std::net::{IpAddr, Ipv4Addr};
use std::process::exit;
use std::{io::ErrorKind, net::SocketAddrV4, os::unix::io::AsRawFd};
use tokio::fs;
// use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::{TcpListener, TcpStream};
use tokio::prelude::*;
use ctrlc;

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

// async fn one_way_pass(mut reader: OwnedReadHalf, mut writer: OwnedWriteHalf) {
//     let mut buf = [0; 40960];

//     // In a loop, read data from the socket and write the data back.
//     loop {
//         let n = match reader.read(&mut buf).await {
//             // socket closed
//             Ok(n) if n == 0 => {
//                 println!("connection closed");
//                 return;
//             }
//             Ok(n) => n,
//             Err(e) => {
//                 println!("failed to read from socket; err = {:?}", e);
//                 return;
//             }
//         };

//         // print!("{:?}", std::str::from_utf8(&buf[0..n]).unwrap());

//         // Write the data back
//         if let Err(e) = writer.write_all(&buf[0..n]).await {
//             println!("failed to write to socket; err = {:?}", e);
//             return;
//         }
//     }
// }

async fn transfer(mut inbound: TcpStream, dst_addr: SocketAddrV4) -> Result<(), Box<dyn Error>> {
    let mut outbound = TcpStream::connect(dst_addr).await?;

    let (mut ri, mut wi) = inbound.split();
    let (mut ro, mut wo) = outbound.split();

    let client_to_server = async {
        io::copy(&mut ri, &mut wo).await?;
        wo.shutdown().await
    };

    let server_to_client = async {
        io::copy(&mut ro, &mut wi).await?;
        wi.shutdown().await
    };

    try_join(client_to_server, server_to_client).await?;

    Ok(())
}

async fn proxy(mut inbound: TcpStream, dst_addr: SocketAddrV4) -> Result<(), Box<dyn Error>> {
    let mut outbound = TcpStream::connect("127.0.0.1:1086").await?;

    outbound.write(&[5, 1, 0]).await?;

    let mut buf = [0 as u8; 20];
    let size = outbound.read(&mut buf).await.unwrap();
    if size != 2 || buf[0] != 5 || buf[1] != 0 {
        panic!("connect to socks5 proxy failed.");
    }

    let mut data: [u8; 10] = [5, 1, 0, 1, 0, 0, 0, 0, 0, 0];
    let mut i = 4;
    for item in dst_addr.ip().to_string().split(".") {
        data[i] = item.parse::<u8>().unwrap();
        i = i + 1;
    }

    let port_big_endian = dst_addr.port().to_be_bytes();

    data[8] = port_big_endian[0];
    data[9] = port_big_endian[1];

    outbound.write(&data).await?;

    let size = outbound.read(&mut buf).await.unwrap();
    if size == 10 && buf[0] == 5 && buf[1] == 0 {
        // println!("连接建立成功");
    } else {
        panic!("connect to socks5 proxy failed with error code {}", buf[1]);
    }

    let (mut ri, mut wi) = inbound.split();
    let (mut ro, mut wo) = outbound.split();

    let client_to_server = async {
        io::copy(&mut ri, &mut wo).await?;
        wo.shutdown().await
    };

    let server_to_client = async {
        io::copy(&mut ro, &mut wi).await?;
        wi.shutdown().await
    };

    try_join(client_to_server, server_to_client).await?;

    Ok(())
}

#[tokio::main]
async fn main() {
    println!("Awaki is running.");

    ctrlc::set_handler(|| {
        println!("\nGoodbye.");
        exit(0);
    }).expect("Error setting Ctrl-C handler");

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

        let dst_addr = match get_original_dest(&socket) {
            Ok(addr) => addr,
            Err(err) => {
                println!("{}", err);
                continue;
            }
        };
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

        println!(
            "from {}, to: {}:{} in {}",
            addr, dst_ip, dst_port, dst_country_code
        );

        if dst_country_code != "CN" {
            let transfer = proxy(socket, dst_addr.clone()).map(|r| {
                if let Err(e) = r {
                    println!("Failed to transfer: {}", e);
                }
            });
            tokio::spawn(transfer);
        } else {
            let transfer = transfer(socket, dst_addr.clone()).map(|r| {
                if let Err(e) = r {
                    println!("Failed to transfer: {}", e);
                }
            });
            tokio::spawn(transfer);
        }

        // let stream = match TcpStream::connect(dst_addr).await {
        //     Ok(stream) => stream,
        //     Err(_) => {
        //         println!("error on connecting to destination");
        //         continue;
        //     }
        // };

        // let (mut socket_reader, mut socket_writter) = socket.into_split();
        // let (mut stream_reader, mut stream_writter) = stream.into_split();

        // let client_to_server = async {
        //     io::copy(&mut socket_reader, &mut stream_writter).await?;
        //     stream_writter.shutdown().await
        // };
        // let server_to_client = async {
        //     io::copy(&mut stream_reader, &mut socket_writter).await?;
        //     socket_writter.shutdown().await
        // };
        // let transfer = async {
        //     try_join(client_to_server, server_to_client).await
        // };

        // tokio::spawn(transfer);

        // tokio::spawn(one_way_pass(socket_reader, stream_writter));
        // tokio::spawn(one_way_pass(stream_reader, socket_writter));
        // println!("aaa");
    }
}
