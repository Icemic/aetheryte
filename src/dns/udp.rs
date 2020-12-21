use std::net::SocketAddr;

use crate::dns::DNSServer;
use domain::{
    base::Message,
};
use tokio::{net::UdpSocket, time::timeout};

impl DNSServer {
    pub async fn lookup_udp(
        &self,
        message: Message<Vec<u8>>,
        remote_addr: SocketAddr,
    ) -> Result<Message<Vec<u8>>, String> {
        let local_addr: SocketAddr = if remote_addr.is_ipv4() {
            "0.0.0.0:0".parse().unwrap()
        } else {
            "[::]:0".parse().unwrap()
        };
        let socket = UdpSocket::bind(local_addr).await.unwrap();
        socket.connect(remote_addr.to_string()).await.unwrap();
        socket.send(&message.into_octets()).await.unwrap();

        let duration = tokio::time::Duration::from_millis(500);
        let mut ret_message;
        loop {
            let mut buf = vec![0u8; 1024];
            let size = match timeout(duration, socket.recv(&mut buf)).await {
                Ok(r) => r.unwrap(),
                Err(_) => {
                    return Err("Query timeout.".to_string());
                }
            };
            ret_message = Message::from_octets(buf[..size].to_vec()).unwrap();
            if self.is_valid_response(&ret_message) {
                break;
            }
        }

        Ok(ret_message)
    }
}
