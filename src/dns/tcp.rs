use crate::dns::DNSServer;
use domain::base::Message;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::{net::TcpStream, time::timeout, time::Duration};

impl DNSServer {
    pub async fn lookup_tcp(
        &self,
        message: &Message<Vec<u8>>,
        remote_addr: &std::string::String,
    ) -> Result<(String, Message<Vec<u8>>), String> {
        let mut socket = TcpStream::connect(format!("{}:{}", remote_addr, 53))
            .await
            .unwrap();

        let packet = self.get_wrapped_packet(message);
        socket.write(&packet).await.unwrap();

        let mut packet = Vec::with_capacity(1024);
        let result = match timeout(Duration::from_millis(500), socket.read_buf(&mut packet)).await {
            Ok(r) => r,
            Err(_) => {
                return Err("Query timeout.".to_string());
            }
        };

        let size = match result {
            Ok(s) => s,
            Err(msg) => return Err(msg.to_string()),
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
            return Ok(("TCP".to_string(), ret_message));
        }

        Err("[TCP] Packet size checking failed.".to_string())
    }
}
