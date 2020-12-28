use std::net::IpAddr;

use super::{settings, DNSServer};
use domain::base::iana::Class;
use domain::base::{Dname, Message, Record};
use domain::rdata::{Aaaa, A};
use glob::Pattern;

impl DNSServer {
    pub fn load_patterns(settings: &settings::DNSSettings) -> Vec<(Pattern, String)> {
        let mut patterns = vec![];
        for (key, value) in &settings.custom_hosts {
            if let Ok(pattern) = Pattern::new(key.as_str()) {
                patterns.push((pattern, value.clone()));
            } else {
                println!("[Custom] Failed to load pattern {}", key);
            }
        }
        patterns
    }
    pub fn lookup_custom(&self, message: &Message<Vec<u8>>) -> Option<Message<Vec<u8>>> {
        let domain = message.first_question().unwrap().qname().to_string();
        for (pattern, value) in &self.custom_patterns {
            if pattern.matches(domain.as_str()) {
                let ip: IpAddr = value.parse().unwrap();
                let ret_message;
                match ip {
                    IpAddr::V4(ip4) => {
                        let record = Record::new(Dname::root_ref(), Class::In, 60, A::new(ip4));
                        ret_message = self.decorate_message(&message, Some(vec![record]));
                    }
                    IpAddr::V6(ip6) => {
                        let record = Record::new(Dname::root_ref(), Class::In, 60, Aaaa::new(ip6));
                        ret_message = self.decorate_message(&message, Some(vec![record]));
                    }
                }

                return Some(ret_message);
            }
        }
        None
    }
}
