use super::{lookup::utils::QueryResponse, utils::get_response_message};
use domain::{
    base::{Dname, Message, Record},
    rdata::A,
};
use redis::{aio::Connection, AsyncCommands};
use std::{
    io::{Error, ErrorKind},
    sync::Arc,
};
use tokio::sync::Mutex;

pub async fn lookup_cache(
    message: &Message<Vec<u8>>,
    redis: &Arc<Mutex<Option<Connection>>>,
    identifier: &String,
) -> Result<(QueryResponse, bool), Error> {
    let mut redis = redis.lock().await;
    if redis.is_some() {
        let redis = redis.as_mut().unwrap();
        let mut buf: Vec<u8> = redis
            .get::<String, Vec<u8>>(identifier.clone())
            .await
            .unwrap();
        if buf.is_empty() {
            return Err(Error::new(ErrorKind::NotFound, "[Cache] Not found"));
        }
        let is_china = buf.pop().unwrap() > 0;
        let saved_message = Message::from_octets(buf).unwrap();
        let ret_message = get_response_message::<Record<Dname<Vec<u8>>, A>>(
            message.header().id(),
            &saved_message,
            None,
        );

        return Ok((QueryResponse::Cache(ret_message), is_china));
    }

    // return Ok(QueryResponse::Custom(ret_message));
    Err(Error::new(ErrorKind::NotFound, "[Cache] Not found"))
}
