use super::settings::{self, DNSSettings};
use crate::router::GeoIP;
use domain::{base::Message, rdata::AllRecordData};
use futures::{
    future::{join_all, select_all, select_ok},
    join, Future,
};
use std::{net::Ipv4Addr, pin::Pin};
use tokio::net::UdpSocket;

pub struct DNSServer {
    server: UdpSocket,
    geoip: GeoIP,
    settings: DNSSettings,
}

impl DNSServer {
    pub async fn new() -> Self {
        let settings = Self::load_settings().await;
        let geoip = GeoIP::new().await;
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
        DNSServer {
            server,
            geoip,
            settings,
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

            let message = &self.decorate_message(message);

            let (method, is_china, ret_message) = self.batch_query(&message).await;
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

    async fn batch_query(&self, message: &Message<Vec<u8>>) -> (String, bool, Message<Vec<u8>>) {
        let mut queries_china: Vec<
            Pin<Box<dyn Future<Output = Result<(String, Message<Vec<u8>>), String>>>>,
        > = vec![];
        let mut queries_abroad: Vec<
            Pin<Box<dyn Future<Output = Result<(String, Message<Vec<u8>>), String>>>>,
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
                let ret_message = self.lookup_dot(&message, &upstream.address, &upstream.hostname);
                queries.push(Box::pin(ret_message));
            }
            if upstream.enable_doh {
                let ret_message = self.lookup_doh(&message, &upstream.address, &upstream.hostname);
                queries.push(Box::pin(ret_message));
            }
        }

        let ((method, ret_message_china), _) = select_ok(queries_china).await.unwrap();

        if self.is_china_site(&ret_message_china) {
            return (method, true, ret_message_china);
        }

        let (ret_message, _) = select_ok(queries_abroad).await.unwrap();
        (ret_message.0, false, ret_message.1)
    }
    fn is_china_site(&self, message: &Message<Vec<u8>>) -> bool {
        let mut answers_china = message.answer().unwrap().limit_to::<AllRecordData<_, _>>();

        if let Ok(answer_first_china) = answers_china.next().unwrap() {
            let rtype = answer_first_china.rtype().to_string();
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
