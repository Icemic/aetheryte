extern crate bytes;
extern crate domain;
extern crate maxminddb;
extern crate tokio;

mod dns;
mod router;

use dns::DNSServer;
use futures::future::try_join;
use router::main as router_main;
use std::process::exit;

#[tokio::main]
async fn main() {
    println!("Awaki is running.");

    ctrlc::set_handler(|| {
        println!("\nGoodbye.");
        exit(0);
    })
    .expect("Error setting Ctrl-C handler");

    let mut dns_server = DNSServer::new().await;
    // dns_server.start().await;

    // router_main().await;

    let x = async {
        dns_server.start().await
    };

    let y = async {
        router_main().await
    };

    match try_join(x, y).await {
        Ok(_) => {},
        Err(err) => {
            panic!(err);
        }
    }
}
