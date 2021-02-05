mod dns;
mod router;

use dns::DNSServer;
use futures::future::try_join;
use router::Router;
use std::env;
use std::process::exit;

#[tokio::main]
async fn main() {
    println!("Aetheryte is running.");

    let is_transfer_only = env::var("TRANSFER_ONLY")
        .map(|v| {
            println!("TRANSFER_ONLY: {}", v);
            true
        })
        .unwrap_or(false);

    ctrlc::set_handler(|| {
        println!("\nGoodbye.");
        exit(0);
    })
    .expect("Error setting Ctrl-C handler");

    if is_transfer_only {
        let traffic_router = Router::new().await;
        traffic_router.start().await.unwrap();
    } else {
        let dns_server = DNSServer::new().await;
        let traffic_router = Router::new().await;

        match try_join(dns_server.start(), traffic_router.start()).await {
            Ok(_) => {}
            Err(err) => {
                panic!(err);
            }
        }
    }
}
