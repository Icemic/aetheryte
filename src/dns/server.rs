use super::settings::DNSSettings;
use domain::{base::Message, rdata::AllRecordData};
use tokio::net::UdpSocket;

pub struct DNSServer {
    server: UdpSocket,
    settings: DNSSettings,
}

impl DNSServer {
    pub async fn new() -> Self {
        let server = match UdpSocket::bind("0.0.0.0:5353").await {
            Ok(server) => server,
            Err(_) => {
                panic!("error on udp listening");
            }
        };
        let settings = Self::load_settings().await;
        // Message::from_octets(octets)
        DNSServer { server, settings }
    }
    pub async fn start(&mut self) -> Result<(), ()> {
        // let mut buf = BytesMut::with_capacity(1024);
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

            println!("[{}, {} bytes] {}", addr.to_string(), size, domain);

            let message = self.decorate_message(message);

            // self.lookup_udp
            let ret_message = self
                .lookup_udp(message, "8.8.8.8:53".parse().unwrap())
                .await
                .unwrap();

            let answers = ret_message
                .answer()
                .unwrap()
                .limit_to::<AllRecordData<_, _>>();

            for answer in answers {
                let answer = answer.expect("parsing has failed.");
                println!(
                    "rtype {}, class {}, {}",
                    answer.rtype(),
                    answer.class(),
                    answer.data().to_string()
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
}
