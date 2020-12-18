use std::net::{IpAddr, SocketAddr};

use domain::base::opt::{ClientSubnet, KeyTag, Padding, TcpKeepalive};
use domain::{
    base::Message,
    base::{
        iana::{Opcode, OptionCode},
        opt::rfc7830::PaddingMode,
        MessageBuilder,
    },
    rdata::AllRecordData,
};
use tokio::net::UdpSocket;

pub struct DNSServer {
    server: UdpSocket,
}

impl DNSServer {
    pub async fn new() -> Self {
        let server = match UdpSocket::bind("0.0.0.0:5353").await {
            Ok(server) => server,
            Err(_) => {
                panic!("error on udp listening");
            }
        };
        // Message::from_octets(octets)
        DNSServer { server }
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
            let ret_buf = self
                .lookup_udp(message, "8.8.8.8:53".parse().unwrap())
                .await;
            let ret_buf: &[u8] = &ret_buf;
            let ret_message = Message::from_octets(ret_buf).unwrap();

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
                .send_to(ret_message.into_octets(), addr)
                .await
                .expect("failed to send back via udp.");
        }
        Ok(())
    }
    pub fn decorate_message(&self, origin: Message<Vec<u8>>) -> Message<Vec<u8>> {
        // message.header().set_rd(true);
        // message.opt().unwrap().rcode(header)
        let mut msg = MessageBuilder::new_vec();
        msg.header_mut().set_opcode(Opcode::Query);
        msg.header_mut().set_id(origin.header().id());
        msg.header_mut().set_rd(true);
        msg.header_mut().set_aa(true);
        msg.header_mut().set_ra(true);

        let mut msg = msg.question();

        for question in origin.question() {
            let question = question.unwrap();
            msg.push(question).unwrap();
        }

        let mut msg = msg.additional();
        msg.opt(|opt| {
            opt.set_dnssec_ok(true);
            opt.set_udp_payload_size(1024);
            opt.set_version(0);
            let option1 = ClientSubnet::new(16, 16, "127.0.0.1".parse().unwrap());
            let option2 = ClientSubnet::new(64, 64, "fe80::".parse().unwrap());
            let padding = Padding::new(31, PaddingMode::Zero);
            let tcp_keepalive = TcpKeepalive::new(150);
            let key_tag = KeyTag::new(&[1, 2, 3, 82]);

            opt.push(&option1).unwrap();
            opt.push(&option2).unwrap();
            opt.push(&padding).unwrap();
            opt.push(&tcp_keepalive).unwrap();
            opt.push(&key_tag).unwrap();
            Ok(())
        })
        .unwrap();

        let target = msg.finish();
        Message::from_octets(target).unwrap()
    }
    pub async fn lookup_udp(&self, message: Message<Vec<u8>>, remote_addr: SocketAddr) -> Vec<u8> {
        let local_addr: SocketAddr = if remote_addr.is_ipv4() {
            "0.0.0.0:0".parse().unwrap()
        } else {
            "[::]:0".parse().unwrap()
        };
        let mut socket = UdpSocket::bind(local_addr).await.unwrap();
        socket.connect(remote_addr.to_string()).await.unwrap();
        socket.send(&message.into_octets()).await.unwrap();
        let mut buf = vec![0u8; 4096];
        let size = socket.recv(&mut buf).await.unwrap();
        buf[..size].into()
        // Message::from_octets(&buf[..size]).unwrap()
    }
}
