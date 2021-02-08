use super::utils::{get_wrapped_packet, is_valid_response, QueryResponse};
use domain::base::Message;
use std::io::{Error, ErrorKind};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::{net::TcpStream, time::timeout, time::Duration};

pub async fn lookup_tcp(
    message: &Message<Vec<u8>>,
    remote_addr: &std::string::String,
) -> Result<QueryResponse, Error> {
    let mut socket = TcpStream::connect(format!("{}:{}", remote_addr, 53)).await?;

    let packet = get_wrapped_packet(message);
    socket.write(&packet).await?;

    let mut packet = Vec::with_capacity(1024);
    timeout(Duration::from_millis(2000), socket.read_buf(&mut packet)).await??;

    // tips: here omits checking packet size
    let ret_message = Message::from_octets(packet[2..].to_vec()).unwrap();

    if is_valid_response(&ret_message) {
        return Ok(QueryResponse::TCP(ret_message));
    }

    Err(Error::new(
        ErrorKind::InvalidData,
        "[TCP] Packet size checking failed.".to_string(),
    ))
}
