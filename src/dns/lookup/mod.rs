mod doh;
mod dot;
mod tcp;
mod udp;
pub mod utils;

use super::settings::DNSServerUpstream;
use crate::router::GeoIP;
use doh::*;
use domain::base::Message;
use dot::*;
use futures::future::select_ok;
use std::{io::Error, sync::Arc, time::Duration};
use tcp::*;
use tokio::time::timeout;
use udp::*;
use utils::{get_message_from_response_ref, is_china_site, QueryResponse, QueryType};

pub async fn lookup(
    t: QueryType,
    message: &Message<Vec<u8>>,
    upstream: &DNSServerUpstream,
) -> Result<QueryResponse, Error> {
    match t {
        QueryType::UDP => lookup_udp(message, &upstream.address).await,
        QueryType::TCP => lookup_tcp(message, &upstream.address).await,
        QueryType::DoT => lookup_dot(message, &upstream.address, &upstream.hostname).await,
        QueryType::DoH => lookup_doh(message, &upstream.address, &upstream.hostname).await,
        QueryType::Custom => panic!("Custom query should be performed independently"),
        QueryType::Cache => panic!("Cache query should be performed independently"),
    }
}

pub async fn batch_query(
    message: &Message<Vec<u8>>,
    upstreams: &Vec<DNSServerUpstream>,
    geoip: Arc<GeoIP>,
) -> Result<(QueryResponse, bool), Error> {
    let mut queries_china = vec![];
    let mut queries_abroad = vec![];

    for upstream in upstreams {
        let queries;
        if upstream.is_china {
            queries = &mut queries_china;
        } else {
            queries = &mut queries_abroad;
        }

        if upstream.enable_udp {
            let ret_message = lookup(QueryType::UDP, message, &upstream);
            queries.push(Box::pin(ret_message));
        }
        if upstream.enable_tcp {
            let ret_message = lookup(QueryType::TCP, message, &upstream);
            queries.push(Box::pin(ret_message));
        }
        if upstream.enable_dot {
            let ret_message = lookup(QueryType::DoT, message, &upstream);
            queries.push(Box::pin(ret_message));
        }
        if upstream.enable_doh {
            let ret_message = lookup(QueryType::DoH, message, &upstream);
            queries.push(Box::pin(ret_message));
        }
    }

    let duration = Duration::from_millis(5000);

    let (response, _) = timeout(duration, select_ok(queries_china)).await??;
    let (ret_message, _) = get_message_from_response_ref(&response);
    if is_china_site(&ret_message, geoip) {
        return Ok((response, true));
    }

    let (response, _) = timeout(duration, select_ok(queries_abroad)).await??;
    Ok((response, false))
}
