extern crate domain;
extern crate maxminddb;
extern crate rustls_native_certs;
extern crate tokio;
extern crate tokio_rustls;

mod dns;
mod router;

use dns::DNSServer;
use futures::future::try_join;
use router::Router;
use std::process::exit;

#[tokio::main]
async fn main() {
    println!("Awaki is running.");

    ctrlc::set_handler(|| {
        println!("\nGoodbye.");
        exit(0);
    })
    .expect("Error setting Ctrl-C handler");

    let dns_server = DNSServer::new().await;
    let traffic_router = Router::new().await;

    match try_join(dns_server.start(), traffic_router.start()).await {
        Ok(_) => {}
        Err(err) => {
            panic!(err);
        }
    }
}
