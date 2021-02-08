use super::{lookup::utils::QueryResponse, utils::decorate_message};
use domain::{base::Message, rdata::AllRecordData};
use redis::{aio::Connection, AsyncCommands};
use std::{
    io::{Error, ErrorKind},
    sync::Arc,
};
use tokio::sync::Mutex;

pub async fn lookup_cache(
    message: &Message<Vec<u8>>,
    redis: &Arc<Mutex<Option<Connection>>>,
    domain: &String,
) -> Result<(QueryResponse, bool), Error> {
    let mut redis = redis.lock().await;
    if redis.is_some() {
        let redis = redis.as_mut().unwrap();
        let mut buf: Vec<u8> = redis.get::<String, Vec<u8>>(domain.clone()).await.unwrap();
        if buf.is_empty() {
            return Err(Error::new(ErrorKind::NotFound, "[Cache] Not found"));
        }
        let is_china = buf.pop().unwrap() > 0;
        let saved_message = Message::from_octets(buf).unwrap();

        let answers = saved_message
            .answer()
            .unwrap()
            .limit_to::<AllRecordData<_, _>>();

        let mut answers_vec = vec![];
        for answer in answers {
            let answer = answer.expect("parsing has failed.");
            answers_vec.push(answer);
        }

        let ret_message = decorate_message(&message, Some(answers_vec));

        return Ok((QueryResponse::Cache(ret_message), is_china));
    }

    // return Ok(QueryResponse::Custom(ret_message));
    Err(Error::new(ErrorKind::NotFound, "[Cache] Not found"))
}
