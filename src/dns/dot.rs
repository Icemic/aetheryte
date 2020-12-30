use super::utils::{get_wrapped_packet, is_valid_response, QueryResponse};
use domain::base::Message;
use rustls_native_certs::load_native_certs;
use std::io::{Error, ErrorKind};
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::{net::TcpStream, time::timeout, time::Duration};
use tokio_rustls::{
    rustls::{ClientConfig, ProtocolVersion},
    webpki::DNSNameRef,
    TlsConnector,
};

pub async fn lookup_dot(
    message: &Message<Vec<u8>>,
    remote_addr: &std::string::String,
    hostname: &std::string::String,
) -> Result<QueryResponse, Error> {
    let mut config = ClientConfig::new();
    config.root_store = load_native_certs().unwrap();
    config.enable_sni = true;
    config.enable_early_data = true;
    config.versions = vec![ProtocolVersion::TLSv1_3, ProtocolVersion::TLSv1_2];
    let connector = TlsConnector::from(Arc::new(config));

    let socket = TcpStream::connect(format!("{}:{}", remote_addr, 853)).await?;

    let mut socket = connector
        .connect(DNSNameRef::try_from_ascii_str(hostname).unwrap(), socket)
        .await?;

    let packet = get_wrapped_packet(message);
    socket.write(&packet).await?;

    let mut packet = Vec::with_capacity(1024);

    timeout(Duration::from_millis(1000), socket.read_buf(&mut packet)).await??;

    // tips: here omits checking packet size
    let ret_message = Message::from_octets(packet[2..].to_vec()).unwrap();

    if is_valid_response(&ret_message) {
        return Ok(QueryResponse::DoT(ret_message));
    }

    Err(Error::new(
        ErrorKind::InvalidData,
        "[DoT] Packet size checking failed.".to_string(),
    ))
}
