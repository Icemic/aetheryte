use std::net::SocketAddr;

use crate::dns::DNSServer;
use domain::base::Message;
use tokio::prelude::*;
use tokio::{net::TcpStream, time::timeout, time::Duration};

impl DNSServer {
    pub async fn lookup_tcp(
        &self,
        message: Message<Vec<u8>>,
        remote_addr: SocketAddr,
    ) -> Result<Message<Vec<u8>>, String> {
        let mut socket = TcpStream::connect(remote_addr.to_string()).await.unwrap();

        let packet = self.get_wrapped_packet(message);
        socket.write(&packet).await.unwrap();

        let mut packet = Vec::with_capacity(1024);
        let size = match timeout(Duration::from_millis(500), socket.read_to_end(&mut packet)).await
        {
            Ok(r) => r.unwrap(),
            Err(_) => {
                return Err("Query timeout.".to_string());
            }
        };

        if size > 2 {
            let high = *packet.get(0).unwrap();
            let low = *packet.get(1).unwrap();
            let expected_size = (((high as u16) << 8) + (low as u16)) as usize;
            if expected_size < 12 {
                return Err("[TCP] Below DNS minimum packet length.".to_string());
            }
        } else {
            packet.clear();
        }

        // tips: here omits checking packet size
        let ret_message = Message::from_octets(packet[2..].to_vec()).unwrap();

        if self.is_valid_response(&ret_message) {
            return Ok(ret_message);
        }

        Err("[TCP] Packet size checking failed.".to_string())
    }
}
