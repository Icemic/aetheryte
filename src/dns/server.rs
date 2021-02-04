use super::{
    custom::lookup_custom,
    doh::lookup_doh,
    dot::lookup_dot,
    settings::{DNSServerUpstream, DNSSettings},
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
use futures::future::{select_ok, try_join};
use glob::Pattern;
use std::io::{Error, ErrorKind};
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream, UdpSocket};
use tokio::time::{timeout, Duration};

pub struct DNSServer {
    server_udp: Arc<UdpSocket>,
    server_tcp: Arc<TcpListener>,
    geoip: Arc<GeoIP>,
    settings: Arc<DNSSettings>,
    custom_patterns: Arc<Vec<(Pattern, String)>>,
}

enum TargetType {
    UDP(String),
    TCP(TcpStream, String),
}

impl DNSServer {
    pub async fn new() -> Self {
        let geoip = Arc::new(GeoIP::new().await);
        let settings = Arc::new(Self::load_settings().await);
        let custom_patterns = Arc::new(Self::load_patterns(Self::load_settings().await));
        let server_udp =
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
        let server_tcp =
            match TcpListener::bind(format!("{}:{}", settings.listen_ip, settings.listen_port))
                .await
            {
                Ok(server) => {
                    println!(
                        "DNS service now is serving on tcp://{}:{}",
                        settings.listen_ip, settings.listen_port
                    );
                    server
                }
                Err(e) => {
                    panic!("error on udp listening: {}", e);
                }
            };

        let server_udp = Arc::new(server_udp);
        let server_tcp = Arc::new(server_tcp);

        DNSServer {
            server_udp,
            server_tcp,
            geoip,
            settings,
            custom_patterns,
        }
    }

    pub fn load_patterns(settings: DNSSettings) -> Vec<(Pattern, String)> {
        let mut patterns = vec![];
        for (key, value) in settings.custom_hosts {
            if let Ok(pattern) = Pattern::new(key.as_str()) {
                patterns.push((pattern, value));
            } else {
                println!("[Custom] Failed to load pattern {}", key);
            }
        }
        patterns
    }

    pub async fn start_udp(&self) -> Result<(), Error> {
        let mut buf = vec![0u8; 4096];
        loop {
            let (size, addr) = self.server_udp.recv_from(&mut buf).await?;

            let task = run_task(
                self.server_udp.clone(),
                self.server_tcp.clone(),
                self.settings.clone(),
                self.custom_patterns.clone(),
                self.geoip.clone(),
                TargetType::UDP(addr.to_string()),
                buf[..size].to_vec(),
            );

            tokio::spawn(task);
        }
        #[allow(unreachable_code)]
        Ok(())
    }

    pub async fn start_tcp(&self) -> Result<(), Error> {
        let mut buf = vec![0u8; 4096];
        loop {
            let (socket, addr) = self.server_tcp.accept().await?;

            socket.readable().await?;
            let size = socket.try_read(&mut buf)?;

            let task = run_task(
                self.server_udp.clone(),
                self.server_tcp.clone(),
                self.settings.clone(),
                self.custom_patterns.clone(),
                self.geoip.clone(),
                TargetType::TCP(socket, addr.to_string()),
                buf[..size].to_vec(),
            );

            tokio::spawn(task);
        }
        #[allow(unreachable_code)]
        Ok(())
    }

    pub async fn start(&self) -> Result<(), Error> {
        try_join(self.start_udp(), self.start_tcp()).await?;
        Ok(())
    }
}

async fn run_task(
    server_udp: Arc<UdpSocket>,
    _: Arc<TcpListener>,
    settings: Arc<DNSSettings>,
    custom_patterns: Arc<Vec<(Pattern, String)>>,
    geoip: Arc<GeoIP>,
    target: TargetType,
    buf: Vec<u8>,
) -> Result<(), Error> {
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
            return Err(Error::new(
                ErrorKind::Other,
                format!("Timeout on batch query {}, skip the task.", domain),
            ));
        }
    }

    let (ret_message, method) = get_message_from_response(response);

    let answers = ret_message
        .answer()
        .unwrap()
        .limit_to::<AllRecordData<_, _>>();

    let mut i: u8 = 0;
    let mut answer_log = String::new();
    for answer in answers {
        i += 1;
        let answer = answer.expect("parsing has failed.");
        answer_log.push_str(format!("{} {}, ", answer.rtype(), answer.data().to_string()).as_str());
    }

    let ret_buf = ret_message.into_octets();
    let t;
    let source;
    match target {
        TargetType::UDP(addr) => {
            server_udp
                .send_to(&ret_buf, addr.clone())
                .await
                .expect("failed to send back via udp.");

            t = "UDP";
            source = addr;
        }
        TargetType::TCP(socket, addr) => {
            socket.writable().await?;
            socket
                .try_write(&ret_buf)
                .expect("failed to send back via tcp.");

            t = "TCP";
            source = addr
        }
    }

    if i == 0 {
        println!(
            "<{}> -> [{:?} {} {}] {} --> {} ({}) #{}",
            t,
            method,
            if is_china { "China" } else { "Abroad" },
            source,
            domain,
            "-",
            "-",
            i
        );
    } else {
        answer_log.pop();
        answer_log.pop();
        println!(
            "<{}> -> [{:?} {} {}] {} --> {}",
            t,
            method,
            if is_china { "China" } else { "Abroad" },
            source,
            domain,
            answer_log
        );
    }

    Ok(())
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

    let duration = Duration::from_millis(5000);

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
