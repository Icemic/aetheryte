use super::utils::{is_valid_response_udp, QueryResponse};
use domain::base::Message;
use std::{io::Error, net::SocketAddr};
use tokio::{net::UdpSocket, time::timeout};

pub async fn lookup_udp(
    message: &Message<Vec<u8>>,
    remote_addr: &std::string::String,
) -> Result<QueryResponse, Error> {
    let remote_addr: SocketAddr = format!("{}:{}", remote_addr, 53).parse().unwrap();
    let local_addr: SocketAddr = if remote_addr.is_ipv4() {
        "0.0.0.0:0".parse().unwrap()
    } else {
        "[::]:0".parse().unwrap()
    };

    let socket = UdpSocket::bind(local_addr).await?;
    socket.connect(remote_addr).await?;
    socket.send(message.as_octets()).await?;

    let duration = tokio::time::Duration::from_millis(2000);
    let mut ret_message;
    loop {
        let mut buf = vec![0u8; 1024];
        let size = timeout(duration, socket.recv(&mut buf)).await??;
        ret_message = Message::from_octets(buf[..size].to_vec()).unwrap();
        if is_valid_response_udp(&ret_message) {
            break;
        }
    }

    Ok(QueryResponse::UDP(ret_message))
}
