mod geoip;
mod passthrough;
mod proxy;
mod utils;

use futures::FutureExt;
use geoip::GeoIP;
use passthrough::passthrough;
use proxy::proxy;
use std::process::exit;
use std::sync::Arc;
use tokio::net::TcpListener;
use utils::get_original_dest;

pub async fn main() -> Result<(), ()> {
    let geoip = GeoIP::new().await;

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
        let country_code = geoip.lookup_country_code(dst_ip);

        let info_message = format!(
            "from {}, to {}:{} in {}",
            addr, dst_ip, dst_port, country_code
        );

        let info_message = Arc::new(info_message);

        if country_code != "CN" {
            let transfer = proxy(socket, dst_addr.clone(), info_message.clone()).map(|r| {
                if let Err(e) = r {
                    println!("Failed to proxy {}", e);
                } else if let Ok(info) = r {
                    println!("Success to proxy {}", info);
                }
            });
            tokio::spawn(transfer);
        } else {
            let transfer = passthrough(socket, dst_addr.clone(), info_message.clone()).map(|r| {
                if let Err(e) = r {
                    println!("Failed to transfer {}", e);
                } else if let Ok(info) = r {
                    println!("Success to transfer {}", info);
                }
            });
            tokio::spawn(transfer);
        }
    }
    #[allow(unreachable_code)]
    Ok(())
}
