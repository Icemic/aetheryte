use crate::dns::DNSServer;
use domain::base::Message;
use rustls_native_certs::load_native_certs;
use std::io::{Error, ErrorKind};
use std::str::FromStr;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::{net::TcpStream, time::timeout, time::Duration};
use tokio_rustls::{
    rustls::{ClientConfig, ProtocolVersion},
    webpki::DNSNameRef,
    TlsConnector,
};

impl DNSServer {
    pub async fn lookup_doh(
        &self,
        message: &Message<Vec<u8>>,
        remote_addr: &std::string::String,
        hostname: &std::string::String,
    ) -> Result<(String, Message<Vec<u8>>), Error> {
        let mut config = ClientConfig::new();
        config.root_store = load_native_certs().unwrap();
        config.enable_sni = true;
        config.enable_early_data = true;
        config.versions = vec![ProtocolVersion::TLSv1_3, ProtocolVersion::TLSv1_2];
        let connector = TlsConnector::from(Arc::new(config));
        let socket = TcpStream::connect(format!("{}:{}", remote_addr, 443)).await?;

        let mut socket = connector
            .connect(DNSNameRef::try_from_ascii_str(hostname).unwrap(), socket)
            .await?;

        // let packet = self.get_wrapped_packet(message);
        let packet = message.as_octets();

        let mut data = std::string::String::new();
        data.push_str("POST /dns-query HTTP/1.1\r\n");
        data.push_str(format!("Host: {}\r\n", hostname).as_str());
        data.push_str("Content-Type: application/dns-message\r\n");
        data.push_str(format!("Content-Length: {}\r\n", packet.len()).as_str());
        data.push_str("\r\n");

        socket.write(&data.as_bytes()).await?;
        socket.write(&packet).await?;

        // It stores the response message
        let mut packet = Vec::with_capacity(1024);

        timeout(Duration::from_millis(1000), socket.read_buf(&mut packet)).await??;

        if std::str::from_utf8(&packet[..15]).unwrap() != "HTTP/1.1 200 OK" {
            return Err(Error::new(
                ErrorKind::Other,
                "[DoH] non-200 status response.".to_string(),
            ));
        }

        // find response body size
        let size_position = match packet
            .windows(15)
            .position(|s| std::str::from_utf8(&s).unwrap() == "Content-Length:")
        {
            Some(p) => p,
            None => {
                return Err(Error::new(
                    ErrorKind::Other,
                    "[DoH] Wrong response.".to_string(),
                ));
            }
        };
        let body_size: Vec<u8> = packet
            .iter()
            .skip(size_position + 15)
            .take_while(|s| **s != 13)
            .map(|c| *c)
            .collect();
        let body_size = std::str::from_utf8(&body_size).unwrap().trim();
        let body_size = usize::from_str(body_size).unwrap();

        // slice DNS message out of http response message
        let packet: Vec<u8> = packet.into_iter().rev().take(body_size).rev().collect();

        let ret_message = Message::from_octets(packet.to_vec()).unwrap();

        if self.is_valid_response(&ret_message) {
            return Ok(("DoH".to_string(), ret_message));
        }

        Err(Error::new(
            ErrorKind::InvalidData,
            "[DoH] Packet size checking failed.".to_string(),
        ))
    }
}
