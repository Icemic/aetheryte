use crate::dns::DNSServer;
use domain::base::Message;
use rustls_native_certs::load_native_certs;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::{net::TcpStream, time::timeout, time::Duration};
use tokio_rustls::{
    rustls::{ClientConfig, ProtocolVersion},
    webpki::DNSNameRef,
    TlsConnector,
};

impl DNSServer {
    pub async fn lookup_dot(
        &self,
        message: &Message<Vec<u8>>,
        remote_addr: &std::string::String,
        hostname: &std::string::String,
    ) -> Result<(String, Message<Vec<u8>>), String> {
        let mut config = ClientConfig::new();
        config.root_store = load_native_certs().unwrap();
        config.enable_sni = true;
        config.enable_early_data = true;
        config.versions = vec![ProtocolVersion::TLSv1_3, ProtocolVersion::TLSv1_2];
        let connector = TlsConnector::from(Arc::new(config));

        let socket = TcpStream::connect(format!("{}:{}", remote_addr, 853))
            .await
            .unwrap();

        let mut socket = connector
            .connect(DNSNameRef::try_from_ascii_str(hostname).unwrap(), socket)
            .await
            .unwrap();

        let packet = self.get_wrapped_packet(message);
        socket.write(&packet).await.unwrap();

        let mut packet = Vec::with_capacity(1024);

        let size = match timeout(Duration::from_millis(1000), socket.read_buf(&mut packet)).await {
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
            return Ok(("DoT".to_string(), ret_message));
        }

        Err("[DoT] Packet size checking failed.".to_string())
    }
}
