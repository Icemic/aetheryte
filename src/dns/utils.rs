use crate::router::GeoIP;
use domain::{
    base::Message,
    base::{
        iana::{Opcode, Rcode},
        opt::rfc7830::PaddingMode,
        MessageBuilder,
    },
};
use domain::{
    base::{
        opt::Opt,
        opt::{ClientSubnet, KeyTag, Padding, TcpKeepalive},
        record::AsRecord,
    },
    rdata::AllRecordData,
};
use std::{net::Ipv4Addr, sync::Arc};

#[derive(Debug)]
pub enum QueryType {
    UDP,
    TCP,
    DoT,
    DoH,
    Custom,
}

pub enum QueryResponse {
    UDP(Message<Vec<u8>>),
    TCP(Message<Vec<u8>>),
    DoT(Message<Vec<u8>>),
    DoH(Message<Vec<u8>>),
    Custom(Message<Vec<u8>>),
}

pub fn decorate_message<T: AsRecord>(
    origin: &Message<Vec<u8>>,
    answers: Option<Vec<T>>,
) -> Message<Vec<u8>> {
    let mut msg = MessageBuilder::new_vec();
    msg.header_mut().set_opcode(Opcode::Query);
    msg.header_mut().set_id(origin.header().id());
    msg.header_mut().set_rd(true);
    // msg.header_mut().set_aa(true);
    msg.header_mut().set_ra(true);
    msg.header_mut().set_qr(false);
    msg.header_mut().set_rcode(Rcode::NoError);

    let msg = if let Some(answers) = answers {
        let mut _msg = msg.start_answer(origin, Rcode::NoError).unwrap();
        _msg.header_mut().set_qr(true);
        for answer in answers {
            _msg.push(answer).unwrap();
        }
        _msg
    } else {
        let mut msg = msg.question();

        for question in origin.question() {
            let question = question.unwrap();
            msg.push(question).unwrap();
        }
        msg.answer()
    };

    let mut msg = msg.additional();
    let mut additionals_copied = false;
    let options = origin.additional().unwrap();
    for record in options {
        let option = record
            .unwrap()
            .into_record::<Opt<&[u8]>>()
            .unwrap()
            .unwrap();
        msg.push(&option).unwrap();
        additionals_copied = true;
    }

    if !additionals_copied {
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
    }

    let buf = msg.finish();
    Message::from_octets(buf).unwrap()
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
