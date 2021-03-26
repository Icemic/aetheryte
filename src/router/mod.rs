mod geoip;
mod passthrough;
mod proxy;
mod utils;

use futures::{future::try_join, FutureExt};
pub use geoip::GeoIP;
use passthrough::passthrough;
use proxy::proxy;
use std::{io::Error, process::exit};
use tokio::net::TcpListener;
use utils::{get_original6_dest, get_original_dest};

pub struct Router {
    geoip: GeoIP,
    server_ipv4: TcpListener,
    server_ipv6: TcpListener,
}

impl Router {
    pub async fn new() -> Self {
        let geoip = GeoIP::new().await;

        let server_ipv4 = match TcpListener::bind("0.0.0.0:3333").await {
            Ok(server) => {
                println!("Traffic routing service now is serving on tcp://0.0.0.0:3333");
                server
            }
            Err(e) => {
                println!("error on tcp listening: {}", e);
                exit(-1);
            }
        };

        let server_ipv6 = match TcpListener::bind("[::1]:3333").await {
            Ok(server) => {
                println!("Traffic routing service now is serving on tcp://[::1]:3333");
                server
            }
            Err(e) => {
                println!("error on tcp listening: {}", e);
                exit(-1);
            }
        };

        Router {
            geoip,
            server_ipv4,
            server_ipv6,
        }
    }

    pub async fn start(&self) -> Result<(), Error> {
        match try_join(self.start_for_ipv4(), self.start_for_ipv6()).await {
            Ok(_) => Ok(()),
            Err(err) => Err(err),
        }
    }

    pub async fn start_for_ipv4(&self) -> Result<(), Error> {
        loop {
            let (socket, addr) = self.server_ipv4.accept().await?;

            let (dst_ip, dst_port) = match get_original_dest(&socket) {
                Ok(addr) => addr,
                Err(err) => {
                    println!("error on getting original destination: {}", err);
                    continue;
                }
            };

            let country_code = self.geoip.lookup_country_code(&dst_ip);

            let info_message = format!(
                "from {}, to {}:{} in {}",
                addr, dst_ip, dst_port, country_code
            );

            if country_code != "CN" {
                let transfer = proxy(socket, dst_ip, dst_port, info_message).map(|r| {
                    if let Err(e) = r {
                        println!("Failed to proxy {}", e);
                    } else if let Ok(info) = r {
                        println!("Success to proxy {}", info);
                    }
                });
                tokio::spawn(transfer);
            } else {
                let transfer = passthrough(socket, dst_ip, dst_port, info_message).map(|r| {
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

    pub async fn start_for_ipv6(&self) -> Result<(), Error> {
        loop {
            let (socket, addr) = self.server_ipv6.accept().await?;

            let (dst_ip, dst_port) = match get_original6_dest(&socket) {
                Ok(addr) => addr,
                Err(err) => {
                    println!("error on getting original destination: {}", err);
                    continue;
                }
            };

            let info_message = format!(
                "from {}, to {}:{} in Unknown(ipv6 address)",
                addr, dst_ip, dst_port
            );

            let transfer = passthrough(socket, dst_ip, dst_port, info_message).map(|r| {
                if let Err(e) = r {
                    println!("Failed to transfer {}", e);
                } else if let Ok(info) = r {
                    println!("Success to transfer {}", info);
                }
            });

            tokio::spawn(transfer);
        }
        #[allow(unreachable_code)]
        Ok(())
    }
}
