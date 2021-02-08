use crate::router::GeoIP;
use domain::{base::Message, rdata::AllRecordData};
use std::{net::Ipv4Addr, sync::Arc};

#[derive(Debug)]
pub enum QueryType {
    UDP,
    TCP,
    DoT,
    DoH,
    Custom,
    Cache,
}

pub enum QueryResponse {
    UDP(Message<Vec<u8>>),
    TCP(Message<Vec<u8>>),
    DoT(Message<Vec<u8>>),
    DoH(Message<Vec<u8>>),
    Custom(Message<Vec<u8>>),
    Cache(Message<Vec<u8>>),
}

// add 2-byte head to packet
pub fn get_wrapped_packet(message: &Message<Vec<u8>>) -> Vec<u8> {
    let buf = &mut message.as_octets().clone();
    let mut packet = Vec::with_capacity(2 + buf.len());
    packet.push((buf.len() >> 8) as u8);
    packet.push(buf.len() as u8);
    packet.append(buf);
    packet
}

pub fn is_china_site(message: &Message<Vec<u8>>, geoip: Arc<GeoIP>) -> bool {
    // let message = Message::from_octets(buf).unwrap();
    let answers = message.answer().unwrap().limit_to::<AllRecordData<_, _>>();
    let mut is_china = false;
    for answer in answers {
        if let Ok(answer_first_china) = answer {
            let rtype = answer_first_china.rtype().to_string();

            if rtype != "A" {
                continue;
            }

            let ip = answer_first_china.data().to_string();
            let ip: Ipv4Addr = match ip.parse() {
                Ok(ip) => ip,
                Err(_) => return false,
            };

            if rtype.starts_with('A') && geoip.lookup_country_code(&ip) == "CN" {
                is_china = true;
                break;
            }
        }
    }

    is_china
}

pub fn is_valid_response_udp(message: &Message<Vec<u8>>) -> bool {
    if !message.is_error() {
        if message.additional().is_ok() && message.additional().unwrap().count() != 0 {
            if (message.answer().is_ok() && message.answer().unwrap().count() != 0)
                || (message.authority().is_ok() && message.authority().unwrap().count() != 0)
            {
                if message.opt().is_some() && message.opt().unwrap().dnssec_ok() {
                    return true;
                }
            }
        }
        return false;
    }
    true
}
pub fn is_valid_response(message: &Message<Vec<u8>>) -> bool {
    if !message.is_error() {
        if message.additional().is_ok() && message.additional().unwrap().count() != 0 {
            if (message.answer().is_ok() && message.answer().unwrap().count() != 0)
                || (message.authority().is_ok() && message.authority().unwrap().count() != 0)
            {
                return true;
            }
        }
        return false;
    }
    true
}

pub fn get_message_from_response(response: QueryResponse) -> (Message<Vec<u8>>, QueryType) {
    match response {
        QueryResponse::UDP(message) => (message, QueryType::UDP),
        QueryResponse::TCP(message) => (message, QueryType::TCP),
        QueryResponse::DoT(message) => (message, QueryType::DoT),
        QueryResponse::DoH(message) => (message, QueryType::DoH),
        QueryResponse::Custom(message) => (message, QueryType::Custom),
        QueryResponse::Cache(message) => (message, QueryType::Cache),
    }
}

pub fn get_message_from_response_ref(response: &QueryResponse) -> (&Message<Vec<u8>>, QueryType) {
    match response {
        QueryResponse::UDP(message) => (message, QueryType::UDP),
        QueryResponse::TCP(message) => (message, QueryType::TCP),
        QueryResponse::DoT(message) => (message, QueryType::DoT),
        QueryResponse::DoH(message) => (message, QueryType::DoH),
        QueryResponse::Custom(message) => (message, QueryType::Custom),
        QueryResponse::Cache(message) => (message, QueryType::Cache),
    }
}
