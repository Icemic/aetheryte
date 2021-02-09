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

pub fn get_request_message(origin: &Message<Vec<u8>>) -> Message<Vec<u8>> {
    let mut msg = MessageBuilder::new_vec();
    let header_mut = msg.header_mut();
    header_mut.set_opcode(Opcode::Query);
    header_mut.set_id(origin.header().id());
    header_mut.set_rd(true);
    header_mut.set_aa(true);
    header_mut.set_ra(true);
    header_mut.set_qr(false);
    header_mut.set_rcode(Rcode::NoError);

    let mut msg = msg.question();

    for question in origin.question() {
        let question = question.unwrap();
        msg.push(question).unwrap();
    }
    let msg = msg.answer();

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

pub fn get_response_message<T: AsRecord>(
    id: u16,
    origin: &Message<Vec<u8>>,
    force_answers: Option<Vec<T>>,
) -> Message<Vec<u8>> {
    let msg = MessageBuilder::new_vec();

    let mut msg = msg.start_answer(origin, Rcode::NoError).unwrap();
    let header = origin.header();
    let header_mut = msg.header_mut();
    header_mut.set_id(id);
    header_mut.set_aa(header.aa());
    header_mut.set_tc(header.tc());
    header_mut.set_rd(header.rd());
    header_mut.set_ra(header.ra());
    header_mut.set_ad(header.ad());
    header_mut.set_cd(true);
    header_mut.set_qr(true);

    if let Some(force_answers) = force_answers {
        for answer in force_answers {
            msg.push(answer).unwrap();
        }
    } else {
        let answers = origin.answer().unwrap().limit_to::<AllRecordData<_, _>>();
        for answer in answers {
            let answer = answer.expect("parsing has failed.");
            msg.push(answer).unwrap();
        }
    }

    let mut msg = msg.additional();
    let options = origin.additional().unwrap();
    for record in options {
        let option = record
            .unwrap()
            .into_record::<Opt<&[u8]>>()
            .unwrap()
            .unwrap();
        msg.push(&option).unwrap();
    }

    let buf = msg.finish();
    let msg = Message::from_octets(buf).unwrap();

    msg
}
