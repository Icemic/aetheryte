use super::{
    cache::lookup_cache,
    custom::lookup_custom,
    lookup::{batch_query, utils::get_message_from_response},
    settings::DNSSettings,
    utils::get_request_message,
};
use crate::router::GeoIP;
use core::panic;
use domain::{base::Message, rdata::AllRecordData};
use futures::future::try_join;
use glob::Pattern;
use redis::{aio::Connection, AsyncCommands};
use std::io::{Error, ErrorKind};
use std::sync::Arc;
use tokio::{
    net::{TcpListener, TcpStream, UdpSocket},
    sync::Mutex,
};

pub struct DNSServer {
    server_udp: Arc<UdpSocket>,
    server_tcp: Arc<TcpListener>,
    redis: Arc<Mutex<Option<Connection>>>,
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

        let mut redis = None;
        if let Some(redis_server) = &settings.redis_server {
            let client = redis::Client::open(redis_server.clone()).unwrap();
            redis = match client.get_async_connection().await {
                Ok(server) => {
                    println!("Using redis at {} as cache.", redis_server);
                    Some(server)
                }
                Err(e) => {
                    panic!("error on redis instance: {}", e);
                }
            };
        }

        let redis = Arc::new(Mutex::new(redis));

        DNSServer {
            server_udp,
            server_tcp,
            geoip,
            redis,
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
                self.redis.clone(),
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
                self.redis.clone(),
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
    redis: Arc<Mutex<Option<Connection>>>,
    geoip: Arc<GeoIP>,
    target: TargetType,
    buf: Vec<u8>,
) -> Result<(), Error> {
    let message = Message::from_octets(buf).unwrap();
    let question = message.first_question().unwrap();
    let domain = question.qname().to_string();
    let identifier = format!(
        "{}|{}|{}",
        question.qname(),
        question.qtype(),
        question.qclass()
    );

    let is_china;
    let mut is_cache = false;
    let response;
    if let Ok(r) = lookup_custom(&message, &custom_patterns, &domain).await {
        response = r;
        is_china = true;
    } else if let Ok((r, china)) = lookup_cache(&message, &redis, &identifier).await {
        response = r;
        is_china = china;
        is_cache = true;
    } else {
        let message = get_request_message(&message);
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

    // save to cache
    let mut redis = redis.lock().await;
    if !is_cache && redis.is_some() && settings.cache_expire.is_some() {
        let expire = settings.cache_expire.unwrap();
        let redis = redis.as_mut().unwrap();
        let mut cache_buf = ret_buf.clone();
        cache_buf.push(is_china as u8);
        match redis
            .set_ex::<String, Vec<u8>, String>(identifier, cache_buf, expire)
            .await
        {
            Ok(_) => {
                //
            }
            Err(err) => {
                println!("Failed to save data to cache ({}), ignored.", err);
            }
        };
    }

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
