mod geoip;
mod passthrough;
mod proxy;
mod utils;

use futures::FutureExt;
pub use geoip::GeoIP;
use passthrough::passthrough;
use proxy::proxy;
use std::{io::Error, process::exit};
use tokio::net::TcpListener;
use utils::get_original_dest;

pub struct Router {
    geoip: GeoIP,
    server: TcpListener,
}

impl Router {
    pub async fn new() -> Self {
        let geoip = GeoIP::new().await;

        let server = match TcpListener::bind("0.0.0.0:3333").await {
            Ok(server) => {
                println!("Traffic routing service now is serving on tcp://0.0.0.0:3333");
                server
            }
            Err(_) => {
                println!("error on tcp listening");
                exit(-1);
            }
        };

        Router { geoip, server }
    }

    pub async fn start(&self) -> Result<(), Error> {
        loop {
            let (socket, addr) = self.server.accept().await?;

            let dst_addr = match get_original_dest(&socket) {
                Ok(addr) => addr,
                Err(err) => {
                    println!("{}", err);
                    continue;
                }
            };
            let dst_ip = dst_addr.ip();
            let dst_port = dst_addr.port();
            let country_code = self.geoip.lookup_country_code(dst_ip);

            let info_message = format!(
                "from {}, to {}:{} in {}",
                addr, dst_ip, dst_port, country_code
            );

            if country_code != "CN" {
                let transfer = proxy(socket, dst_addr, info_message).map(|r| {
                    if let Err(e) = r {
                        println!("Failed to proxy {}", e);
                    } else if let Ok(info) = r {
                        println!("Success to proxy {}", info);
                    }
                });
                tokio::spawn(transfer);
            } else {
                let transfer = passthrough(socket, dst_addr, info_message).map(|r| {
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
}
