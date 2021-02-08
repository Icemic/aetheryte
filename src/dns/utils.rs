use domain::base::{
    opt::Opt,
    opt::{ClientSubnet, KeyTag, Padding, TcpKeepalive},
    record::AsRecord,
};
use domain::{
    base::Message,
    base::{
        iana::{Opcode, Rcode},
        opt::rfc7830::PaddingMode,
        MessageBuilder,
    },
};

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
