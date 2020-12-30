use super::{
    custom::lookup_custom,
    doh::lookup_doh,
    dot::lookup_dot,
    settings::{self, DNSServerUpstream, DNSSettings},
    tcp::lookup_tcp,
    udp::lookup_udp,
    utils::{decorate_message, is_china_site, QueryResponse, QueryType},
};
use crate::router::GeoIP;
use core::panic;
use domain::rdata::A;
use domain::{
    base::{Dname, Message, Record},
    rdata::AllRecordData,
};
use futures::{
    future::{select, select_ok},
    pin_mut, Future,
};
use glob::Pattern;
use std::io::{Error, ErrorKind};
use std::rc::Rc;
use std::sync::Arc;
use std::{net::Ipv4Addr, pin::Pin};
use tokio::net::UdpSocket;
use tokio::time::{timeout, Duration};

pub struct DNSServer {
    server: UdpSocket,
    geoip: Arc<GeoIP>,
}

impl DNSServer {
    pub async fn new() -> Self {
        let geoip = Arc::new(GeoIP::new().await);
        let settings = Self::load_settings().await;
        let server =
            match UdpSocket::bind(format!("{}:{}", settings.listen_ip, settings.listen_port)).await
            {
                Ok(server) => {
                    println!(
                        "DNS service now is serving on udp://{}:{}",
                        settings.listen_ip, settings.listen_port
                    );
                    server
                }
                Err(e) => {
                    panic!("error on udp listening: {}", e);
                }
            };
        server.set_ttl(2).unwrap();
        DNSServer { server, geoip }
    }

    pub fn load_patterns(settings: &DNSSettings) -> Vec<(Pattern, String)> {
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

    pub async fn start(&self) -> Result<(), ()> {
        let settings = Self::load_settings().await;
        let custom_patterns = Self::load_patterns(&settings);
        let mut buf = vec![0u8; 4096];
        loop {
            let (size, addr) = match self.server.recv_from(&mut buf).await {
                Ok(socket) => socket,
                Err(_) => {
                    println!("error on accept socket");
                    continue;
                }
            };

            let task = run_task(
                settings.clone(),
                custom_patterns.clone(),
                addr.to_string(),
                buf[..size].to_vec(),
                self.geoip.clone(),
            );
            let buf = tokio::spawn(task).await.unwrap().unwrap();

            self.server
                .send_to(&buf, addr)
                .await
                .expect("failed to send back via udp.");
        }
        #[allow(unreachable_code)]
        Ok(())
    }
}
async fn run_task(
    settings: DNSSettings,
    custom_patterns: Vec<(Pattern, String)>,
    addr: String,
    buf: Vec<u8>,
    geoip: Arc<GeoIP>,
) -> Result<Vec<u8>, Error> {
    let message = Message::from_octets(buf).unwrap();
    let domain = message.first_question().unwrap().qname().to_string();

    let is_china;
    let response;
    if let Ok(r) = lookup_custom(&message, &custom_patterns, &domain).await {
        response = r;
        is_china = true;
    } else {
        let message = decorate_message::<Record<Dname<&[u8]>, A>>(&message, None);
        if let Ok((r, is_china_)) = batch_query(&message, &settings.upstreams, geoip).await {
            response = r;
            is_china = is_china_;
        } else {
            println!("[Warning] batch query timeout, skip the task.");
            return Err(Error::new(ErrorKind::Other, ""));
        }
    }

    let (ret_message, method) = get_message_from_response(response);

    // let ret_message = Message::from_octets(ret_message).unwrap();
    let answers = ret_message
        .answer()
        .unwrap()
        .limit_to::<AllRecordData<_, _>>();

    let mut i: u8 = 0;
    for answer in answers {
        i += 1;
        let answer = answer.expect("parsing has failed.");
        println!(
            "[{:?} {} {}] {} --> {} ({}) #{}",
            method,
            if is_china { "China" } else { "Abroad" },
            addr.to_string(),
            domain,
            answer.data().to_string(),
            answer.rtype(),
            i
        );
    }

    if i == 0 {
        println!(
            "[{:?} {} {}] {} --> {} ({}) #{}",
            method,
            if is_china { "China" } else { "Abroad" },
            addr.to_string(),
            domain,
            "-",
            "-",
            i
        );
    }

    Ok(ret_message.into_octets())
}

async fn batch_query(
    message: &Message<Vec<u8>>,
    upstreams: &Vec<DNSServerUpstream>,
    geoip: Arc<GeoIP>,
) -> Result<(QueryResponse, bool), Error> {
    let mut queries_china = vec![];
    let mut queries_abroad = vec![];

    for upstream in upstreams {
        let queries;
        if upstream.is_china {
            queries = &mut queries_china;
        } else {
            queries = &mut queries_abroad;
        }

        if upstream.enable_udp {
            let ret_message = lookup(QueryType::UDP, message, &upstream);
            queries.push(Box::pin(ret_message));
        }
        if upstream.enable_tcp {
            let ret_message = lookup(QueryType::TCP, message, &upstream);
            queries.push(Box::pin(ret_message));
        }
        if upstream.enable_dot {
            let ret_message = lookup(QueryType::DoT, message, &upstream);
            queries.push(Box::pin(ret_message));
        }
        if upstream.enable_doh {
            let ret_message = lookup(QueryType::DoH, message, &upstream);
            queries.push(Box::pin(ret_message));
        }
    }

    let duration = Duration::from_millis(1000);

    let (response, _) = timeout(duration, select_ok(queries_china)).await??;
    let (ret_message, _) = get_message_from_response_ref(&response);
    if is_china_site(&ret_message, geoip) {
        return Ok((response, true));
    }

    let (response, _) = timeout(duration, select_ok(queries_abroad)).await??;
    Ok((response, false))
}

async fn lookup(
    t: QueryType,
    message: &Message<Vec<u8>>,
    upstream: &DNSServerUpstream,
) -> Result<QueryResponse, Error> {
    match t {
        QueryType::UDP => lookup_udp(message, &upstream.address).await,
        QueryType::TCP => lookup_tcp(message, &upstream.address).await,
        QueryType::DoT => lookup_dot(message, &upstream.address, &upstream.hostname).await,
        QueryType::DoH => lookup_doh(message, &upstream.address, &upstream.hostname).await,
        QueryType::Custom => panic!("Custom query should be performed independently"),
    }
}

fn get_message_from_response(response: QueryResponse) -> (Message<Vec<u8>>, QueryType) {
    match response {
        QueryResponse::UDP(message) => (message, QueryType::UDP),
        QueryResponse::TCP(message) => (message, QueryType::TCP),
        QueryResponse::DoT(message) => (message, QueryType::DoT),
        QueryResponse::DoH(message) => (message, QueryType::DoH),
        QueryResponse::Custom(message) => (message, QueryType::Custom),
    }
}

fn get_message_from_response_ref(response: &QueryResponse) -> (&Message<Vec<u8>>, QueryType) {
    match response {
        QueryResponse::UDP(message) => (message, QueryType::UDP),
        QueryResponse::TCP(message) => (message, QueryType::TCP),
        QueryResponse::DoT(message) => (message, QueryType::DoT),
        QueryResponse::DoH(message) => (message, QueryType::DoH),
        QueryResponse::Custom(message) => (message, QueryType::Custom),
    }
}
