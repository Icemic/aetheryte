use crate::dns::DNSServer;
use domain::base::{
    opt::{ClientSubnet, KeyTag, Padding, TcpKeepalive},
    record::AsRecord,
};
use domain::{
    base::Message,
    base::{iana::Opcode, opt::rfc7830::PaddingMode, MessageBuilder},
};

impl DNSServer {
    pub fn decorate_message<T: AsRecord>(
        &self,
        origin: &Message<Vec<u8>>,
        answers: Option<Vec<T>>,
    ) -> Message<Vec<u8>> {
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

        let mut msg = msg.answer();
        if let Some(answers) = answers {
            for answer in answers {
                msg.push(answer).unwrap();
            }
        }

        let mut msg = msg.additional();
        msg.opt(|opt| {
            opt.set_dnssec_ok(true);
            opt.set_udp_payload_size(1024);
            opt.set_version(0);
            let option1 = ClientSubnet::new(24, 0, "122.233.242.188".parse().unwrap());
            let option2 = ClientSubnet::new(64, 0, "240e:390:e5b:8280::1".parse().unwrap());
            let padding = Padding::new(31, PaddingMode::Zero);
            let tcp_keepalive = TcpKeepalive::new(20);
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
    pub fn is_valid_response(&self, message: &Message<Vec<u8>>) -> bool {
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
        }
        false
    }
    // add 2-byte head to packet
    pub fn get_wrapped_packet(&self, message: &Message<Vec<u8>>) -> Vec<u8> {
        let buf = &mut message.as_octets().clone();
        let mut packet = Vec::with_capacity(2 + buf.len());
        packet.push((buf.len() >> 8) as u8);
        packet.push(buf.len() as u8);
        packet.append(buf);
        packet
    }
}
