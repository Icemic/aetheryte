use std::collections::HashMap;

use crate::dns::DNSServer;
use serde::{Deserialize, Serialize};
use tokio::fs;

#[derive(Serialize, Deserialize, Debug)]
pub struct DNSSettings {
    upstreams: Vec<DNSServerUpstream>,
    custom_hosts: HashMap<String, String>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct DNSServerUpstream {
    address: String,
    hostname: String,
    enable_udp: bool,
    enable_tcp: bool,
    enable_doh: bool,
    enable_dot: bool,
    is_china: bool,
}

impl DNSServer {
    pub async fn load_settings() -> DNSSettings {
        match fs::read_to_string("data/dns_settings.json").await {
            Ok(text) => {
                let settings: DNSSettings =
                    serde_json::from_str(text.as_str()).expect("Failed to load settings.");
                println!("dns settings:\n{:?}", settings);
                settings
            }
            Err(_) => {
                panic!("failed to open geolite2 mmdb file");
            }
        }
    }
}
