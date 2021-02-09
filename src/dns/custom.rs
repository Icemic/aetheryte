use super::{lookup::utils::QueryResponse, utils::get_response_message};
use domain::base::iana::Class;
use domain::base::{Dname, Message, Record};
use domain::rdata::{Aaaa, A};
use glob::Pattern;
use std::{
    io::{Error, ErrorKind},
    net::IpAddr,
};

pub async fn lookup_custom(
    message: &Message<Vec<u8>>,
    custom_patterns: &Vec<(Pattern, String)>,
    domain: &String,
) -> Result<QueryResponse, Error> {
    for (pattern, value) in custom_patterns {
        if pattern.matches(domain.as_str()) {
            let ip: IpAddr = value.parse().unwrap();
            let ret_message;
            match ip {
                IpAddr::V4(ip4) => {
                    let record = Record::new(
                        Dname::vec_from_str(domain).unwrap(),
                        Class::In,
                        120,
                        A::new(ip4),
                    );
                    ret_message =
                        get_response_message(message.header().id(), &message, Some(vec![record]));
                }
                IpAddr::V6(ip6) => {
                    let record = Record::new(
                        Dname::vec_from_str(domain).unwrap(),
                        Class::In,
                        120,
                        Aaaa::new(ip6),
                    );
                    ret_message =
                        get_response_message(message.header().id(), &message, Some(vec![record]));
                }
            }

            return Ok(QueryResponse::Custom(ret_message));
        }
    }
    Err(Error::new(ErrorKind::NotFound, "[Custom] Not found"))
}
