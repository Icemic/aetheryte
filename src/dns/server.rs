use super::settings::DNSSettings;
use crate::router::GeoIP;
use domain::rdata::A;
use domain::{
    base::{Dname, Message, Record},
    rdata::AllRecordData,
};
use futures::{future::select_ok, Future};
use glob::Pattern;
use std::io::Error;
use std::{net::Ipv4Addr, pin::Pin};
use tokio::net::UdpSocket;
use tokio::time::{timeout, Duration};

pub struct DNSServer {
    server: UdpSocket,
    geoip: GeoIP,
    settings: DNSSettings,
    pub custom_patterns: Vec<(Pattern, String)>,
}

impl DNSServer {
    pub async fn new() -> Self {
        let settings = Self::load_settings().await;
        let geoip = GeoIP::new().await;
        let custom_patterns = Self::load_patterns(&settings);
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
        DNSServer {
            server,
            geoip,
            settings,
            custom_patterns,
        }
    }
    pub async fn start(&mut self) -> Result<(), ()> {
        let mut buf = vec![0u8; 4096];
        loop {
            let (size, addr) = match self.server.recv_from(&mut buf).await {
                Ok(socket) => socket,
                Err(_) => {
                    println!("error on accept socket");
                    continue;
                }
            };

            let message = Message::from_octets(buf[..size].to_vec()).unwrap();
            let domain = message.first_question().unwrap().qname().to_string();

            let method;
            let is_china;
            let ret_message;
            if let Some(message) = self.lookup_custom(&message) {
                method = "Custom".to_string();
                is_china = true;
                ret_message = message;
            } else {
                let message = &self.decorate_message::<Record<Dname<&[u8]>, A>>(&message, None);
                if let Ok(result) = self.batch_query(&message).await {
                    method = result.0;
                    is_china = result.1;
                    ret_message = result.2;
                } else {
                    println!("[Warning] batch query timeout, skip the task.");
                    continue;
                }
            }
            let answers = ret_message
                .answer()
                .unwrap()
                .limit_to::<AllRecordData<_, _>>();

            let mut i = 0;
            for answer in answers {
                i += 1;
                let answer = answer.expect("parsing has failed.");
                println!(
                    "[{} {} {}] {} --> {} ({}) #{}",
                    method,
                    if is_china { "China" } else { "Abroad" },
                    addr.to_string(),
                    domain,
                    answer.data().to_string(),
                    answer.rtype(),
                    i
                );
            }

            self.server
                .send_to(&ret_message.into_octets(), addr)
                .await
                .expect("failed to send back via udp.");
        }
        #[allow(unreachable_code)]
        Ok(())
    }

    async fn batch_query(
        &self,
        message: &Message<Vec<u8>>,
    ) -> Result<(String, bool, Message<Vec<u8>>), Error> {
        let mut queries_china: Vec<
            Pin<Box<dyn Future<Output = Result<(String, Message<Vec<u8>>), Error>>>>,
        > = vec![];
        let mut queries_abroad: Vec<
            Pin<Box<dyn Future<Output = Result<(String, Message<Vec<u8>>), Error>>>>,
        > = vec![];

        for upstream in &self.settings.upstreams {
            let queries;
            if upstream.is_china {
                queries = &mut queries_china;
            } else {
                queries = &mut queries_abroad;
            }

            if upstream.enable_udp {
                let ret_message = self.lookup_udp(message, &upstream.address);
                queries.push(Box::pin(ret_message));
            }
            if upstream.enable_tcp {
                let ret_message = self.lookup_tcp(message, &upstream.address);
                queries.push(Box::pin(ret_message));
            }
            if upstream.enable_dot {
                let ret_message = self.lookup_dot(message, &upstream.address, &upstream.hostname);
                queries.push(Box::pin(ret_message));
            }
            if upstream.enable_doh {
                let ret_message = self.lookup_doh(message, &upstream.address, &upstream.hostname);
                queries.push(Box::pin(ret_message));
            }
        }

        let duration = Duration::from_millis(1000);

        if let Ok(((method, ret_message_china), _)) =
            timeout(duration, select_ok(queries_china)).await?
        {
            if self.is_china_site(&ret_message_china) {
                return Ok((method, true, ret_message_china));
            }
        }

        let (ret_message, _) = timeout(duration, select_ok(queries_abroad)).await??;
        Ok((ret_message.0, false, ret_message.1))
    }
    fn is_china_site(&self, message: &Message<Vec<u8>>) -> bool {
        let mut answers_china = message.answer().unwrap().limit_to::<AllRecordData<_, _>>();
        let answer = answers_china.next();
        if answer.is_none() {
            return false;
        }
        if let Ok(answer_first_china) = answer.unwrap() {
            let rtype = answer_first_china.rtype().to_string();

            if rtype == "CNAME" {
                return true;
            }

            let ip = answer_first_china.data().to_string();
            let ip: Ipv4Addr = match ip.parse() {
                Ok(ip) => ip,
                Err(_) => return false,
            };

            if rtype.starts_with('A') && self.geoip.lookup_country_code(&ip) == "CN" {
                return true;
            }
        }
        false
    }
}
