use std::collections::HashMap;

use crate::dns::DNSServer;
use serde::{Deserialize, Serialize};
use tokio::fs;

#[derive(Serialize, Deserialize, Debug)]
pub struct DNSSettings {
    pub listen_ip: std::string::String,
    pub listen_port: u16,
    pub query_timeout: u32,
    pub upstreams: Vec<DNSServerUpstream>,
    pub custom_hosts: HashMap<String, String>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct DNSServerUpstream {
    pub address: String,
    pub hostname: String,
    pub enable_udp: bool,
    pub enable_tcp: bool,
    pub enable_doh: bool,
    pub enable_dot: bool,
    pub is_china: bool,
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
                panic!("failed to open settings file.");
            }
        }
    }
}
