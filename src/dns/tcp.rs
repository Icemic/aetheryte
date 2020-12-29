use crate::dns::DNSServer;
use domain::base::Message;
use std::io::{Error, ErrorKind};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::{net::TcpStream, time::timeout, time::Duration};

impl DNSServer {
    pub async fn lookup_tcp(
        &self,
        message: &Message<Vec<u8>>,
        remote_addr: &std::string::String,
    ) -> Result<(String, Message<Vec<u8>>), Error> {
        let mut socket = TcpStream::connect(format!("{}:{}", remote_addr, 53)).await?;

        let packet = self.get_wrapped_packet(message);
        socket.write(&packet).await?;

        let mut packet = Vec::with_capacity(1024);
        timeout(Duration::from_millis(500), socket.read_buf(&mut packet)).await??;

        // tips: here omits checking packet size
        let ret_message = Message::from_octets(packet[2..].to_vec()).unwrap();

        if self.is_valid_response(&ret_message) {
            return Ok(("TCP".to_string(), ret_message));
        }

        Err(Error::new(
            ErrorKind::InvalidData,
            "[TCP] Packet size checking failed.".to_string(),
        ))
    }
}
